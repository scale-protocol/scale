use anyhow;
use log::debug;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{fs, path::PathBuf};
extern crate serde_yaml;

pub fn get_home_dir() -> PathBuf {
    match home::home_dir() {
        Some(p) => p,
        None => PathBuf::from("/tmp/"),
    }
}

pub fn get_or_create_config_dir(sub_dir: Vec<&str>) -> PathBuf {
    let mut root_dir = get_home_dir();
    for dir in sub_dir {
        root_dir = root_dir.join(dir);
    }
    if !root_dir.exists() {
        debug!("create default config dir: {:?}", root_dir);
        fs::create_dir_all(&root_dir).unwrap();
    }
    root_dir
}

pub trait Config {
    fn load(&mut self) -> anyhow::Result<()>
    where
        Self: DeserializeOwned;
    fn save(&mut self) -> anyhow::Result<()>
    where
        Self: Serialize,
    {
        let config = serde_yaml::to_string(&self)?;
        fs::write(&self.get_config_file(), config)?;
        Ok(())
    }
    fn get_config_file(&self) -> PathBuf;
    fn set_config_file(&mut self, path: PathBuf);
    fn print(&self);
}

pub fn config<C: Config>(cfg: &mut C, config_file: Option<&PathBuf>) -> anyhow::Result<()>
where
    C: DeserializeOwned,
{
    if let Some(c) = config_file {
        cfg.set_config_file(c.to_path_buf());
    }
    cfg.load()?;
    Ok(())
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceConfig {
    pub price_server_url: String,
    pub ws_url: String,
    pub db: Influxdb,
    pub worm_package: String,
    pub worm_state: String,
    pub pyth_package: String,
    pub pyth_state: String,
    pub pyth_symbol: Vec<PythSymbol>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlDbConfig {
    // mysql or postgresql
    pub db_type: String,
    pub db_url: String,
}
impl PriceConfig {
    pub fn get_feed_ids(&self) -> Vec<String> {
        let mut ids = vec![];
        for symbol in &self.pyth_symbol {
            ids.push(symbol.pyth_feed.clone());
        }
        ids
    }
    pub fn get_price_info_object_ids(&self) -> Vec<String> {
        let mut ids = vec![];
        for symbol in &self.pyth_symbol {
            ids.push(symbol.price_info_object_id.clone());
        }
        ids
    }
    pub fn get_symbols(&self) -> Vec<String> {
        let mut symbols = vec![];
        for symbol in &self.pyth_symbol {
            symbols.push(symbol.symbol.clone());
        }
        symbols
    }
}
impl Default for PriceConfig {
    fn default() -> Self {
        let price_server_url = "https://xc-testnet.pyth.network".to_string();
        Self {
            price_server_url,
            ws_url: "wss://xc-testnet.pyth.network/ws".to_string(),
            db: Influxdb::default(),
            worm_package: "0x0".to_string(),
            worm_state: "0x0".to_string(),
            pyth_package: "0x0".to_string(),
            pyth_state: "0x0".to_string(),
            pyth_symbol: vec![PythSymbol::default()],
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Influxdb {
    pub url: String,
    pub org: String,
    pub bucket: String,
    pub token: String,
}
impl Default for Influxdb {
    fn default() -> Self {
        Self {
            url: "http://127.0.0.1:8086".to_string(),
            org: "scale".to_string(),
            bucket: "pyth.network".to_string(),
            token: "some token".to_string(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PythSymbol {
    pub symbol: String,
    pub pyth_feed: String,
    pub price_info_object_id: String,
}

impl Default for PythSymbol {
    fn default() -> Self {
        Self {
            symbol: "Crypto.BTC/USD".to_string(),
            pyth_feed: "0x0".to_string(),
            price_info_object_id: "0x0".to_string(),
        }
    }
}
