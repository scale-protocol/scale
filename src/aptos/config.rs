use crate::config::Config as cfg;
extern crate serde;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{fs, path::PathBuf};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub scale_config_file: PathBuf,
}
impl Default for Config {
    fn default() -> Self {
        let home_dir = match home::home_dir() {
            Some(p) => p,
            None => PathBuf::from("/tmp/"),
        };
        let scale_home_dir = home_dir.join(".scale").join("aptos");
        if !scale_home_dir.is_dir() {
            fs::create_dir(&scale_home_dir).unwrap();
        }
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
