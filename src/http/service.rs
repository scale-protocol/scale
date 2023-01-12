use crate::bot::{
    self,
    influxdb::Influxdb,
    machine::{AccountDynamicData, PositionDynamicData},
    state::{Account, Address, Position, State},
};
use crate::bot::{machine, storage};
use crate::com::{self, CliError};
use csv::ReaderBuilder;
use influxdb2_client::models::Query;
use log::*;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountInfo {
    pub account_data: Account,
    pub address: Address,
    pub dynamic_data: Option<AccountDynamicData>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionInfo {
    pub position_data: Position,
    pub address: Address,
    pub dynamic_data: Option<PositionDynamicData>,
}

pub fn get_account_info(
    address: String,
    mp: bot::machine::SharedStateMap,
) -> anyhow::Result<Option<AccountInfo>> {
    let address = Address::from_str(address.as_str())
        .map_err(|e| CliError::HttpServerError(e.to_string()))?;
    let rs = match mp.account.get(&address) {
        Some(user) => {
            let data = match mp.account_dynamic_idx.get(&address) {
                Some(d) => {
                    let mut dynamic_data = machine::AccountDynamicData::default();
                    dynamic_data.margin_percentage = com::f64_round(d.value().margin_percentage);
                    dynamic_data.profit_rate = com::f64_round(d.value().profit_rate);
                    Some(dynamic_data)
                }
                None => None,
            };
            let user_account = (*user.value()).clone();
            let user_info = AccountInfo {
                account_data: user_account,
                dynamic_data: data,
                address,
            };
            Some(user_info)
        }
        None => None,
    };
    Ok(rs)
}

pub fn get_position_list(
    mp: machine::SharedStateMap,
    prefix: String,
    address: String,
) -> anyhow::Result<Vec<PositionInfo>> {
    let address = Address::from_str(address.as_str())
        .map_err(|e| CliError::HttpServerError(e.to_string()))?;
    let prefix = storage::Prefix::from_str(prefix.as_str())?;
    let mut rs: Vec<PositionInfo> = Vec::new();
    match prefix {
        storage::Prefix::Active => {
            let r = mp.position.get(&address);
            match r {
                Some(p) => {
                    for v in p.value() {
                        let p = (*v.value()).clone();
                        let data = mp.position_dynamic_idx.get(v.key()).map(|d| {
                            let mut dynamic_data = machine::PositionDynamicData::default();
                            dynamic_data.profit_rate = com::f64_round(d.value().profit_rate);
                            dynamic_data
                        });
                        rs.push(PositionInfo {
                            position_data: p,
                            address: v.key().copy(),
                            dynamic_data: data,
                        });
                    }
                }
                None => {}
            }
        }
        storage::Prefix::History => {
            let items = mp.storage.get_position_history_list(&address);
            for i in items {
                match i {
                    Ok((k, v)) => {
                        let key = String::from_utf8(k.to_vec())
                            .map_err(|e| CliError::JsonError(e.to_string()))?;
                        let keys = storage::Keys::from_str(key.as_str())?;
                        let pk = keys.get_end();
                        let pbk = Address::from_str(pk.as_str())
                            .map_err(|e| CliError::Unknown(e.to_string()))?;
                        let values: State = serde_json::from_slice(v.to_vec().as_slice())
                            .map_err(|e| CliError::JsonError(e.to_string()))?;
                        let data = mp.position_dynamic_idx.get(&pbk).map(|d| {
                            let mut dynamic_data = machine::PositionDynamicData::default();
                            dynamic_data.profit_rate = com::f64_round(d.value().profit_rate);
                            dynamic_data
                        });
                        match values {
                            State::Position(p) => {
                                rs.push(PositionInfo {
                                    position_data: p,
                                    address: pbk,
                                    dynamic_data: data,
                                });
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
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceStatus {
    pub change_rate: i64,
    pub change: i64,
    pub high_24h: i64,
    pub low_24h: i64,
    pub current_price: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Price {
    #[serde(rename(deserialize = "_value", serialize = "value"))]
    value: i64,
    #[serde(rename(deserialize = "_start", serialize = "time"))]
    time: String,
}
#[derive(Debug, Serialize, Deserialize)]
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
        "1H" => Ok(("-1h".to_string(), "5s".to_string())),
        "1D" => Ok(("-1d".to_string(), "1m".to_string())),
        "1W" => Ok(("-1w".to_string(), "1h".to_string())),
        "1M" => Ok(("-1m".to_string(), "1h".to_string())),
        "1Y" => Ok(("-1y".to_string(), "1h".to_string())),
        _ => Err(CliError::InvalidRange.into()),
    }
}
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
