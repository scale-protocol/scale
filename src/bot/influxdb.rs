use chrono::{DateTime, FixedOffset, TimeZone, Utc};
use influxdb2::{self, Client};
use influxdb2_structmap::FromMap;
use std::sync::Arc;
#[derive(Debug, influxdb2_structmap_derive::FromMap)]
pub struct PriceData {
    feed: String,
    // conf: i64,
    price: i64,
    time: DateTime<FixedOffset>,
}

impl Default for PriceData {
    fn default() -> Self {
        let now = Utc::now().naive_utc();
        Self {
            feed: "".to_owned(),
            // conf: 0,
            price: 0,
            time: FixedOffset::east_opt(7 * 3600)
                .unwrap()
                .from_utc_datetime(&now),
        }
    }
}

pub struct InfluxdbConfig {
    pub url: String,
    pub org: String,
    pub bucket: String,
    pub token: String,
}
#[derive(Clone)]
pub struct Influxdb {
    pub bucket: String,
    pub client: Client,
}
impl Influxdb {
    pub fn new(conf: InfluxdbConfig) -> Self {
        Self {
            bucket: conf.bucket,
            client: Client::new(conf.url, conf.org, conf.token),
        }
    }
}

// pub async fn query_price_history(
//     symbol: String,
//     range: String,
//     db: Arc<Influxdb>,
// ) -> anyhow::Result<Vec<PriceData>> {
// }
