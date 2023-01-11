use crate::bot::{
    self,
    influxdb::{Influxdb, PriceData},
    machine::{AccountDynamicData, PositionDynamicData},
    state::{Account, Address, Position, State},
};
use crate::bot::{machine, storage};
use crate::com::{self, CliError};
use chrono::{DateTime, FixedOffset};
use log::*;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use std::str::FromStr;
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
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceColumn {
    pub open: i64,
    pub close: i64,
    pub high: i64,
    pub low: i64,
    pub time: DateTime<FixedOffset>,
}
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Price {
    pub price: i64,
    pub time: DateTime<FixedOffset>,
}
fn get_filter(bucket: &str, symbol: &str) -> String {
    format!(
        r#"from(bucket: "{}")
        |> range(start: -1d)
        |> filter(fn: (r) => r["_measurement"] == "{}")
        |> filter(fn: (r) => r["feed"] == "price")
        |> filter(fn: (r) => r["_field"] == "price")
        "#,
        bucket, symbol
    )
}
pub async fn get_price_history(symbol: String, range: String, db: Arc<Influxdb>) {
    let range = match range.as_str() {
        "1H" => "-1h",
        "1D" => "-1d",
        "1W" => "-1w",
        "1M" => "-1mo",
        "1Y" => "-1y",
        _ => "",
    };
}
pub async fn get_price_history_column(symbol: String, range: String, db: Arc<Influxdb>) {
    let range = match range.as_str() {
        "1H" => "1h",
        "1D" => "1d",
        "1W" => "7d",
        "1M" => "30d",
        "1Y" => "365d",
        _ => "",
    };
}
