use crate::com::CliError;
use futures_util::{SinkExt, StreamExt};
use log::*;
use std::future::Future;
use std::pin::Pin;
use tokio::{
    sync::{
        mpsc::{self, Receiver, Sender},
        oneshot,
    },
    task::JoinHandle,
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::protocol::{frame::coding::CloseCode, CloseFrame, Message},
};

pub struct WsClient {
    pub url: String,
    pub tx: Sender<WsMessage>,
    task: (oneshot::Sender<()>, JoinHandle<anyhow::Result<()>>),
}
#[derive(Debug, Clone)]
pub enum WsMessage {
    Txt(String),
    Bin(Vec<u8>),
}
impl WsClient {
    pub async fn new<F>(url: String, handle_msg: F) -> anyhow::Result<Self>
    where
        F: 'static,
        F: Fn(
                WsMessage,
                &Sender<WsMessage>,
            ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>>
            + Send
            + Sync,
    {
        let (tx, rx) = mpsc::channel(1);
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let u = url.clone();
        let send_tx = tx.clone();
        Ok(Self {
            url,
            tx,
            task: (
                shutdown_tx,
                tokio::spawn(handle(u, shutdown_rx, send_tx, rx, handle_msg)),
            ),
        })
    }
    pub async fn send(&mut self, msg: WsMessage) -> anyhow::Result<()> {
        self.tx
            .send(msg)
            .await
            .map_err(|e| CliError::WebSocketError(e.to_string()))?;
        Ok(())
    }
    pub async fn shutdown(self) -> anyhow::Result<()> {
        debug!("Shutdown ws client: {}", self.url);
        self.task.0.send(()).map_err(|e| {
            CliError::WebSocketError(format!("Send shutdown signal failed: {:?}", e))
        })?;
        self.task.1.await??;
        Ok(())
    }
}

async fn handle<F>(
    url: String,
    mut shutdown_rx: oneshot::Receiver<()>,
    send_tx: Sender<WsMessage>,
    mut send_rx: Receiver<WsMessage>,
    handle_msg: F,
) -> anyhow::Result<()>
where
    F: 'static,
    F: Fn(
            WsMessage,
            &Sender<WsMessage>,
        ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>>
        + Send
        + Sync,
{
    let ws_stream = match connect_async(url.clone()).await {
        Ok((stream, response)) => {
            debug!("Server response was {:?}", response);
            stream
        }
        Err(e) => {
            error!("WebSocket handshake for client failed with {:?}!", e);
            return Err(e.into());
        }
    };
    // ws_stream
    let (mut sender, mut receiver) = ws_stream.split();
    debug!("Start ws client: {}", url);
    loop {
        tokio::select! {
            _ = (&mut shutdown_rx) => {
                info!("Got shutdown signal , break loop price ws client!");
                sender.send(Message::Close(Some(CloseFrame {
                    code: CloseCode::Normal,
                    reason: "Shutdown".into(),
                }))).await?;
                break;
            },
            msg = send_rx.recv() => {
                match msg {
                    Some(WsMessage::Txt(txt)) => {
                        debug!("Send text message: {}", txt);
                        sender.send(Message::Text(txt)).await?;
                    }
                    Some(WsMessage::Bin(bin)) => {
                        debug!("Send binary message: {:?}", bin);
                        sender.send(Message::Binary(bin)).await?;
                    }
                    None => {
                        debug!("Send message channel closed");
                        break;
                    }
                }
            },
            Some(Ok(msg)) = receiver.next() => {
                match msg {
                    Message::Text(text) => {
                        // debug!("Received text message: {}", text);
                        if let Err(e)=tokio::spawn(handle_msg(WsMessage::Txt(text), &send_tx)).await{
                            error!("Handle text message error: {:?}", e);
                        }
                    }
                    Message::Binary(bin) => {
                        // debug!("Received binary message: {:?}", bin);
                        if let Err(e)=tokio::spawn(handle_msg(WsMessage::Bin(bin), &send_tx)).await{
                            error!("Handle binary message error: {:?}", e);
                        }
                    }
                    Message::Ping(ping) => {
                        debug!("Received ping message: {:?}", ping);
                        sender.send(Message::Pong(ping)).await?;
                    }
                    Message::Pong(pong) => {
                        debug!("Received pong message: {:?}", pong);
                        sender.send(Message::Ping(pong)).await?;
                    }
                    Message::Close(close) => {
                        debug!("Received close message: {:?}", close);
                        break;
                    }
                    Message::Frame(_) => {
                        debug!("Received frame message");
                        break;
                    }
                }
            },
        }
    }
    Ok(())
}
