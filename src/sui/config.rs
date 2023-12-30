use crate::com::{self, ClientError};
use crate::config::{self, Config as cfg};
use log::debug;
use std::sync::Arc;
use std::{fs, path::PathBuf, str::FromStr, time::Duration};
use sui_sdk::rpc_types::{
    ObjectChange, SuiTransactionBlockResponse, SuiTransactionBlockResponseOptions,
};
use sui_sdk::types::base_types::{ObjectID, TransactionDigest};
use sui_sdk::{wallet_context::WalletContext, SuiClient};
use sui_types::base_types::SuiAddress;
extern crate serde;
use move_core_types::language_storage::TypeTag;
use reqwest::Client as HttpClient;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

const DEFAULT_OBJECT_ID: &str = "0x01";
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub sui_cli_config_file: PathBuf,
    #[serde(skip_serializing, skip_deserializing)]
    pub scale_config_file: PathBuf,
    pub scale_store_path: PathBuf,
    pub scale_package_id: ObjectID,
    pub scale_market_list_id: ObjectID,
    pub scale_bot_id: ObjectID,
    pub scale_bond_factory_id: ObjectID,
    pub scale_publisher_id: ObjectID,
    pub scale_coin_package_id: ObjectID,
    pub scale_coin_reserve_id: ObjectID,
    pub scale_coin_admin_id: ObjectID,
    pub scale_oracle_package_id: ObjectID,
    pub scale_oracle_admin_id: ObjectID,
    pub scale_oracle_state_id: ObjectID,
    pub scale_oracle_pyth_state_id: ObjectID,
    pub scale_admin_cap_id: ObjectID,
    pub scale_nft_package_id: ObjectID,
    pub scale_nft_admin_id: ObjectID,
    pub price_config: config::PriceConfig,
    pub sql_db_config: config::SqlDbConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KeystoreFile {
    #[serde(alias = "File")]
    pub file: PathBuf,
}
pub type Ctx = Arc<Context>;
// #[derive(Clone)]
pub struct Context {
    pub config: Config,
    pub client: SuiClient,
    pub wallet: WalletContext,
    pub http_client: HttpClient,
}

impl Context {
    pub async fn new(config: Config) -> anyhow::Result<Ctx> {
        let wallet = WalletContext::new(
            &config.sui_cli_config_file,
            Some(Duration::from_secs(10)),
            None,
        )
        .await?;
        Ok(Arc::new(Context {
            config: config.clone(),
            client: wallet.get_client().await?,
            wallet,
            http_client: HttpClient::new(),
        }))
    }
    pub fn get_active_address(&self) -> anyhow::Result<SuiAddress> {
        Ok(self
            .wallet
            .config
            .active_address
            .ok_or_else(|| ClientError::NoActiveAccount("no active account".to_string()))?)
    }

    pub fn get_feed_ids(&self) -> anyhow::Result<Vec<ObjectID>> {
        let mut ids = Vec::new();
        for i in self.config.price_config.get_feed_ids(None) {
            ids.push(ObjectID::from_str(i.as_str())?);
        }
        Ok(ids)
    }
    // pub fn get_price_info_object_ids(&self) -> anyhow::Result<Vec<ObjectID>> {
    //     let mut ids = Vec::new();
    //     for i in self.config.price_config.get_price_info_object_ids() {
    //         ids.push(ObjectID::from_str(i.as_str())?);
    //     }
    //     Ok(ids)
    // }
    pub fn get_worm_package_id(&self) -> anyhow::Result<ObjectID> {
        Ok(ObjectID::from_str(
            self.config.price_config.worm_package.as_str(),
        )?)
    }
    pub fn get_worm_vaa_type(&self) -> anyhow::Result<TypeTag> {
        let t = format!(
            "{}::vaa::VAA",
            self.config.price_config.worm_package.as_str()
        );
        Ok(TypeTag::from_str(t.as_str())?)
    }
    pub fn get_price_info_type(&self) -> anyhow::Result<TypeTag> {
        let t = format!(
            "{}::price_info::PriceInfo",
            self.config.price_config.pyth_package.as_str()
        );
        Ok(TypeTag::from_str(t.as_str())?)
    }
    pub fn get_worm_state_id(&self) -> anyhow::Result<ObjectID> {
        Ok(ObjectID::from_str(
            self.config.price_config.worm_state.as_str(),
        )?)
    }
    pub fn get_pyth_package_id(&self) -> anyhow::Result<ObjectID> {
        Ok(ObjectID::from_str(
            self.config.price_config.pyth_package.as_str(),
        )?)
    }
    pub fn get_pyth_state_id(&self) -> anyhow::Result<ObjectID> {
        Ok(ObjectID::from_str(
            self.config.price_config.pyth_state.as_str(),
        )?)
    }
}
impl Default for Config {
    fn default() -> Self {
        let home_dir = config::get_home_dir();
        let scale_home_dir = config::get_or_create_config_dir(vec![".scale", ".sui"]);
        let sui_dir = home_dir.join(".sui").join("sui_config");
        // let keystore_file = sui_dir.join("sui.keystore");
        let default_id = ObjectID::from_str(DEFAULT_OBJECT_ID).unwrap();
        Config {
            sui_cli_config_file: sui_dir.join("client.yaml"),
            scale_config_file: scale_home_dir.join("sui_config.yaml"),
            scale_store_path: scale_home_dir.join("store"),
            scale_package_id: default_id,
            scale_market_list_id: default_id,
            scale_bot_id: default_id,
            scale_bond_factory_id: default_id,
            scale_publisher_id: default_id,
            scale_coin_package_id: default_id,
            scale_coin_reserve_id: default_id,
            scale_coin_admin_id: default_id,
            scale_admin_cap_id: default_id,
            scale_oracle_package_id: default_id,
            scale_oracle_admin_id: default_id,
            scale_oracle_state_id: default_id,
            scale_oracle_pyth_state_id: default_id,
            scale_nft_package_id: default_id,
            scale_nft_admin_id: default_id,
            price_config: config::PriceConfig::default(),
            sql_db_config: config::SqlDbConfig::default(),
        }
    }
}

impl cfg for Config {
    fn load(&mut self) -> anyhow::Result<()>
    where
        Self: DeserializeOwned,
    {
        let config_str = fs::read_to_string(&self.scale_config_file)?;
        debug!("read config from local config file: {}", config_str);
        match serde_yaml::from_str::<Config>(&config_str) {
            Ok(c) => {
                self.sui_cli_config_file = c.sui_cli_config_file;
                self.scale_store_path = c.scale_store_path;
                self.scale_package_id = c.scale_package_id;
                self.scale_market_list_id = c.scale_market_list_id;
                self.scale_bond_factory_id = c.scale_bond_factory_id;
                self.scale_publisher_id = c.scale_publisher_id;
                self.scale_admin_cap_id = c.scale_admin_cap_id;
                self.scale_coin_package_id = c.scale_coin_package_id;
                self.scale_coin_reserve_id = c.scale_coin_reserve_id;
                self.scale_coin_admin_id = c.scale_coin_admin_id;
                self.scale_oracle_package_id = c.scale_oracle_package_id;
                self.scale_oracle_admin_id = c.scale_oracle_admin_id;
                self.scale_oracle_state_id = c.scale_oracle_state_id;
                self.scale_oracle_pyth_state_id = c.scale_oracle_pyth_state_id;
                self.scale_nft_package_id = c.scale_nft_package_id;
                self.scale_nft_admin_id = c.scale_nft_admin_id;
                self.price_config = c.price_config;

                // if c.scale_package_id == ObjectID::from_str(DEFAULT_OBJECT_ID).unwrap() {
                //     return self.init();
                // }
            }
            Err(e) => {
                debug!("load scale config error: {}", e);
                return Err(com::ClientError::ConfigError(e.to_string()).into());
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
    fn get_storage_path(&self) -> PathBuf {
        self.scale_store_path.clone()
    }
    fn get_influxdb_config(&self) -> config::InfluxdbConfig {
        self.price_config.db.clone()
    }
    fn get_sql_db_config(&self) -> config::SqlDbConfig {
        self.sql_db_config.clone()
    }
    fn get_price_config(&self) -> config::PriceConfig {
        self.price_config.clone()
    }
    fn get(&mut self) {
        if !self.scale_config_file.exists() {
            if let Err(e) = self.init() {
                println!("init config error: {}", e);
                return;
            }
        }
        println!(
            r#"sui cli config file: {}
scale config file: {}
scale store path: {}
scale package id: {}
scale market list id: {}
scale bot id: {}
scale bond factory id: {}
scale publisher id: {}
scale coin package id: {}
scale coin reserve id: {}
scale coin admin id: {}
scale admin cap id: {}
scale oracle package id: {}
scale oracle admin id: {}
scale oracle state id: {}
scale oracle pyth state id: {}
scale nft package id: {}
scale nft admin id: {}
"#,
            self.sui_cli_config_file.display(),
            self.scale_config_file.display(),
            self.scale_store_path.display(),
            self.scale_package_id,
            self.scale_market_list_id,
            self.scale_bot_id,
            self.scale_bond_factory_id,
            self.scale_publisher_id,
            self.scale_coin_package_id,
            self.scale_coin_reserve_id,
            self.scale_coin_admin_id,
            self.scale_admin_cap_id,
            self.scale_oracle_package_id,
            self.scale_oracle_admin_id,
            self.scale_oracle_state_id,
            self.scale_oracle_pyth_state_id,
            self.scale_nft_package_id,
            self.scale_nft_admin_id,
        );
    }
}

impl Config {
    fn init(&mut self) -> anyhow::Result<()> {
        com::new_tokio_one_thread().block_on(async {
            let wallet = WalletContext::new(
                &self.sui_cli_config_file,
                Some(Duration::from_secs(10)),
                None,
            )
            .await;
            if let Ok(wallet) = wallet {
                if let Ok(client) = wallet.get_client().await {
                    // get coin package info
                    if let Ok(coin_package) = self
                        .get_publish_info(&client, com::SUI_COIN_PUBLISH_TX)
                        .await
                    {
                        self.set_value(com::SUI_COIN_PUBLISH_TX, coin_package.object_changes);
                    } else {
                        println!("please init coin package");
                        return;
                    }
                    // get oracle package info
                    if let Ok(oracle_package) = self
                        .get_publish_info(&client, com::SUI_ORACLE_PUBLISH_TX)
                        .await
                    {
                        self.set_value(com::SUI_ORACLE_PUBLISH_TX, oracle_package.object_changes);
                    } else {
                        println!("please init oracle package");
                        return;
                    }
                    // get nft package info
                    if let Ok(nft_package) = self
                        .get_publish_info(&client, com::SUI_NFT_PUBLISH_TX)
                        .await
                    {
                        self.set_value(com::SUI_NFT_PUBLISH_TX, nft_package.object_changes);
                    } else {
                        println!("please init nft package");
                        return;
                    }
                    // get scale move package info
                    if let Ok(scale_package) = self
                        .get_publish_info(&client, com::SUI_SCALE_PUBLISH_TX)
                        .await
                    {
                        self.set_value(com::SUI_SCALE_PUBLISH_TX, scale_package.object_changes);
                    } else {
                        println!("please init scale package");
                        return;
                    }
                } else {
                }
            } else {
                println!("please init wallet first");
                return;
            }
        });
        self.save()?;
        Ok(())
    }

    fn set_value(&mut self, tx: &str, change: Option<Vec<ObjectChange>>) {
        debug!("get publish changes info: {:?}", change);
        if let Some(change) = change {
            for v in change {
                match v {
                    ObjectChange::Created {
                        sender: _,
                        owner: _,
                        object_type,
                        object_id,
                        version: _,
                        digest: _,
                    } => {
                        debug!("object type: {:?}", object_type);
                        if object_type.module.as_str() == "scale" {
                            if object_type.name.as_str() == "AdminCap" {
                                self.scale_coin_admin_id = object_id;
                            }
                            if object_type.name.as_str() == "Reserve" {
                                self.scale_coin_reserve_id = object_id;
                            }
                        }
                        if object_type.module.as_str() == "bond"
                            && object_type.name.as_str() == "BondFactory"
                        {
                            self.scale_bond_factory_id = object_id;
                        }
                        if object_type.module.as_str() == "bot"
                            && object_type.name.as_str() == "ScaleBot"
                        {
                            self.scale_bot_id = object_id;
                        }
                        if object_type.module.as_str() == "admin"
                            && object_type.name.as_str() == "AdminCap"
                        {
                            self.scale_admin_cap_id = object_id;
                        }
                        if object_type.module.as_str() == "oracle" {
                            if object_type.name.as_str() == "AdminCap" {
                                self.scale_oracle_admin_id = object_id;
                            }
                            if object_type.name.as_str() == "State" {
                                self.scale_oracle_state_id = object_id;
                            }
                        }
                        if object_type.module.as_str() == "pyth_network" {
                            if object_type.name.as_str() == "State" {
                                self.scale_oracle_pyth_state_id = object_id;
                            }
                        }
                        if object_type.module.as_str() == "nft" {
                            if object_type.name.as_str() == "AdminCap" {
                                self.scale_nft_admin_id = object_id;
                            }
                        }
                        if object_type.module.as_str() == "package" {
                            if object_type.name.as_str() == "Publisher" {
                                self.scale_publisher_id = object_id;
                            }
                        }
                    }
                    ObjectChange::Published {
                        package_id,
                        version: _,
                        digest: _,
                        modules: _,
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
                        if tx == com::SUI_NFT_PUBLISH_TX {
                            self.scale_nft_package_id = package_id;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    async fn get_publish_info(
        &self,
        client: &SuiClient,
        tx: &str,
    ) -> anyhow::Result<SuiTransactionBlockResponse> {
        let td = TransactionDigest::from_str(tx)?;
        let opt = SuiTransactionBlockResponseOptions {
            show_input: false,
            show_raw_input: false,
            show_effects: false,
            show_events: false,
            show_object_changes: true,
            show_balance_changes: false,
        };
        let rs = client
            .read_api()
            .get_transaction_with_options(td, opt)
            .await
            .map_err(|e| ClientError::RpcError(e.to_string()))?;
        Ok(rs)
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
