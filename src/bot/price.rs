// sub pyth.network price.
// see https://docs.pyth.network/pythnet-price-feeds/best-practices
// see ids: https://pyth.network/developers/price-feed-ids
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    #[serde(rename = "type")]
    pub type_field: String,
    pub ids: Vec<String>,
    pub verbose: bool,
    pub binary: bool,
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
