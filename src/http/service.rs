use crate::bot::{
    self,
    influxdb::Influxdb,
    state::{Account, Address, Market, OrgPrice, Position, State},
    ws::{PriceStatus, PriceStatusWatchRx, PriceWatchRx, SubType, WsSrvMessage, WsWatchRx},
};

use crate::bot::{
    machine::SharedStateMap,
    storage::local::{self, Local},
};
use crate::com::{self, ClientError, Task};
use axum::extract::ws::{Message, WebSocket};
use cached::proc_macro::cached;
use chrono::Utc;
use csv::ReaderBuilder;
use dashmap::{DashMap, DashSet};
use influxdb2_client::models::Query;
use log::*;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::{broadcast, oneshot};

pub async fn get_symbol_list(ssm: SharedStateMap) -> anyhow::Result<Vec<String>> {
    let mut rs: Vec<String> = Vec::new();
    for i in ssm.ws_state.supported_symbol.iter() {
        rs.push(i.clone());
    }
    Ok(rs)
}
#[derive(Debug, Clone)]
struct PricesCache<T> {
    prices: T,
    last_update: i64,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Price {
    #[serde(rename(deserialize = "_value", serialize = "value"))]
    value: i64,
    #[serde(rename(deserialize = "_start", serialize = "time"))]
    time: String,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PriceColumn {
    #[serde(rename(deserialize = "_start", serialize = "start_time"))]
    start_time: String,
    #[serde(rename(deserialize = "_stop", serialize = "stop_time"))]
    stop_time: String,
    #[serde(rename(deserialize = "_value_first", serialize = "open"))]
    value_first: i64,
    #[serde(rename(deserialize = "_value_last", serialize = "close"))]
    value_last: i64,
    #[serde(rename(deserialize = "_value_min", serialize = "low"))]
    value_min: i64,
    #[serde(rename(deserialize = "_value_max", serialize = "high"))]
    value_max: i64,
}

fn get_price_history_query(
    bucket: &str,
    start: &str,
    symbol: &str,
    feed: &str,
    window: &str,
) -> String {
    format!(
        r#"from(bucket: "{}")
        |> range(start: {})
        |> filter(fn: (r) => r["_measurement"] == "{}")
        |> filter(fn: (r) => r["_field"] == "price")
        |> filter(fn: (r) => r["feed"] == "{}")
        |> window(every: {})
        |> keep(columns: ["_value","_start","_stop"])
        |> first()
        "#,
        bucket, start, symbol, feed, window
    )
}

fn get_start_and_window(range: &str) -> anyhow::Result<(String, String)> {
    match range {
        "1H" => Ok(("-4d".to_string(), "1h".to_string())),
        "1D" => Ok(("-90d".to_string(), "1d".to_string())),
        "1W" => Ok(("-1y".to_string(), "1w".to_string())),
        "1M" => Ok(("-10y".to_string(), "1mo".to_string())),
        "1Y" => Ok(("-10y".to_string(), "1y".to_string())),
        _ => Err(ClientError::InvalidRange.into()),
    }
}

pub async fn init_price_history_cache(ssm: SharedStateMap, db: Arc<Influxdb>) {
    let mut tasks = Vec::new();
    for i in ssm.ws_state.supported_symbol.iter() {
        let symbol = i.clone();
        let db = db.clone();
        tasks.push(tokio::spawn(async move {
            if let Err(e) = init_price_cache_with_symbol(symbol, db).await {
                error!("init price history data error:{}", e);
            };
        }));
    }
    for task in tasks {
        task.await.unwrap();
    }
}

async fn init_price_cache_with_symbol(symbol: String, db: Arc<Influxdb>) -> anyhow::Result<()> {
    let ranges = vec![
        "1H".to_string(),
        "1D".to_string(),
        "1W".to_string(),
        "1M".to_string(),
        "1Y".to_string(),
    ];
    for range in ranges {
        if let Err(e) =
            get_price_history_with_cache(symbol.clone(), range.clone(), db.clone()).await
        {
            error!("init price history data error:{} , symbol: {}", e, symbol);
        };
        if let Err(e) =
            get_price_history_column_with_cache(symbol.clone(), range.clone(), db.clone()).await
        {
            error!("init price history data error:{} , symbol: {}", e, symbol);
        };
    }
    Ok(())
}
pub fn get_cache_key(symbol: &String, range: &String) -> String {
    format!("{}-{}", symbol.as_str(), range.as_str())
}
// cache for 7 days
#[cached(
    time = 604800,
    key = "String",
    convert = r#"{ get_cache_key(&symbol, &range) }"#,
    result = true
)]
async fn get_price_history_with_cache(
    symbol: String,
    range: String,
    db: Arc<Influxdb>,
) -> anyhow::Result<PricesCache<Vec<Price>>> {
    let (start, window) = get_start_and_window(range.as_str())?;
    let rs = get_price_history_from_db(symbol, start, window, db).await?;
    Ok(PricesCache {
        prices: rs,
        last_update: Utc::now().timestamp(),
    })
}

async fn get_price_history_from_db(
    symbol: String,
    start: String,
    window: String,
    db: Arc<Influxdb>,
) -> anyhow::Result<Vec<Price>> {
    let query = get_price_history_query(
        db.bucket.as_str(),
        start.as_str(),
        symbol.as_str(),
        "price",
        window.as_str(),
    );
    debug!("price db query: {}", query);
    let db_query_rs = db
        .client
        .query_raw(db.org.as_str(), Some(Query::new(query)))
        .await?;
    let rs = ReaderBuilder::new()
        .delimiter(b',')
        .from_reader(db_query_rs.as_bytes())
        .deserialize()
        .collect::<Result<Vec<Price>, _>>()?;
    debug!("price db query result: {:?}", rs);
    Ok(rs)
}

pub async fn get_price_history(
    symbol: Option<String>,
    range: Option<String>,
    db: Arc<Influxdb>,
) -> anyhow::Result<Vec<Price>> {
    let symbol = symbol.ok_or_else(|| ClientError::UnknownSymbol)?;
    if symbol.is_empty() {
        return Err(ClientError::UnknownSymbol.into());
    }
    let range = range.ok_or_else(|| ClientError::InvalidRange)?;
    let (_, window) = get_start_and_window(range.as_str())?;
    let mut ch = get_price_history_with_cache(symbol.clone(), range, db.clone()).await?;
    let rs =
        get_price_history_from_db(symbol.clone(), ch.last_update.to_string(), window, db).await?;
    for r in rs {
        ch.prices.push(r);
    }
    Ok(ch.prices)
}

fn get_price_history_column_query(
    bucket: &str,
    start: &str,
    symbol: &str,
    feed: &str,
    window: &str,
) -> String {
    format!(
        r#"dataSet=from(bucket: "{}")
    |> range(start: {})
    |> filter(fn: (r) => r["_measurement"] == "{}")
    |> filter(fn: (r) => r["_field"] == "price")
    |> filter(fn: (r) => r["feed"] == "{}")
    |> window(every: {})
    |> keep(columns: ["_value","_start","_stop"])
    dataMin = dataSet|> min()
    dataMax = dataSet|> max()
    dataFirst = dataSet|> first()
    dataLast = dataSet|> last()
    j1=join(tables: {{min: dataMin, max: dataMax}}, on: ["_start", "_stop"], method: "inner")
    j2=join(tables: {{first: dataFirst, last: dataLast}}, on: ["_start", "_stop"], method: "inner")
    join(tables: {{t1: j1, t2: j2}}, on: ["_start", "_stop"], method: "inner")
    "#,
        bucket, start, symbol, feed, window
    )
}

fn get_24h_price_status_query(bucket: &str, symbol: &str, feed: &str) -> String {
    format!(
        r#"dataSet=from(bucket: "{}")
    |> range(start: -24h)
    |> filter(fn: (r) => r["_measurement"] == "{}")
    |> filter(fn: (r) => r["_field"] == "price")
    |> filter(fn: (r) => r["feed"] == "{}")
    |> keep(columns: ["_value","_start","_stop"])
    dataMin = dataSet|> min()
    dataMax = dataSet|> max()
    dataFirst = dataSet|> first()
    dataLast = dataSet|> last()
    j1=join(tables: {{min: dataMin, max: dataMax}}, on: ["_start", "_stop"], method: "inner")
    j2=join(tables: {{first: dataFirst, last: dataLast}}, on: ["_start", "_stop"], method: "inner")
    join(tables: {{t1: j1, t2: j2}}, on: ["_start", "_stop"], method: "inner")
    "#,
        bucket, symbol, feed
    )
}
// cache for 7 days
#[cached(
    time = 604800,
    key = "String",
    convert = r#"{ get_cache_key(&symbol, &range) }"#,
    result = true
)]
async fn get_price_history_column_with_cache(
    symbol: String,
    range: String,
    db: Arc<Influxdb>,
) -> anyhow::Result<PricesCache<Vec<PriceColumn>>> {
    let (start, window) = get_start_and_window(range.as_str())?;
    let rs = get_price_history_column_from_db(symbol, start, window, db).await?;
    Ok(PricesCache {
        prices: rs,
        last_update: Utc::now().timestamp(),
    })
}
async fn get_price_history_column_from_db(
    symbol: String,
    start: String,
    window: String,
    db: Arc<Influxdb>,
) -> anyhow::Result<Vec<PriceColumn>> {
    let query = get_price_history_column_query(
        db.bucket.as_str(),
        start.as_str(),
        symbol.as_str(),
        "price",
        window.as_str(),
    );
    debug!("price db query: {}", query);
    let db_query_rs = db
        .client
        .query_raw(db.org.as_str(), Some(Query::new(query)))
        .await?;
    let rs = ReaderBuilder::new()
        .delimiter(b',')
        .from_reader(db_query_rs.as_bytes())
        .deserialize()
        .collect::<Result<Vec<PriceColumn>, _>>()?;
    debug!("price db query result: {:?}", rs);
    Ok(rs)
}
pub async fn get_price_history_column(
    symbol: Option<String>,
    range: Option<String>,
    db: Arc<Influxdb>,
) -> anyhow::Result<Vec<PriceColumn>> {
    let symbol = symbol.ok_or_else(|| ClientError::UnknownSymbol)?;
    if symbol.is_empty() {
        return Err(ClientError::UnknownSymbol.into());
    }
    let range = range.ok_or_else(|| ClientError::InvalidRange)?;
    let (_, window) = get_start_and_window(range.as_str())?;
    let mut ch = get_price_history_column_with_cache(symbol.clone(), range, db.clone()).await?;
    let rs =
        get_price_history_column_from_db(symbol.clone(), ch.last_update.to_string(), window, db)
            .await?;
    for r in rs {
        ch.prices.push(r);
    }
    Ok(ch.prices)
}

// key: symbol , value: PriceStatus
pub type DmPriceStatus = Arc<DashMap<String, PriceStatus>>;

pub fn new_price_status() -> DmPriceStatus {
    Arc::new(DashMap::new())
}

pub async fn init_price_status(
    ssm: SharedStateMap,
    dps: DmPriceStatus,
    db: Arc<Influxdb>,
) -> anyhow::Result<()> {
    debug!("init price status");
    for symbol in ssm.ws_state.supported_symbol.iter() {
        let query = get_24h_price_status_query(db.bucket.as_str(), symbol.as_str(), "price");
        let db_query_rs = db
            .client
            .query_raw(db.org.as_str(), Some(Query::new(query)))
            .await?;
        let rs = ReaderBuilder::new()
            .delimiter(b',')
            .from_reader(db_query_rs.as_bytes())
            .deserialize()
            .collect::<Result<Vec<PriceColumn>, _>>()?;
        if let Some(p) = rs.get(0) {
            let price_status = PriceStatus {
                symbol: symbol.to_string(),
                change_rate: 0.0,
                change: 0,
                opening_price: p.value_first,
                high_24h: p.value_max,
                low_24h: p.value_min,
                current_price: 0,
            };
            dps.insert(symbol.to_string(), price_status);
        }
    }
    Ok(())
}

fn get_broadcast_price_status(
    dps: &DmPriceStatus,
    org_price: &OrgPrice,
) -> anyhow::Result<Option<PriceStatus>> {
    if let Some(mut price_status) = dps.get_mut(&org_price.symbol) {
        if org_price.price > price_status.value().high_24h {
            price_status.value_mut().high_24h = org_price.price;
        }
        if org_price.price < price_status.value().low_24h {
            price_status.value_mut().low_24h = org_price.price;
        }
        let mut price_status = price_status.value().clone();
        price_status.current_price = org_price.price;
        price_status.change = org_price.price - price_status.opening_price;
        price_status.change_rate =
            com::f64_round_4(price_status.change as f64 / price_status.opening_price as f64);
        return Ok(Some(price_status));
    }
    Ok(None)
}
pub struct PriceBroadcast {
    task: Task,
}

impl PriceBroadcast {
    pub async fn new(
        ssm: SharedStateMap,
        dps: DmPriceStatus,
        price_ws_rx: PriceWatchRx,
        db: Arc<Influxdb>,
    ) -> (Self, PriceStatusWatchRx) {
        let (shutdown_tx, shutdown_rx) = Task::new_shutdown_channel();
        let (price_status_ws_tx, price_status_ws_rx) =
            broadcast::channel(ssm.ws_state.supported_symbol.len());
        let task = Task::new(
            "price broadcast",
            shutdown_tx,
            tokio::spawn(broadcast_price(
                dps.clone(),
                price_ws_rx,
                price_status_ws_tx,
                shutdown_rx,
            )),
        );
        tokio::spawn(async move {
            if let Err(e) = init_price_status(ssm, dps, db).await {
                error!("init price status error: {}", e);
            }
        });
        (Self { task }, PriceStatusWatchRx(price_status_ws_rx))
    }
    pub async fn shutdown(self) {
        self.task.shutdown().await;
    }
}

async fn broadcast_price(
    dps: DmPriceStatus,
    mut price_ws_rx: PriceWatchRx,
    price_status_ws_tx: broadcast::Sender<PriceStatus>,
    mut shutdown_rx: oneshot::Receiver<()>,
) -> anyhow::Result<()> {
    loop {
        tokio::select! {
            _ = (&mut shutdown_rx) => {
                info!("got shutdown signal , break price broadcast!");
                break;
            },
            Ok(price) = price_ws_rx.0.recv() => {
                if let Ok(Some(price_status)) = get_broadcast_price_status(&dps, &price) {
                    if let Err(e) = price_status_ws_tx.send(price_status) {
                        error!("broadcast price status error: {}", e);
                    }
                }
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubRequest {
    pub symbol: String,
    pub sub_type: SubType,
}

pub async fn handle_ws(
    ssm: SharedStateMap,
    mut socket: WebSocket,
    address: Option<Address>,
    mut price_status_rx: PriceStatusWatchRx,
    mut event_ws_rx: WsWatchRx,
) {
    // let (tx, mut rx) = mpsc::channel::<WsSrvMessage>(10);
    let symbols_set: DashSet<String> = DashSet::new();
    // let is_login = address.is_some();
    let mut user_account = Address::default();
    if let Some(addr) = address {
        user_account = addr;
    }
    loop {
        tokio::select! {
            Ok(price_status) = price_status_rx.0.recv() => {
                // debug!("got price from ws broadcast channel: {:?}", price);
                if symbols_set.contains(&price_status.symbol) {
                    if let Err(e) = socket.send(Message::Text(WsSrvMessage::PriceUpdate(price_status).into_txt())).await {
                        error!("send ws message error: {}", e);
                        break;
                    }
                }
            }
            Ok(ws_event) = event_ws_rx.0.recv() => {
                match ws_event {
                    WsSrvMessage::AccountUpdate(data) => {
                        if data.id == user_account {
                            if let Err(e) = socket.send(Message::Text(WsSrvMessage::AccountUpdate(data).into_txt())).await {
                                error!("send ws message error: {}", e);
                                break;
                            }
                        }
                    },
                    WsSrvMessage::PositionUpdate(data) => {
                        if data.account_id == user_account {
                            debug!("got position update: {:?}", data);
                            if let Err(e) = socket.send(Message::Text(WsSrvMessage::PositionUpdate(data).into_txt())).await {
                                error!("send ws message error: {}", e);
                                break;
                            }
                        }
                    },
                    WsSrvMessage::PositionOpen(data) => {
                        if data.account_id == user_account {
                            if let Err(e) = socket.send(Message::Text(WsSrvMessage::PositionOpen(data).into_txt())).await {
                                error!("send ws message error: {}", e);
                                break;
                            }
                        }
                    },
                    WsSrvMessage::PositionClose(data) => {
                        if data.account_id == user_account {
                            if let Err(e) = socket.send(Message::Text(WsSrvMessage::PositionClose(data).into_txt())).await {
                                error!("send ws message error: {}", e);
                                break;
                            }
                        }
                    },
                    WsSrvMessage::SpreadUpdate(data) => {
                        if symbols_set.contains(&data.symbol) {
                            if let Err(e) = socket.send(Message::Text(WsSrvMessage::SpreadUpdate(data).into_txt())).await {
                                error!("send ws message error: {}", e);
                                break;
                            }
                        }
                    },
                    WsSrvMessage::Close => {
                        if let Err(e) = socket.send(Message::Text(WsSrvMessage::Close.into_txt())).await {
                            error!("send ws message error: {}", e);
                        }
                        debug!("close ws connection");
                        break;
                    }
                    _ => {}
                }
            }
            ws_msg = socket.recv() => {
                match ws_msg {
                    Some(Ok(msg))=>{
                        match msg {
                            Message::Text(t) => {
                                handle_ws_events(ssm.clone(), t, &symbols_set);
                            }
                            Message::Binary(_) => {
                                debug!("client sent binary data");
                            }
                            Message::Ping(ping) => {
                                debug!("socket got ping");
                                if let Err(e) = socket.send(Message::Pong(ping)).await{
                                    error!("send ws pong message error: {}", e);
                                    break;
                                }
                            }
                            Message::Pong(pong) => {
                                debug!("socket got pong");
                                if let Err(e) = socket.send(Message::Ping(pong)).await{
                                    error!("send ws ping message error: {}", e);
                                    break;
                                }
                            }
                            Message::Close(_) => {
                                debug!("client disconnected");
                                break;
                            }
                        }
                    }
                    Some(Err(e)) => {
                        error!("recv ws message error: {}", e);
                        break;
                    }
                    None => {
                        debug!("client disconnected");
                        break;
                    }
                }
            }
        }
    }
    // info!("client disconnected, clean connection :{:?}", user_account);
}

fn handle_ws_events(
    ssm: SharedStateMap,
    msg: String,
    // address: &Address,
    symbols_set: &DashSet<String>,
) {
    let sub_req: SubRequest = serde_json::from_str(msg.as_str()).unwrap();
    let symbol = sub_req.symbol;
    if !ssm.ws_state.is_supported_symbol(&symbol) {
        return;
    }
    let sub_type = sub_req.sub_type;
    match sub_type {
        SubType::Subscribe => {
            // ssm.ws_state.add_symbol_sub(symbol.clone(), address.copy());
            symbols_set.insert(symbol.clone());
        }
        SubType::Unsubscribe => {
            // ssm.ws_state.remove_symbol_sub(&symbol, &address);
            // symbols.retain(|s| s != &symbol);
            symbols_set.remove(&symbol);
        }
        SubType::None => {}
    }
}
