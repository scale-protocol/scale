use crate::bot::{
    self,
    influxdb::Influxdb,
    state::{Account, Address, Market, OrgPrice, Position, State},
    ws::{PriceWatchRx, SubType, WsSrvMessage},
};
use crate::bot::{machine, storage};
use crate::com::{self, CliError, Task};
use axum::extract::ws::{Message, WebSocket};
use cached::proc_macro::cached;
use csv::ReaderBuilder;
use dashmap::{DashMap, DashSet};
use influxdb2_client::models::Query;
use log::*;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;
use tokio::{
    sync::{mpsc, oneshot},
    time::{self, Duration},
};

pub fn get_account_info(
    mp: bot::machine::SharedStateMap,
    address: String,
) -> anyhow::Result<Option<Account>> {
    let address = Address::from_str(address.as_str())
        .map_err(|e| CliError::HttpServerError(e.to_string()))?;
    let r = mp.account.get(&address);
    Ok(r.map(|a| a.value().clone()))
}

pub fn get_position_info(
    mp: bot::machine::SharedStateMap,
    address: String,
    position_address: String,
) -> anyhow::Result<Option<Position>> {
    let address = Address::from_str(address.as_str())
        .map_err(|e| CliError::HttpServerError(e.to_string()))?;
    let position_address = Address::from_str(position_address.as_str())
        .map_err(|e| CliError::HttpServerError(e.to_string()))?;
    let r = mp.position.get(&address);
    if let Some(p) = r {
        let v = p.value();
        let s = v.get(&position_address);
        if let Some(p) = s {
            return Ok(Some(p.clone()));
        }
    }
    Ok(mp.storage.get_position_info(&address, &position_address))
}

pub fn get_position_list(
    mp: machine::SharedStateMap,
    prefix: String,
    address: String,
) -> anyhow::Result<Vec<Position>> {
    let address = Address::from_str(address.as_str())
        .map_err(|e| CliError::HttpServerError(e.to_string()))?;
    let prefix = storage::Prefix::from_str(prefix.as_str())?;
    let mut rs: Vec<Position> = Vec::new();
    match prefix {
        storage::Prefix::Active => {
            let r = mp.position.get(&address);
            match r {
                Some(p) => {
                    for i in p.value().iter() {
                        rs.push(i.clone());
                    }
                }
                None => {}
            }
        }
        storage::Prefix::History => {
            let items = mp.storage.get_position_history_list(&address);
            for i in items {
                match i {
                    Ok((_k, v)) => {
                        // let key = String::from_utf8(k.to_vec())
                        //     .map_err(|e| CliError::JsonError(e.to_string()))?;
                        // let keys = storage::Keys::from_str(key.as_str())?;
                        // let pk = keys.get_end();
                        // let pbk = Address::from_str(pk.as_str())
                        //     .map_err(|e| CliError::Unknown(e.to_string()))?;
                        let values: State = serde_json::from_slice(v.to_vec().as_slice())
                            .map_err(|e| CliError::JsonError(e.to_string()))?;
                        match values {
                            State::Position(p) => {
                                rs.push(p);
                            }
                            _ => {}
                        }
                    }
                    Err(e) => {
                        error!("{}", e);
                    }
                }
            }
        }
        storage::Prefix::None => {}
    }
    Ok(rs)
}

pub async fn get_market_list(
    mp: machine::SharedStateMap,
    prefix: String,
) -> anyhow::Result<Vec<Market>> {
    let prefix = storage::Prefix::from_str(prefix.as_str())?;
    let mut rs: Vec<Market> = Vec::new();
    match prefix {
        storage::Prefix::Active => {
            for i in mp.market.iter() {
                if i.value().id
                    != Address::from_str(
                        "0xfd8a967be00215082a4500701aff7628eda05409c3f8ad32db619ffd2f96ffee",
                    )
                    .unwrap()
                {
                    continue;
                }
                rs.push(i.value().clone());
            }
        }
        storage::Prefix::History => {
            let items = mp.storage.get_market_history_list();
            for i in items {
                match i {
                    Ok((_k, v)) => {
                        let values: State = serde_json::from_slice(v.to_vec().as_slice())
                            .map_err(|e| CliError::JsonError(e.to_string()))?;
                        match values {
                            State::Market(m) => {
                                rs.push(m);
                            }
                            _ => {}
                        }
                    }
                    Err(e) => {
                        error!("{}", e);
                    }
                }
            }
        }
        storage::Prefix::None => {}
    }
    Ok(rs)
}
pub async fn get_symbol_list(mp: machine::SharedStateMap) -> anyhow::Result<Vec<String>> {
    let mut rs: Vec<String> = Vec::new();
    for i in mp.ws_state.supported_symbol.iter() {
        rs.push(i.clone());
    }
    Ok(rs)
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
        _ => Err(CliError::InvalidRange.into()),
    }
}
#[cached(
    time = 60,
    key = "String",
    convert = r#"{ get_cache_key(&symbol, &range) }"#,
    result = true
)]
pub async fn get_price_history(
    symbol: Option<String>,
    range: Option<String>,
    db: Arc<Influxdb>,
) -> anyhow::Result<Vec<Price>> {
    let symbol = symbol.ok_or_else(|| CliError::UnknownSymbol)?;
    if symbol.is_empty() {
        return Err(CliError::UnknownSymbol.into());
    }
    let range = range.ok_or_else(|| CliError::InvalidRange)?;
    let (start, window) = get_start_and_window(range.as_str())?;
    let query = get_price_history_query(
        db.bucket.as_str(),
        start.as_str(),
        symbol.as_str(),
        "price",
        window.as_str(),
    );
    let db_query_rs = db
        .client
        .query_raw(db.org.as_str(), Some(Query::new(query)))
        .await?;
    let rs = ReaderBuilder::new()
        .delimiter(b',')
        .from_reader(db_query_rs.as_bytes())
        .deserialize()
        .collect::<Result<Vec<Price>, _>>()?;
    Ok(rs)
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

pub fn get_cache_key(symbol: &Option<String>, range: &Option<String>) -> String {
    format!("{}-{}", symbol.as_ref().unwrap(), range.as_ref().unwrap())
}

#[cached(
    time = 60,
    key = "String",
    convert = r#"{ get_cache_key(&symbol, &range) }"#,
    result = true
)]
pub async fn get_price_history_column(
    symbol: Option<String>,
    range: Option<String>,
    db: Arc<Influxdb>,
) -> anyhow::Result<Vec<PriceColumn>> {
    let symbol = symbol.ok_or_else(|| CliError::UnknownSymbol)?;
    if symbol.is_empty() {
        return Err(CliError::UnknownSymbol.into());
    }
    let range = range.ok_or_else(|| CliError::InvalidRange)?;
    let (start, window) = get_start_and_window(range.as_str())?;
    let query = get_price_history_column_query(
        db.bucket.as_str(),
        start.as_str(),
        symbol.as_str(),
        "price",
        window.as_str(),
    );
    let db_query_rs = db
        .client
        .query_raw(db.org.as_str(), Some(Query::new(query)))
        .await?;
    let rs = ReaderBuilder::new()
        .delimiter(b',')
        .from_reader(db_query_rs.as_bytes())
        .deserialize()
        .collect::<Result<Vec<PriceColumn>, _>>()?;
    Ok(rs)
}
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct PriceStatus {
    pub symbol: String,
    pub change_rate: f64,
    pub change: i64,
    pub high_24h: i64,
    pub low_24h: i64,
    pub current_price: i64,
}
// key: symbol , value: PriceStatus
pub type DmPriceStatus = Arc<DashMap<String, PriceStatus>>;

pub fn new_price_status() -> DmPriceStatus {
    Arc::new(DashMap::new())
}

async fn update_price_status(
    mp: machine::SharedStateMap,
    dps: &DmPriceStatus,
    db: Arc<Influxdb>,
) -> anyhow::Result<()> {
    for symbol in mp.ws_state.supported_symbol.iter() {
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
            let change = p.value_last - p.value_first;
            let change_rate = com::f64_round(change as f64 / p.value_first as f64);
            let price_status = PriceStatus {
                symbol: symbol.to_string(),
                change_rate,
                change,
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
    symbols_sub_set: &DashSet<String>,
    dps: &DmPriceStatus,
    org_price: &OrgPrice,
) -> anyhow::Result<Option<WsSrvMessage>> {
    if let Some(price_status) = dps.get(&org_price.symbol) {
        let mut price_status = price_status.value().clone();
        price_status.current_price = org_price.price;
        let message = serde_json::to_string(&price_status)?;
        if symbols_sub_set.contains(&org_price.symbol) {
            return Ok(Some(WsSrvMessage::PriceUpdate(message.clone())));
        }
    }
    Ok(None)
}
pub struct PriceBroadcast {
    task: Task,
}

impl PriceBroadcast {
    pub async fn new(
        mp: machine::SharedStateMap,
        price_status: DmPriceStatus,
        db: Arc<Influxdb>,
    ) -> Self {
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let task = Task::new(
            "price broadcast",
            shutdown_tx,
            tokio::spawn(broadcast_price(mp, price_status, db.clone(), shutdown_rx)),
        );
        Self { task }
    }
    pub async fn shutdown(self) {
        self.task.shutdown().await;
    }
}

async fn broadcast_price(
    mp: machine::SharedStateMap,
    price_status: DmPriceStatus,
    db: Arc<Influxdb>,
    mut shutdown_rx: oneshot::Receiver<()>,
) -> anyhow::Result<()> {
    let mut timer = time::interval(Duration::from_secs(5));
    loop {
        tokio::select! {
            _ = (&mut shutdown_rx) => {
                info!("got shutdown signal , break price broadcast!");
                break;
            },
            _ = timer.tick() => {
                if let Err(e) = update_price_status(mp.clone(), &price_status, db.clone()).await {
                    error!("update price status error: {}", e);
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
    mp: machine::SharedStateMap,
    mut socket: WebSocket,
    address: Option<Address>,
    price_status: DmPriceStatus,
    mut price_ws_rx: PriceWatchRx,
) {
    let (tx, mut rx) = mpsc::channel::<WsSrvMessage>(10);
    if let Some(address) = address.clone() {
        mp.ws_state.add_conn(address.clone(), tx);
    }
    let symbols_set: DashSet<String> = DashSet::new();
    loop {
        tokio::select! {
            Some(msg) = rx.recv() => {
                if let Err(e) = socket.send(Message::Text(msg.into_txt())).await {
                    error!("send ws message error: {}", e);
                    break;
                }
            }
            Ok(price) = price_ws_rx.0.recv() => {
                // debug!("got price from ws broadcast channel: {:?}", price);
                if let Ok(m) = get_broadcast_price_status(&symbols_set, &price_status, &price) {
                    if let Some(m) = m {
                        if let Err(e) = socket.send(Message::Text(m.into_txt())).await {
                            error!("send ws message error: {}", e);
                            break;
                        }
                    }
                }
            }
            ws_msg = socket.recv() => {
                match ws_msg {
                    Some(Ok(msg))=>{
                        match msg {
                            Message::Text(t) => {
                                handle_ws_events(mp.clone(), t, &symbols_set);
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
    info!("client disconnected, clean connection :{:?}", address);
    if let Some(address) = address {
        mp.ws_state.remove_conn(&address);
    }
}

fn handle_ws_events(
    mp: machine::SharedStateMap,
    msg: String,
    // address: &Address,
    symbols_set: &DashSet<String>,
) {
    let sub_req: SubRequest = serde_json::from_str(msg.as_str()).unwrap();
    let symbol = sub_req.symbol;
    if !mp.ws_state.is_supported_symbol(&symbol) {
        return;
    }
    let sub_type = sub_req.sub_type;
    match sub_type {
        SubType::Subscribe => {
            // mp.ws_state.add_symbol_sub(symbol.clone(), address.copy());
            symbols_set.insert(symbol.clone());
        }
        SubType::Unsubscribe => {
            // mp.ws_state.remove_symbol_sub(&symbol, &address);
            // symbols.retain(|s| s != &symbol);
            symbols_set.remove(&symbol);
        }
        SubType::None => {}
    }
}
