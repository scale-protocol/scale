// sub pyth.network price.
// see https://docs.pyth.network/pythnet-price-feeds/best-practices
// see ids: https://pyth.network/developers/price-feed-ids
use crate::bot::machine::Message;
use crate::bot::state::{Address, OrgPrice, State, Status};
use crate::bot::ws_client::{WsClient, WsMessage};
use crate::com::{CliError, DECIMALS};
use chrono::{DateTime, NaiveDateTime, Utc};
use dashmap::DashMap;
use influxdb::InfluxDbWriteable;
use influxdb::{Client, Query, ReadQuery, Timestamp};
use log::*;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::mpsc::{self, UnboundedSender};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    #[serde(rename = "type")]
    pub type_field: SubType,
    pub ids: Vec<String>,
    pub verbose: bool,
    pub binary: bool,
}
#[derive(Debug, Clone, PartialEq)]
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

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    #[serde(rename = "type")]
    pub type_field: String,
    #[serde(rename = "price_feed")]
    pub price_feed: PriceFeed,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceFeed {
    #[serde(rename = "ema_price")]
    pub ema_price: EmaPrice,
    pub id: String,
    pub metadata: Metadata,
    pub price: Price,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmaPrice {
    pub conf: String,
    pub expo: i64,
    pub price: String,
    #[serde(rename = "publish_time")]
    pub publish_time: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    #[serde(rename = "emitter_chain")]
    pub emitter_chain: i64,
    #[serde(rename = "attestation_time")]
    pub attestation_time: i64,
    #[serde(rename = "sequence_number")]
    pub sequence_number: i64,
    #[serde(rename = "price_service_receive_time")]
    pub price_service_receive_time: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Price {
    pub conf: String,
    pub expo: i64,
    pub price: String,
    #[serde(rename = "publish_time")]
    pub publish_time: i64,
}
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, InfluxDbWriteable)]
#[serde(rename_all = "camelCase")]
pub struct PriceData {
    pub price: u64,
    pub conf: i64,
    pub time: DateTime<Utc>,
}
impl From<Price> for PriceData {
    fn from(price: Price) -> Self {
        let conf: i64 = price.conf.parse().unwrap();
        // let t = NaiveDateTime::from_timestamp_opt(price.publish_time, 0).unwrap();
        let dt = DateTime::<Utc>::from_utc(
            NaiveDateTime::from_timestamp_opt(price.publish_time, 0).unwrap(),
            Utc,
        );
        Self {
            price: price.get_real_price(),
            conf,
            time: dt,
        }
    }
}
impl Price {
    pub fn get_real_price(&self) -> u64 {
        let price: u64 = self.price.parse().unwrap();
        price * DECIMALS / 10u64.pow(self.expo.abs() as u32) as u64
    }
}

// like Crypto.BTC/USD 0xf9c0172ba10dfa4d19088d94f5bf61d3b54d5bd7483a322a982e1373ee8ea31b
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct SymbolId {
    pub symbol: String,
    pub id: String,
}
// key is id, value is symbol
type DmSymbolId = DashMap<String, String>;
pub async fn sub_price(
    watch_tx: UnboundedSender<Message>,
    price_url: String,
    influxdb_url: String,
    influxdb_db: String,
    symbol_id_vec: Vec<SymbolId>,
) -> anyhow::Result<WsClient> {
    debug!("start sub price url: {:?}", price_url);
    let influxdb_client = Client::new(influxdb_url, influxdb_db);
    let mut sub_req = Request {
        type_field: SubType::Subscribe,
        ids: vec![],
        verbose: true,
        binary: false,
    };
    let sm_mp = Arc::new(DmSymbolId::new());
    for symbol_id in symbol_id_vec {
        let id = symbol_id.id.as_str();
        let id_key = id.strip_prefix("0x").unwrap_or(id).to_string();
        sm_mp.insert(id_key.clone(), symbol_id.symbol.clone());
        sub_req.ids.push(id_key);
    }
    let mut ws_client = WsClient::new(price_url, move |msg, _send_tx| {
        let sm_mp = sm_mp.clone();
        let watch_tx = watch_tx.clone();
        let influxdb_client = influxdb_client.clone();
        Box::pin(async move {
            if let WsMessage::Txt(txt) = msg {
                let resp: Response = serde_json::from_str(&txt)?;
                let symbol_str = sm_mp
                    .get(&resp.price_feed.id)
                    .ok_or_else(|| CliError::UnknownSymbol)?;

                let watch_msg = Message {
                    address: Address::from_str(resp.price_feed.id.as_str())?,
                    state: State::Price(OrgPrice {
                        price: resp.price_feed.price.get_real_price(),
                        update_time: resp.price_feed.price.publish_time,
                        symbol: symbol_str.to_string(),
                    }),
                    status: Status::Normal,
                };
                debug!("......sub price resp: {:?}", watch_msg);
                if let Err(e) = watch_tx.send(watch_msg) {
                    error!("send watch msg error: {:?}", e);
                }
                let db_price_data: PriceData = resp.price_feed.price.into();
                let db_rs = influxdb_client
                    .query(db_price_data.into_query(symbol_str.to_string()))
                    .await?;
                debug!("write price to db success: {:?}", db_rs);
            }
            Ok(())
        })
    })
    .await?;

    let req = serde_json::to_string(&sub_req)?;
    debug!("......sub price req: {:?}", req);
    ws_client.send(WsMessage::Txt(req)).await?;
    Ok(ws_client)
}
