use crate::config::{self, Config as cfg};
extern crate serde;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::path::PathBuf;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub scale_config_file: PathBuf,
}
impl Default for Config {
    fn default() -> Self {
        let _home_dir = config::get_home_dir();
        let scale_home_dir = config::get_or_create_config_dir(vec![".scale", ".sui"]);
        Config {
            scale_config_file: scale_home_dir.join("aptos_config.yaml"),
        }
    }
}
impl cfg for Config {
    fn load(&mut self) -> anyhow::Result<()>
    where
        Self: DeserializeOwned,
    {
        Ok(())
    }
    fn get_config_file(&self) -> PathBuf {
        self.scale_config_file.clone()
    }
    fn set_config_file(&mut self, path: PathBuf) {
        self.scale_config_file = path;
    }
    fn get_storage_path(&self) -> PathBuf {
        PathBuf::from("")
    }
    fn get_influxdb_config(&self) -> config::InfluxdbConfig {
        config::InfluxdbConfig::default()
    }
    fn get_sql_db_config(&self) -> config::SqlDbConfig {
        config::SqlDbConfig::default()
    }
    fn print(&self) {
        println!("scale_config_file: {:?}", self.scale_config_file);
    }
}
