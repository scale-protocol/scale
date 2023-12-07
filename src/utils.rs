use log::debug;
use reqwest::{Client, Url};

pub async fn get_vaa_data(
    c: &Client,
    price_server_url: String,
    feed_ids: Vec<String>,
) -> anyhow::Result<Vec<String>> {
    let url = price_server_url.as_str();
    let base_vaa_url = format!("{}/api/latest_vaas", url.trim_end_matches("/"));
    let mut params = vec![("target_chain", "default")];
    for symbol in feed_ids.iter() {
        params.push(("ids[]", symbol.as_str()));
    }
    // params.extend(feed_ids);
    let vaa_url =
        Url::parse_with_params(base_vaa_url.as_str(), params).map_err(|e| anyhow::anyhow!(e))?;
    debug!("vaa_url: {}", vaa_url.to_string());

    let vaa_data = c.get(vaa_url).send().await?.json::<Vec<String>>().await?;
    Ok(vaa_data)
}
