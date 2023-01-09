use crate::com::CliError;
use futures_util::{SinkExt, StreamExt};
use log::*;
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
pub enum WsMessage {
    Txt(String),
    Bin(Vec<u8>),
}
impl WsClient {
    pub async fn new(url: String) -> anyhow::Result<Self> {
        let (tx, rx) = mpsc::channel(1);
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let u = url.clone();
        Ok(Self {
            url,
            tx,
            task: (shutdown_tx, tokio::spawn(handle(u, shutdown_rx, rx))),
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

async fn handle(
    url: String,
    mut shutdown_rx: oneshot::Receiver<()>,
    mut send_rx: Receiver<WsMessage>,
) -> anyhow::Result<()> {
    let ws_stream = match connect_async(url.clone()).await {
        Ok((stream, response)) => {
            debug!("Server response was {:?}", response);
            stream
        }
        Err(e) => {
            debug!("WebSocket handshake for client failed with {:?}!", e);
            return Err(e.into());
        }
    };
    let (mut sender, mut receiver) = ws_stream.split();
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
            Some(Ok(msg))=receiver.next() => {
                match msg {
                    Message::Text(text) => {
                        debug!("Received text message: {}", text);
                    }
                    Message::Binary(bin) => {
                        debug!("Received binary message: {:?}", bin);
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
