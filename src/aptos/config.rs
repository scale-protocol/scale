use crate::config::{self, Config as cfg};
extern crate serde;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{fs, path::PathBuf};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub scale_config_file: PathBuf,
}
impl Default for Config {
    fn default() -> Self {
        let home_dir = config::get_home_dir();
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
        // let config = fs::read_to_string(&self.scale_config_file)?;
        // let config: Config = serde_yaml::from_str(&config)?;
        Ok(())
    }
    fn get_config_file(&self) -> PathBuf {
        self.scale_config_file.clone()
    }
    fn set_config_file(&mut self, path: PathBuf) {
        self.scale_config_file = path;
    }
    fn print(&self) {
        println!("scale_config_file: {:?}", self.scale_config_file);
    }
}
