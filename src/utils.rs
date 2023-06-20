use crate::config::PriceConfig;
use log::debug;
use reqwest::{Client, Url};
pub async fn get_vaa_data(c: &Client, pc: &PriceConfig) -> anyhow::Result<Vec<String>> {
    let base_vaa_url = format!("{}/api/latest_vaas", pc.price_server_url);
    let mut params = vec![("target_chain", "default")];
    for symbol in &pc.pyth_symbol {
        params.push(("ids[]", symbol.pyth_feed.as_str()));
    }
    let vaa_url =
        Url::parse_with_params(base_vaa_url.as_str(), params).map_err(|e| anyhow::anyhow!(e))?;
    debug!("vaa_url: {}", vaa_url.to_string());

    let vaa_data = c.get(vaa_url).send().await?.json::<Vec<String>>().await?;
    Ok(vaa_data)
}
