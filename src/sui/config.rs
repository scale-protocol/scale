use crate::config::Config as cfg;
use home;
use log::debug;
use std::path::PathBuf;
use std::{fs, path::PathBuf, str::FromStr};
use sui::config::SuiClientConfig;
use sui_keys::keystore::{AccountKeystore, FileBasedKeystore, Keystore};
use sui_sdk::{
    types::base_types::{ObjectID, SuiAddress},
    SuiClient,
};
extern crate serde;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub sui_cli_config_file: PathBuf,
    #[serde(skip_serializing)]
    pub sui_config: SuiClientConfig,
    #[serde(skip_serializing)]
    pub scale_config_file: PathBuf,
    pub store_path: PathBuf,
    pub scale_package_address: SuiAddress,
    pub market_list_address: ObjectID,
    pub scale_nft_factory_address: ObjectID,
}

impl Default for Config {
    fn default() -> Self {
        let home_dir = match home::home_dir() {
            Some(p) => p,
            None => PathBuf::from("/tmp/"),
        };
        let scale_home_dir = home_dir.join(".scale");
        if !scale_home_dir.is_dir() {
            fs::create_dir(&scale_home_dir).unwrap();
        }
        Config {
            sui_cli_config_file: home_dir.join(".sui").join("sui_config").join("client.yaml"),
            sui_config: SuiClientConfig::default(),
            scale_config_file: scale_home_dir.join("sui_config.yaml"),
            store_path: scale_home_dir.join("store"),
            scale_package_address: SuiAddress::from_str("0x0").unwrap(),
            market_list_address: ObjectID::from_str("0x0").unwrap(),
            scale_nft_factory_address: ObjectID::from_str("0x0").unwrap(),
        }
    }
}

impl cfg for Config {
    fn init(&mut self) -> anyhow::Result<()> {
        self.load_sui_config()?;
        // get move package info

        Ok(())
    }
    fn load(&mut self) -> anyhow::Result<()> {
        let config = fs::read_to_string(&self.scale_config_file)?;
        *self = serde_yaml::from_str(&config)?;
        self.load_sui_config()?;
        Ok(())
    }
    fn get_config_file(&self) -> PathBuf {
        self.scale_config_file.clone()
    }
    fn set_config_file(&mut self, path: PathBuf) {
        self.scale_config_file = path;
    }
    fn print(&self) {
        println!("{:?}", self)
    }
}
impl Config {
    fn load_sui_config(&mut self) -> anyhow::Result<()> {
        let sui_config = fs::read_to_string(&self.sui_cli_config_file)?;
        self.sui_config = serde_yaml::from_str(&sui_config)?;
        Ok(())
    }
}
