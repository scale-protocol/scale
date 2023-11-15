// sub pyth.network price.
// see https://docs.pyth.network/pythnet-price-feeds/best-practices
// see ids: https://pyth.network/developers/price-feed-ids
use crate::bot::influxdb::Influxdb;
use crate::bot::machine::Message;
use crate::bot::state::{Address, Event, OrgPrice, State};
use crate::bot::ws::{PriceWatchRx, SharedDmSymbolId, SubType, WsClient, WsClientMessage};
use crate::com::{CliError, DECIMALS};
use futures::prelude::*;
use influxdb2_client::api::write::Precision;
use influxdb2_client::models::DataPoint;
use log::*;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tokio::sync::{broadcast, mpsc::UnboundedSender};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    #[serde(rename = "type")]
    pub type_field: SubType,
    pub ids: Vec<String>,
    pub verbose: bool,
    pub binary: bool,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
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

impl PriceFeed {
    fn get_data_points(&self, measurement: String) -> anyhow::Result<Vec<DataPoint>> {
        // let now = chrono::Utc::now();
        // let ts = Utc.timestamp_millis_opt(111).unwrap();
        let r = vec![
            DataPoint::builder(measurement.clone())
                .field("price", self.price.get_real_price())
                .field("conf", self.price.conf.parse::<i64>().unwrap())
                .tag("feed", "price")
                .timestamp(self.price.publish_time)
                .build()?,
            DataPoint::builder(measurement)
                .field("price", self.ema_price.get_real_price())
                .field("conf", self.ema_price.conf.parse::<i64>().unwrap())
                .tag("feed", "ema_price")
                .timestamp(self.ema_price.publish_time)
                .build()?,
        ];
        Ok(r)
    }
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
    // #[serde(rename = "emitter_chain")]
    // pub emitter_chain: i64,
    // #[serde(rename = "attestation_time")]
    // pub attestation_time: i64,
    // #[serde(rename = "sequence_number")]
    // pub sequence_number: i64,
    // #[serde(rename = "price_service_receive_time")]
    // pub price_service_receive_time: i64,
    pub slot: i64,
    #[serde(rename = "emitter_chain")]
    pub emitter_chain: i64,
    #[serde(rename = "price_service_receive_time")]
    pub price_service_receive_time: i64,
    #[serde(rename = "prev_publish_time")]
    pub prev_publish_time: i64,
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

impl Price {
    pub fn get_real_price(&self) -> i64 {
        let price: i64 = self.price.parse().unwrap();
        price * (DECIMALS as i64) / 10u64.pow(self.expo.abs() as u32) as i64
    }
}
impl EmaPrice {
    pub fn get_real_price(&self) -> i64 {
        let price: i64 = self.price.parse().unwrap();
        price * (DECIMALS as i64) / 10u64.pow(self.expo.abs() as u32) as i64
    }
}

pub async fn sub_price(
    watch_tx: UnboundedSender<Message>,
    price_ws_url: String,
    inf_db: Influxdb,
    sds: SharedDmSymbolId,
    enable_db: bool,
    is_broadcast_price: bool,
) -> anyhow::Result<(WsClient, PriceWatchRx)> {
    debug!("start sub price url: {:?}", price_ws_url);
    let mut sub_req = Request {
        type_field: SubType::Subscribe,
        ids: vec![],
        verbose: true,
        binary: false,
    };
    for id in sds.iter() {
        sub_req.ids.push(id.key().to_string());
    }
    let (ws_price_tx, ws_price_rx) = broadcast::channel::<OrgPrice>(sds.len() * 5);

    let req = serde_json::to_string(&sub_req)?;

    let ws_client = WsClient::new(
        price_ws_url,
        Some(WsClientMessage::Txt(req)),
        move |msg, _send_tx| {
            let sds = sds.clone();
            let watch_tx = watch_tx.clone();
            let influxdb_client = inf_db.client.clone();
            let org = inf_db.org.clone();
            let bucket = inf_db.bucket.clone();
            let ws_price_tx = ws_price_tx.clone();
            Box::pin(async move {
                if let WsClientMessage::Txt(txt) = msg {
                    // debug!("price txt: {:?}", txt);
                    // let resp: Response = serde_json::from_str(&txt)?;
                    let resp: Response = match serde_json::from_str(&txt) {
                        Ok(resp) => resp,
                        Err(e) => {
                            error!("parse price resp error: {:?}", e);
                            return Ok(());
                        }
                    };
                    debug!("price resp: {:?}", resp);
                    let symbol_str = sds
                        .get(&resp.price_feed.id)
                        .ok_or_else(|| CliError::UnknownSymbol)?;
                    let op = OrgPrice {
                        price: resp.price_feed.price.get_real_price(),
                        update_time: resp.price_feed.price.publish_time,
                        symbol: symbol_str.to_string(),
                    };
                    let watch_msg = Message {
                        address: Address::from_str(resp.price_feed.id.as_str())?,
                        state: State::Price(op.clone()),
                        event: Event::Created,
                    };
                    if let Err(e) = watch_tx.send(watch_msg) {
                        error!("send watch msg error: {:?}", e);
                    }
                    if is_broadcast_price {
                        debug!("broadcast ws price: {:?}", op);
                        if let Err(e) = ws_price_tx.send(op) {
                            error!("send ws price msg error: {:?}", e);
                        }
                    }
                    debug!("enable_db: {:?}", enable_db);
                    if !enable_db {
                        return Ok(());
                    }
                    let _db_rs = influxdb_client
                        .write(
                            org.as_str(),
                            bucket.as_str(),
                            Precision::Seconds,
                            stream::iter(resp.price_feed.get_data_points(symbol_str.to_string())?),
                        )
                        .await;

                    // debug!(
                    //     "write price to db success! {:?}",
                    //     resp.price_feed.get_data_points(symbol_str.to_string())?
                    // );
                    debug!("......write price resp.....: {:?}", _db_rs);
                }
                Ok(())
            })
        },
    )
    .await?;
    Ok((ws_client, PriceWatchRx(ws_price_rx)))
}
