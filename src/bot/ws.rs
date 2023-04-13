use crate::bot::state::{Address, OrgPrice};
use crate::com::{CliError, Task};
use dashmap::{DashMap, DashSet};
use futures_util::{SinkExt, StreamExt};
use log::*;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{future::Future, sync::Arc};
use tokio::{
    sync::{
        broadcast,
        mpsc::{self, Receiver, Sender},
        oneshot,
    },
    time::{self, Duration},
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::protocol::{frame::coding::CloseCode, CloseFrame, Message},
};

// like Crypto.BTC/USD 0xf9c0172ba10dfa4d19088d94f5bf61d3b54d5bd7483a322a982e1373ee8ea31b
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct SymbolId {
    pub symbol: String,
    pub id: String,
}
// key is id(address), value is symbol
pub type DmSymbolId = DashMap<String, String>;
pub type SharedDmSymbolId = Arc<DmSymbolId>;
pub type SupportedSymbol = DashSet<String>;
pub fn new_shared_dm_symbol_id(
    symbol_id_vec: Vec<SymbolId>,
) -> (SharedDmSymbolId, SupportedSymbol) {
    let dm = DashMap::new();
    let ss = DashSet::new();
    for symbol_id in symbol_id_vec {
        let id = symbol_id.id.as_str();
        let id_key = id.strip_prefix("0x").unwrap_or(id).to_string();
        dm.insert(id_key.clone(), symbol_id.symbol.clone());
        ss.insert(symbol_id.symbol);
    }
    (Arc::new(dm), ss)
}
#[derive(Clone)]
pub struct WsServerState {
    // pub conns: DashMap<Address, Sender<WsSrvMessage>>,
    // pub sub_idx_map: DmPriceSubMap,
    pub supported_symbol: SupportedSymbol,
}
impl WsServerState {
    pub fn new(supported_symbol: SupportedSymbol) -> Self {
        Self {
            // conns: DashMap::new(),
            // sub_idx_map: DashMap::new(),
            supported_symbol,
        }
    }

    pub fn is_supported_symbol(&self, symbol: &String) -> bool {
        self.supported_symbol.contains(symbol)
    }

    // pub fn add_conn(&self, address: Address, tx: Sender<WsSrvMessage>) {
    //     self.conns.insert(address, tx);
    // }

    // pub fn remove_conn(&self, address: &Address) {
    //     self.conns.remove(address);
    // }

    // pub fn add_symbol_sub(&self, symbol: String, address: Address) {
    //     self.sub_idx_map
    //         .entry(symbol)
    //         .or_insert_with(DashSet::new)
    //         .insert(address);
    // }

    // pub fn remove_symbol_sub(&self, symbol: &String, address: &Address) {
    //     if let Some(set) = self.sub_idx_map.get_mut(symbol) {
    //         set.remove(address);
    //     }
    // }
}
// key is symbol, value is address set
// pub type DmPriceSubMap = DashMap<String, DashSet<Address>>;

pub struct PriceWatchRx(pub broadcast::Receiver<OrgPrice>);
pub struct WsWatchTx(pub broadcast::Sender<WsSrvMessage>);
pub struct WsWatchRx(pub broadcast::Receiver<WsSrvMessage>);
pub struct PriceStatusWatchRx(pub broadcast::Receiver<PriceStatus>);

impl Clone for PriceWatchRx {
    fn clone(&self) -> Self {
        Self(self.0.resubscribe())
    }
}

impl Clone for WsWatchRx {
    fn clone(&self) -> Self {
        Self(self.0.resubscribe())
    }
}
impl Clone for PriceStatusWatchRx {
    fn clone(&self) -> Self {
        Self(self.0.resubscribe())
    }
}
impl Clone for WsWatchTx {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
pub fn new_event_channel(size: usize) -> (WsWatchTx, WsWatchRx) {
    let (ws_tx, ws_rx) = broadcast::channel(size);
    (WsWatchTx(ws_tx), WsWatchRx(ws_rx))
}
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct PriceStatus {
    pub symbol: String,
    pub change_rate: f64,
    pub opening_price: i64,
    pub change: i64,
    pub high_24h: i64,
    pub low_24h: i64,
    pub current_price: i64,
}

#[derive(Debug, Clone)]
pub enum WsSrvMessage {
    AccountUpdate(AccountDynamicData),
    PositionUpdate(PositionDynamicData),
    PositionOpen(PositionDynamicData),
    PositionClose(PositionDynamicData),
    PriceUpdate(PriceStatus),
    SpreadUpdate(SpreadData),
    Close,
}

impl WsSrvMessage {
    pub fn into_txt(self) -> String {
        match self {
            Self::AccountUpdate(account) => Self::json_warp(
                "account_update",
                serde_json::to_string(&account).unwrap_or_default().as_str(),
            ),
            Self::PositionUpdate(positions) => Self::json_warp(
                "position_update",
                serde_json::to_string(&positions)
                    .unwrap_or_default()
                    .as_str(),
            ),
            Self::PositionOpen(position) => Self::json_warp(
                "position_open",
                serde_json::to_string(&position)
                    .unwrap_or_default()
                    .as_str(),
            ),
            Self::PositionClose(position) => Self::json_warp(
                "position_close",
                serde_json::to_string(&position)
                    .unwrap_or_default()
                    .as_str(),
            ),
            Self::SpreadUpdate(spread) => Self::json_warp(
                "spread_update",
                serde_json::to_string(&spread).unwrap_or_default().as_str(),
            ),
            Self::PriceUpdate(price_status) => Self::json_warp(
                "price_update",
                serde_json::to_string(&price_status)
                    .unwrap_or_default()
                    .as_str(),
            ),
            Self::Close => Self::json_warp("close", ""),
        }
    }
    fn json_warp(event: &str, data: &str) -> String {
        format!("{{\"event\":\"{}\",\"data\":{}}}", event, data)
    }
}
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct AccountDynamicData {
    #[serde(skip_serializing)]
    pub id: Address,
    pub balance: i64,
    pub profit: i64,
    pub margin_total: i64,
    pub margin_percentage: f64,
    pub equity: i64,
    pub profit_rate: f64,
}
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct PositionDynamicData {
    pub id: Address,
    #[serde(skip_serializing)]
    pub account_id: Address,
    pub profit_rate: f64,
    pub profit: i64,
}
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct SpreadData {
    pub id: Address,
    pub spread: u64,
    #[serde(skip_serializing)]
    pub symbol: String,
}
#[derive(Debug, Clone)]
pub enum SubType {
    Unsubscribe,
    Subscribe,
    None,
}

impl Serialize for SubType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let t = match *self {
            Self::Unsubscribe => "unsubscribe",
            Self::Subscribe => "subscribe",
            _ => "",
        };
        serializer.serialize_str(t)
    }
}
impl<'de> Deserialize<'de> for SubType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let r = match s.as_str() {
            "unsubscribe" => SubType::Unsubscribe,
            "subscribe" => SubType::Subscribe,
            _ => SubType::None,
        };
        Ok(r)
    }
}

impl Default for SubType {
    fn default() -> Self {
        Self::None
    }
}

pub struct WsClient {
    pub url: String,
    pub tx: Sender<WsClientMessage>,
    task: Task,
}
#[derive(Debug, Clone)]
pub enum WsClientMessage {
    Txt(String),
    Bin(Vec<u8>),
}
impl fmt::Display for WsClientMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Txt(txt) => write!(f, "{}", txt),
            Self::Bin(bin) => write!(f, "{:?}", bin),
        }
    }
}
impl WsClient {
    pub async fn new<F>(
        url: String,
        start_msg: Option<WsClientMessage>,
        handle_msg: F,
    ) -> anyhow::Result<Self>
    where
        F: 'static,
        F: Fn(
                WsClientMessage,
                &Sender<WsClientMessage>,
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
            task: Task::new(
                "ws client",
                shutdown_tx,
                tokio::spawn(handle(u, shutdown_rx, send_tx, rx, start_msg, handle_msg)),
            ),
        })
    }
    pub async fn send(&mut self, msg: WsClientMessage) -> anyhow::Result<()> {
        self.tx
            .send(msg)
            .await
            .map_err(|e| CliError::WebSocketError(e.to_string()))?;
        Ok(())
    }
    pub async fn shutdown(self) {
        self.task.shutdown().await;
    }
}

async fn handle<F>(
    url: String,
    mut shutdown_rx: oneshot::Receiver<()>,
    send_tx: Sender<WsClientMessage>,
    mut send_rx: Receiver<WsClientMessage>,
    start_msg: Option<WsClientMessage>,
    handle_msg: F,
) -> anyhow::Result<()>
where
    F: 'static,
    F: Fn(
            WsClientMessage,
            &Sender<WsClientMessage>,
        ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>>
        + Send
        + Sync,
{
    let first_runing = Arc::new(AtomicBool::new(true));
    'connection: loop {
        info!("Start price ws client: {}", url);
        let ws_stream = match connect_async(url.clone()).await {
            Ok((stream, response)) => {
                debug!("Server response was {:?}", response);
                stream
            }
            Err(e) => {
                error!("WebSocket handshake for client failed with {:?}!", e);
                // If the server is not running for the first time, it will continuously retry.
                if !first_runing.load(Ordering::Relaxed) {
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    continue 'connection;
                }
                return Err(e.into());
            }
        };
        // ws_stream
        let (mut sender, mut receiver) = ws_stream.split();
        // send sub msg
        let msg = start_msg.clone();
        first_runing.store(false, Ordering::Relaxed);
        match msg {
            Some(WsClientMessage::Txt(txt)) => {
                debug!("Send text sub message: {}", txt);
                sender.send(Message::Text(txt)).await?;
            }
            Some(WsClientMessage::Bin(bin)) => {
                sender.send(Message::Binary(bin)).await?;
            }
            None => {}
        }
        debug!("Start ws client: {}", url);
        let duration = Duration::from_secs(10);
        let mut timer = time::interval(duration);
        timer.tick().await;

        'sub: loop {
            tokio::select! {
                _ = (&mut shutdown_rx) => {
                    info!("Got shutdown signal , break loop price ws client!");
                    sender.send(Message::Close(Some(CloseFrame {
                        code: CloseCode::Normal,
                        reason: "Shutdown".into(),
                    }))).await?;
                    break 'connection;
                },
                msg = send_rx.recv() => {
                    match msg {
                        Some(WsClientMessage::Txt(txt)) => {
                            debug!("Send text message: {}", txt);
                            sender.send(Message::Text(txt)).await?;
                        }
                        Some(WsClientMessage::Bin(bin)) => {
                            debug!("Send binary message: {:?}", bin);
                            sender.send(Message::Binary(bin)).await?;
                        }
                        None => {
                            debug!("Send message channel closed");
                            break 'connection;
                        }
                    }
                },
                Some(Ok(msg)) = receiver.next() => {
                    timer.reset();
                    match msg {
                        Message::Text(text) => {
                            debug!("Received text message: {}", text);
                            if let Err(e)=tokio::spawn(handle_msg(WsClientMessage::Txt(text), &send_tx)).await{
                                error!("Handle text message error: {:?}", e);
                            }
                        }
                        Message::Binary(bin) => {
                            // debug!("Received binary message: {:?}", bin);
                            if let Err(e)=tokio::spawn(handle_msg(WsClientMessage::Bin(bin), &send_tx)).await{
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
                        Message::Frame(_) => {
                            debug!("Received frame message");
                        }
                        Message::Close(close) => {
                            debug!("Received close message: {:?}", close);
                            break 'sub;
                        }
                    }
                },
                _ = timer.tick() => {
                    info!("recv timeout, reset connection");
                    // sender.send(Message::Close(Some(CloseFrame {
                    //     code: CloseCode::Normal,
                    //     reason: "Reset".into(),
                    // }))).await?;
                    time::sleep(duration).await;
                    break 'sub;
                }
            }
        }
        info!("price ws client disconnected, reconnecting...");
    }
    info!("price ws client shutdown");
    Ok(())
}
