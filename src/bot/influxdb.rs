use crate::config::InfluxdbConfig;
use influxdb2_client::Client;
#[derive(Clone)]
pub struct Influxdb {
    pub org: String,
    pub bucket: String,
    pub client: Client,
}
impl Influxdb {
    pub fn new(conf: InfluxdbConfig) -> Self {
        let c = Client::new(conf.url, conf.token);
        // c.write_line_protocol(org, bucket, body)
        Self {
            org: conf.org,
            bucket: conf.bucket,
            client: c,
        }
    }
}
