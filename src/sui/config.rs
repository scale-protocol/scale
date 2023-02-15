use crate::com::{self, CliError};
use crate::config::{self, Config as cfg};
use log::debug;
use std::sync::Arc;
use std::{fs, path::PathBuf, str::FromStr, time::Duration};
use sui::config::{SuiClientConfig, SuiEnv};
use sui_keys::keystore::{FileBasedKeystore, Keystore};
use sui_sdk::rpc_types::{SuiEvent, SuiTransactionResponse};
use sui_sdk::types::base_types::{ObjectID, SuiAddress, TransactionDigest};
use sui_sdk::SuiClient;
extern crate serde;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

const DEFAULT_OBJECT_ID: &str = "0x01";
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
    pub scale_coin_package_id: ObjectID,
    pub scale_coin_reserve_id: ObjectID,
    pub scale_coin_admin_id: ObjectID,
    pub scale_oracle_package_id: ObjectID,
    pub scale_oracle_admin_id: ObjectID,
    pub scale_oracle_root_id: ObjectID,
    pub scale_admin_cap_id: ObjectID,
    pub price_config: config::PriceConfig,
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
pub type Ctx = Arc<Context>;
#[derive(Clone)]
pub struct Context {
    pub config: Config,
    pub client: SuiClient,
}

impl Context {
    pub async fn new(config: Config) -> anyhow::Result<Ctx> {
        Ok(Arc::new(Context {
            config: config.clone(),
            client: config
                .get_sui_config()?
                .get_active_env()?
                .create_rpc_client(Some(Duration::from_secs(1000)))
                .await?,
        }))
    }
}
impl Default for Config {
    fn default() -> Self {
        let home_dir = config::get_home_dir();
        let scale_home_dir = config::get_or_create_config_dir(vec![".scale", ".sui"]);
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
            scale_coin_package_id: default_id,
            scale_coin_reserve_id: default_id,
            scale_coin_admin_id: default_id,
            scale_admin_cap_id: default_id,
            scale_oracle_package_id: default_id,
            scale_oracle_admin_id: default_id,
            scale_oracle_root_id: default_id,
            price_config: config::PriceConfig::default(),
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
                self.scale_admin_cap_id = c.scale_admin_cap_id;
                self.scale_coin_package_id = c.scale_coin_package_id;
                self.scale_coin_reserve_id = c.scale_coin_reserve_id;
                self.scale_coin_admin_id = c.scale_coin_admin_id;
                self.scale_oracle_package_id = c.scale_oracle_package_id;
                self.scale_oracle_admin_id = c.scale_oracle_admin_id;
                self.scale_oracle_root_id = c.scale_oracle_root_id;
                self.price_config = c.price_config;

                self.load_sui_config()?;
                if c.scale_package_id == ObjectID::from_str(DEFAULT_OBJECT_ID).unwrap() {
                    return self.init();
                }
                // debug!("load scale config success: {:?}", self);
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
scale admin id: {}
scale coin package id: {}
scale coin reserve id: {}
scale coin admin id: {}
scale oracle package id: {}
scale oracle admin id: {}
"#,
            self.sui_cli_config_file.display(),
            self.scale_config_file.display(),
            self.scale_store_path.display(),
            self.scale_package_id,
            self.scale_market_list_id,
            self.scale_nft_factory_id,
            self.scale_admin_cap_id,
            self.scale_coin_package_id,
            self.scale_coin_reserve_id,
            self.scale_coin_admin_id,
            self.scale_oracle_package_id,
            self.scale_oracle_admin_id,
        );
    }
}

impl Config {
    fn load_sui_config(&mut self) -> anyhow::Result<()> {
        let sui_config_str = fs::read_to_string(&self.sui_cli_config_file)?;
        let sui_config: SuiConfig = serde_yaml::from_str(&sui_config_str)?;
        // debug!("load sui config success: {:?}", sui_config);
        self.sui_config = sui_config;
        Ok(())
    }

    pub fn get_sui_config(&self) -> anyhow::Result<SuiClientConfig> {
        let mut sui_config = SuiClientConfig::new(Keystore::from(
            FileBasedKeystore::new(&self.sui_config.keystore.file)
                .map_err(|e| CliError::CliError(e.to_string()))?,
        ));
        sui_config.envs = self.sui_config.envs.clone();
        sui_config.active_address = self.sui_config.active_address.clone();
        sui_config.active_env = self.sui_config.active_env.clone();
        Ok(sui_config)
    }

    fn init(&mut self) -> anyhow::Result<()> {
        // get scale move package info
        let scale_package = self.get_publish_info(com::SUI_SCALE_PUBLISH_TX)?;
        self.set_value(com::SUI_SCALE_PUBLISH_TX, scale_package.effects.events);
        // get coin package info
        let coin_package = self.get_publish_info(com::SUI_COIN_PUBLISH_TX)?;
        self.set_value(com::SUI_COIN_PUBLISH_TX, coin_package.effects.events);
        // get oracle package info
        let oracle_package = self.get_publish_info(com::SUI_ORACLE_PUBLISH_TX)?;
        self.set_value(com::SUI_ORACLE_PUBLISH_TX, oracle_package.effects.events);
        self.save()?;
        Ok(())
    }

    fn set_value(&mut self, tx: &str, events: Vec<SuiEvent>) {
        debug!("get publish info: {:?}", events);
        for v in events {
            match v {
                SuiEvent::NewObject {
                    package_id: _,
                    transaction_module: _,
                    sender: _,
                    recipient: _,
                    object_type,
                    object_id,
                    version: _,
                } => {
                    if object_type.as_str().ends_with("::scale::AdminCap") {
                        self.scale_coin_admin_id = object_id;
                    }
                    if object_type.as_str().ends_with("::scale::Reserve") {
                        self.scale_coin_reserve_id = object_id;
                    }
                    if object_type.as_str().ends_with("::market::MarketList") {
                        self.scale_market_list_id = object_id;
                    }
                    if object_type.as_str().ends_with("::nft::ScaleNFTFactory") {
                        self.scale_nft_factory_id = object_id;
                    }
                    if object_type.as_str().ends_with("::admin::AdminCap") {
                        self.scale_admin_cap_id = object_id;
                    }
                    if object_type.as_str().ends_with("::oracle::AdminCap") {
                        self.scale_oracle_admin_id = object_id;
                    }
                    if object_type.as_str().ends_with("::oracle::Root") {
                        self.scale_oracle_root_id = object_id;
                    }
                }
                SuiEvent::Publish {
                    sender: _,
                    package_id,
                    version: _,
                    digest: _,
                } => {
                    if tx == com::SUI_COIN_PUBLISH_TX {
                        self.scale_coin_package_id = package_id;
                    }
                    if tx == com::SUI_SCALE_PUBLISH_TX {
                        self.scale_package_id = package_id;
                    }
                    if tx == com::SUI_ORACLE_PUBLISH_TX {
                        self.scale_oracle_package_id = package_id;
                    }
                }
                _ => {}
            }
        }
    }

    fn get_publish_info(&self, tx: &str) -> anyhow::Result<SuiTransactionResponse> {
        let sui_config = self.get_sui_config()?;
        com::new_tokio_one_thread().block_on(async {
            debug!("get move package info");
            if let Ok(active_envs) = sui_config.get_active_env() {
                let client = active_envs
                    .create_rpc_client(Some(Duration::from_secs(1000)))
                    .await;
                match client {
                    Ok(client) => {
                        let pm = TransactionDigest::from_str(tx).unwrap();
                        let rs = client
                            .read_api()
                            .get_transaction(pm)
                            .await
                            .map_err(|e| CliError::RpcError(e.to_string()))?;
                        Ok(rs)
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
