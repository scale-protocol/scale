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
    pub ws_url: String,
    pub db: Influxdb,
    pub pyth_symbol: Vec<PythSymbol>,
}

impl Default for PriceConfig {
    fn default() -> Self {
        Self {
            ws_url: "wss://xc-testnet.pyth.network/ws".to_string(),
            db: Influxdb::default(),
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
    pub id: String,
    pub oracle_feed_address: Option<String>,
}

impl Default for PythSymbol {
    fn default() -> Self {
        Self {
            symbol: "Crypto.BTC/USD".to_string(),
            id: "0xf9c0172ba10dfa4d19088d94f5bf61d3b54d5bd7483a322a982e1373ee8ea31b".to_string(),
            oracle_feed_address: None,
        }
    }
}
