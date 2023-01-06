use crate::com::{self, CliError};
use crate::config::Config as cfg;
use home;
use log::debug;
use std::{fs, path::PathBuf, str::FromStr, time::Duration};
use sui::config::{SuiClientConfig, SuiEnv};
use sui_keys::keystore::{FileBasedKeystore, Keystore};
use sui_sdk::rpc_types::{SuiEvent, SuiTransactionResponse};
use sui_sdk::types::base_types::{ObjectID, SuiAddress, TransactionDigest};
use sui_sdk::SuiClient;
use tokio::runtime::Runtime;
extern crate serde;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

const DEFAULT_OBJECT_ID: &str = "0x0000000000000000000000000000000000000000";
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub sui_cli_config_file: PathBuf,
    #[serde(skip_serializing, skip_deserializing)]
    pub sui_config: SuiConfig,
    #[serde(skip_serializing, skip_deserializing)]
    pub scale_config_file: PathBuf,
    pub scale_store_path: PathBuf,
    pub scale_package_id: ObjectID,
    pub scale_market_list_id: ObjectID,
    pub scale_nft_factory_id: ObjectID,
    pub admin_cap_id: ObjectID,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SuiConfig {
    pub keystore: KeystoreFile,
    pub envs: Vec<SuiEnv>,
    pub active_env: Option<String>,
    pub active_address: Option<SuiAddress>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KeystoreFile {
    #[serde(alias = "File")]
    pub file: PathBuf,
}
#[derive(Clone)]
pub struct Context {
    pub config: Config,
    pub client: SuiClient,
}
impl Context {
    pub async fn new(config: Config) -> anyhow::Result<Self> {
        Ok(Context {
            config: config.clone(),
            client: config
                .get_sui_config()?
                .get_active_env()?
                .create_rpc_client(Some(Duration::from_secs(1000)))
                .await?,
        })
    }
}
impl Default for Config {
    fn default() -> Self {
        let home_dir = match home::home_dir() {
            Some(p) => p,
            None => PathBuf::from("/tmp/"),
        };
        let scale_home_dir = home_dir.join(".scale").join("sui");
        if !scale_home_dir.exists() {
            debug!("create default home dir: {:?}", scale_home_dir);
            fs::create_dir_all(&scale_home_dir).unwrap();
        }
        let sui_dir = home_dir.join(".sui").join("sui_config");
        let keystore_file = sui_dir.join("sui.keystore");
        let default_id = ObjectID::from_str(DEFAULT_OBJECT_ID).unwrap();
        Config {
            sui_cli_config_file: sui_dir.join("client.yaml"),
            sui_config: SuiConfig {
                keystore: KeystoreFile {
                    file: keystore_file,
                },
                envs: vec![],
                active_address: None,
                active_env: None,
            },
            scale_config_file: scale_home_dir.join("sui_config.yaml"),
            scale_store_path: scale_home_dir.join("store"),
            scale_package_id: default_id,
            scale_market_list_id: default_id,
            scale_nft_factory_id: default_id,
            admin_cap_id: default_id,
        }
    }
}

impl cfg for Config {
    fn load(&mut self) -> anyhow::Result<()>
    where
        Self: DeserializeOwned,
    {
        if !self.scale_config_file.exists() {
            self.load_sui_config()?;
            return self.init();
        }
        let config_str = fs::read_to_string(&self.scale_config_file)?;
        match serde_yaml::from_str::<Config>(&config_str) {
            Ok(c) => {
                self.sui_cli_config_file = c.sui_cli_config_file;
                self.scale_store_path = c.scale_store_path;
                self.scale_package_id = c.scale_package_id;
                self.scale_market_list_id = c.scale_market_list_id;
                self.scale_nft_factory_id = c.scale_nft_factory_id;
                self.admin_cap_id = c.admin_cap_id;
                self.load_sui_config()?;
                if c.scale_package_id == ObjectID::from_str(DEFAULT_OBJECT_ID).unwrap() {
                    return self.init();
                }
                debug!("load scale config success: {:?}", self);
            }
            Err(e) => {
                self.load_sui_config()?;
                debug!("load scale config error: {}", e);
                return self.init();
            }
        }
        Ok(())
    }
    fn get_config_file(&self) -> PathBuf {
        self.scale_config_file.clone()
    }
    fn set_config_file(&mut self, path: PathBuf) {
        self.scale_config_file = path;
    }
    fn print(&self) {
        println!(
            r#"sui cli config file: {}
scale config file: {}
scale store path: {}
scale package id: {}
scale market list id: {}
scale nft factory id: {}
scale admin id: {}"#,
            self.sui_cli_config_file.display(),
            self.scale_config_file.display(),
            self.scale_store_path.display(),
            self.scale_package_id,
            self.scale_market_list_id,
            self.scale_nft_factory_id,
            self.admin_cap_id,
        );
    }
}

impl Config {
    fn load_sui_config(&mut self) -> anyhow::Result<()> {
        let sui_config_str = fs::read_to_string(&self.sui_cli_config_file)?;
        let sui_config: SuiConfig = serde_yaml::from_str(&sui_config_str)?;
        debug!("load sui config success: {:?}", sui_config);
        self.sui_config = sui_config;
        Ok(())
    }

    pub fn get_sui_config(&self) -> anyhow::Result<SuiClientConfig> {
        let mut sui_config = SuiClientConfig::new(Keystore::from(
            FileBasedKeystore::new(&self.sui_config.keystore.file).unwrap(),
        ));
        sui_config.envs = self.sui_config.envs.clone();
        sui_config.active_address = self.sui_config.active_address.clone();
        sui_config.active_env = self.sui_config.active_env.clone();
        Ok(sui_config)
    }
    fn init(&mut self) -> anyhow::Result<()> {
        // get move package info
        let rs = self.get_publish_info()?;
        debug!("get publish info: {:?}", rs.effects.events);
        for v in rs.effects.events {
            match v {
                SuiEvent::NewObject {
                    package_id: _,
                    transaction_module,
                    sender: _,
                    recipient: _,
                    object_type: _,
                    object_id,
                    version: _,
                } => match transaction_module.as_str() {
                    "nft" => {
                        self.scale_nft_factory_id = object_id;
                    }
                    "market" => {
                        self.scale_market_list_id = object_id;
                    }
                    "admin" => {
                        self.admin_cap_id = object_id;
                    }
                    _ => {}
                },
                SuiEvent::Publish {
                    sender: _,
                    package_id,
                } => {
                    self.scale_package_id = package_id;
                }
                _ => {}
            }
        }
        self.save()?;
        Ok(())
    }

    fn get_publish_info(&self) -> anyhow::Result<SuiTransactionResponse> {
        let rt = Runtime::new().unwrap();
        let sui_config = self.get_sui_config()?;
        rt.block_on(async {
            debug!("get move package info");
            if let Ok(active_envs) = sui_config.get_active_env() {
                let client = active_envs
                    .create_rpc_client(Some(Duration::from_secs(1000)))
                    .await;
                match client {
                    Ok(client) => {
                        let pm = TransactionDigest::from_str(com::SUI_SCALE_PUBLISH_TX).unwrap();
                        client.read_api().get_transaction(pm).await
                    }
                    Err(e) => {
                        debug!("get move package info failed: {:?}", e);
                        Err(CliError::ActiveEnvNotFound.into())
                    }
                }
            } else {
                debug!("get move package info failed, active env not found");
                Err(CliError::ActiveEnvNotFound.into())
            }
        })
    }
    pub fn set_config(&mut self, args: &clap::ArgMatches) {
        let storage_path = args.get_one::<PathBuf>("storage");
        let sui_config_file = args.get_one::<PathBuf>("sui-client-config");
        if let Some(path) = storage_path {
            self.scale_store_path = path.clone();
        }
        if let Some(path) = sui_config_file {
            self.sui_cli_config_file = path.clone();
        }
    }
}
