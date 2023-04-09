use crate::bot::state::{Address, MoveCall};
use crate::com;
use crate::{
    bot::state::DENOMINATOR,
    com::CliError,
    sui::config::{Config, Context, Ctx},
};
use async_trait::async_trait;
use chrono::Utc;
// use log::*;
use serde_json::{json, Value as JsonValue};
use shared_crypto::intent::Intent;
use std::str::FromStr;
use sui_json_rpc_types::SuiTypeTag;
use sui_keys::keystore::AccountKeystore;
use sui_sdk::{
    json::SuiJsonValue,
    rpc_types::SuiTransactionBlockResponseOptions,
    types::{
        base_types::ObjectID,
        messages::{Command, Transaction, TransactionData, TransactionKind},
    },
};
use sui_types::messages::ExecuteTransactionRequestType;

const COIN_PACKAGE_NAME: &str = "scale";
const SCALE_PACKAGE_NAME: &str = "enter";
const SCALE_NFT_PACKAGE_NAME: &str = "nft";
const SCALE_ORACLE_NAME: &str = "oracle";
pub struct Tool {
    ctx: Ctx,
    gas_budget: u64,
}

impl Tool {
    pub async fn new(conf: Config, gas_budget: u64) -> anyhow::Result<Self> {
        let ctx = Context::new(conf).await?;
        Ok(Self { ctx, gas_budget })
    }

    pub fn get_t(&self) -> SuiTypeTag {
        SuiTypeTag::from(
            sui_types::parse_sui_type_tag(
                format!("{}::scale::SCALE", self.ctx.config.scale_coin_package_id).as_str(),
            )
            .expect("cannot patransaction_datae SuiTypeTag"),
        )
    }

    pub fn get_p(&self) -> SuiTypeTag {
        SuiTypeTag::from(
            sui_types::parse_sui_type_tag(
                format!("{}::pool::Scale", self.ctx.config.scale_package_id).as_str(),
            )
            .expect("cannot parse SuiTypeTag"),
        )
    }

    async fn exec(&self, pm: TransactionData) -> anyhow::Result<()> {
        let address =
            self.ctx.config.sui_config.active_address.ok_or_else(|| {
                CliError::InvalidCliParams("active address not found".to_string())
            })?;
        let signature = self.ctx.config.get_sui_config()?.keystore.sign_secure(
            &address,
            &pm,
            Intent::sui_transaction(),
        )?;
        let opt = SuiTransactionBlockResponseOptions::default();
        let tx = self
            .ctx
            .client
            .quorum_driver()
            .execute_transaction_block(
                Transaction::from_data(pm.clone(), Intent::sui_transaction(), vec![signature])
                    .verify()?,
                opt,
                Some(ExecuteTransactionRequestType::WaitForLocalExecution),
            )
            .await?;
        let TransactionData::V1(v) = pm;
        if let TransactionKind::ProgrammableTransaction(s) = v.kind {
            s.commands.into_iter().for_each(|c| match c {
                Command::MoveCall(m) => {
                    println!("call: {}::{}::{}", m.package, m.module, m.function);
                }
                _ => {}
            });
        }
        println!("exec: {:?} , error: {:?}", tx.digest.to_string(), tx.errors);
        Ok(())
    }

    async fn get_transaction_data(
        &self,
        package: ObjectID,
        module: &str,
        function: &str,
        call_args: Vec<SuiJsonValue>,
        type_args: Vec<SuiTypeTag>,
    ) -> anyhow::Result<TransactionData> {
        self.ctx
            .client
            .transaction_builder()
            .move_call(
                self.ctx.config.sui_config.active_address.ok_or_else(|| {
                    CliError::InvalidCliParams("active address not found".to_string())
                })?,
                package,
                module,
                function,
                type_args,
                call_args,
                None,
                self.gas_budget,
            )
            .await
    }

    pub async fn coin_set(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let ratio = args
            .get_one::<u64>("ratio")
            .ok_or_else(|| CliError::InvalidCliParams("ratio".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_coin_package_id,
                COIN_PACKAGE_NAME,
                "set_subscription_ratio",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_coin_admin_id),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_coin_reserve_id),
                    SuiJsonValue::new(json!(ratio.to_string()))?,
                ],
                vec![],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn coin_burn(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let coins = args
            .get_many::<String>("coins")
            .ok_or_else(|| CliError::InvalidCliParams("coins".to_string()))?
            .map(|c| json!(c))
            .collect::<Vec<JsonValue>>();
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_coin_package_id,
                COIN_PACKAGE_NAME,
                "burn",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_coin_reserve_id),
                    SuiJsonValue::new(json!(coins))?,
                ],
                vec![],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn coin_airdrop(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let coins = args
            .get_many::<String>("coins")
            .ok_or_else(|| CliError::InvalidCliParams("coins".to_string()))?
            .map(|c| json!(c))
            .collect::<Vec<JsonValue>>();
        let amount = args
            .get_one::<u64>("amount")
            .ok_or_else(|| CliError::InvalidCliParams("amount".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_coin_package_id,
                COIN_PACKAGE_NAME,
                "airdrop",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_coin_reserve_id),
                    SuiJsonValue::new(json!(coins))?,
                    SuiJsonValue::new(json!(amount.to_string()))?,
                ],
                vec![],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn create_price_feed(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let symbol = args
            .get_one::<String>("symbol")
            .ok_or_else(|| CliError::InvalidCliParams("symbol".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_oracle_package_id,
                SCALE_ORACLE_NAME,
                "create_price_feed",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_oracle_admin_id),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_oracle_root_id),
                    SuiJsonValue::new(json!(symbol))?,
                ],
                vec![],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn update_owner(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let feed = args
            .get_one::<String>("feed")
            .ok_or_else(|| CliError::InvalidCliParams("feed".to_string()))?;
        let owner = args
            .get_one::<String>("owner")
            .ok_or_else(|| CliError::InvalidCliParams("owner".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_oracle_package_id,
                SCALE_ORACLE_NAME,
                "update_owner",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_oracle_admin_id),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_oracle_root_id),
                    SuiJsonValue::from_object_id(ObjectID::from_str(&feed)?),
                    SuiJsonValue::from_object_id(ObjectID::from_str(&owner)?),
                ],
                vec![],
            )
            .await?;
        self.exec(transaction_data).await
    }

    async fn update_price_inner(&self, feed: &str, price: u64) -> anyhow::Result<()> {
        let timestamp = Utc::now().timestamp();
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_oracle_package_id,
                SCALE_ORACLE_NAME,
                "update_price",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_oracle_root_id),
                    SuiJsonValue::from_object_id(ObjectID::from_str(feed)?),
                    SuiJsonValue::new(json!(price.to_string()))?,
                    SuiJsonValue::new(json!(timestamp.to_string()))?,
                ],
                vec![],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn update_price(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let feed = args
            .get_one::<String>("feed")
            .ok_or_else(|| CliError::InvalidCliParams("feed".to_string()))?;
        let price = args
            .get_one::<u64>("price")
            .ok_or_else(|| CliError::InvalidCliParams("price".to_string()))?;
        self.update_price_inner(feed.as_str(), *price).await
    }

    pub async fn create_account(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let coin = args
            .get_one::<String>("coin")
            .ok_or_else(|| CliError::InvalidCliParams("coin".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_PACKAGE_NAME,
                "create_account",
                vec![SuiJsonValue::new(json!(coin))?],
                vec![self.get_t()],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn deposit(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let account = args
            .get_one::<String>("account")
            .ok_or_else(|| CliError::InvalidCliParams("account".to_string()))?;
        let coins = args
            .get_many::<String>("coins")
            .ok_or_else(|| CliError::InvalidCliParams("coins".to_string()))?
            .map(|c| json!(c))
            .collect::<Vec<JsonValue>>();
        let amount = args
            .get_one::<u64>("amount")
            .ok_or_else(|| CliError::InvalidCliParams("amount".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_PACKAGE_NAME,
                "deposit",
                vec![
                    SuiJsonValue::from_object_id(ObjectID::from_str(account.as_str())?),
                    SuiJsonValue::new(json!(coins))?,
                    SuiJsonValue::new(json!(amount.to_string()))?,
                ],
                vec![self.get_t()],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn withdrawal(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let account = args
            .get_one::<String>("account")
            .ok_or_else(|| CliError::InvalidCliParams("account".to_string()))?;
        let amount = args
            .get_one::<u64>("amount")
            .ok_or_else(|| CliError::InvalidCliParams("amount".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_PACKAGE_NAME,
                "withdrawal",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_market_list_id),
                    SuiJsonValue::from_object_id(ObjectID::from_str(account.as_str())?),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_oracle_root_id),
                    SuiJsonValue::new(json!(amount.to_string()))?,
                ],
                vec![self.get_p(), self.get_t()],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn mint(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let name = args
            .get_one::<String>("name")
            .ok_or_else(|| CliError::InvalidCliParams("name".to_string()))?;
        let description = args
            .get_one::<String>("description")
            .ok_or_else(|| CliError::InvalidCliParams("description".to_string()))?;
        let img_url = args
            .get_one::<String>("img_url")
            .ok_or_else(|| CliError::InvalidCliParams("img_url".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_NFT_PACKAGE_NAME,
                "mint",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_admin_cap_id),
                    SuiJsonValue::new(json!(name.as_bytes()))?,
                    SuiJsonValue::new(json!(description.as_bytes()))?,
                    SuiJsonValue::new(json!(img_url.as_bytes()))?,
                ],
                vec![],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn mint_recipient(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let name = args
            .get_one::<String>("name")
            .ok_or_else(|| CliError::InvalidCliParams("name".to_string()))?;
        let description = args
            .get_one::<String>("description")
            .ok_or_else(|| CliError::InvalidCliParams("description".to_string()))?;
        let img_url = args
            .get_one::<String>("img_url")
            .ok_or_else(|| CliError::InvalidCliParams("img_url".to_string()))?;
        let recipient = args
            .get_one::<String>("recipient")
            .ok_or_else(|| CliError::InvalidCliParams("recipient".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_NFT_PACKAGE_NAME,
                "mint_recipient",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_admin_cap_id),
                    SuiJsonValue::new(json!(name.as_bytes()))?,
                    SuiJsonValue::new(json!(description.as_bytes()))?,
                    SuiJsonValue::new(json!(img_url.as_bytes()))?,
                    SuiJsonValue::new(json!(recipient.as_bytes()))?,
                ],
                vec![],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn mint_multiple_recipient(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let name = args
            .get_one::<String>("name")
            .ok_or_else(|| CliError::InvalidCliParams("name".to_string()))?;
        let description = args
            .get_one::<String>("description")
            .ok_or_else(|| CliError::InvalidCliParams("description".to_string()))?;
        let img_url = args
            .get_one::<String>("img_url")
            .ok_or_else(|| CliError::InvalidCliParams("img_url".to_string()))?;
        let amount = args
            .get_one::<u64>("amount")
            .ok_or_else(|| CliError::InvalidCliParams("amount".to_string()))?;
        let recipient = args
            .get_one::<String>("recipient")
            .ok_or_else(|| CliError::InvalidCliParams("recipient".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_NFT_PACKAGE_NAME,
                "mint_multiple_recipient",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_admin_cap_id),
                    SuiJsonValue::new(json!(name.as_bytes()))?,
                    SuiJsonValue::new(json!(description.as_bytes()))?,
                    SuiJsonValue::new(json!(img_url.as_bytes()))?,
                    SuiJsonValue::new(json!(amount.to_string()))?,
                    SuiJsonValue::new(json!(recipient.as_bytes()))?,
                ],
                vec![],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn mint_multiple(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let name = args
            .get_one::<String>("name")
            .ok_or_else(|| CliError::InvalidCliParams("name".to_string()))?;
        let description = args
            .get_one::<String>("description")
            .ok_or_else(|| CliError::InvalidCliParams("description".to_string()))?;
        let img_url = args
            .get_one::<String>("img_url")
            .ok_or_else(|| CliError::InvalidCliParams("img_url".to_string()))?;
        let amount = args
            .get_one::<u64>("amount")
            .ok_or_else(|| CliError::InvalidCliParams("amount".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_NFT_PACKAGE_NAME,
                "mint_multiple",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_admin_cap_id),
                    SuiJsonValue::new(json!(name.as_bytes()))?,
                    SuiJsonValue::new(json!(description.as_bytes()))?,
                    SuiJsonValue::new(json!(img_url.as_bytes()))?,
                    SuiJsonValue::new(json!(amount.to_string()))?,
                ],
                vec![],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn add_admin_member(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let admin = args
            .get_one::<String>("admin")
            .ok_or_else(|| CliError::InvalidCliParams("admin".to_string()))?;
        let member = args
            .get_one::<String>("member")
            .ok_or_else(|| CliError::InvalidCliParams("member".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_PACKAGE_NAME,
                "add_admin_member",
                vec![
                    SuiJsonValue::from_object_id(ObjectID::from_str(admin.as_str())?),
                    SuiJsonValue::from_object_id(ObjectID::from_str(member.as_str())?),
                ],
                vec![],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn remove_admin_member(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let admin = args
            .get_one::<String>("admin")
            .ok_or_else(|| CliError::InvalidCliParams("admin".to_string()))?;
        let member = args
            .get_one::<String>("member")
            .ok_or_else(|| CliError::InvalidCliParams("member".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_PACKAGE_NAME,
                "remove_admin_member",
                vec![
                    SuiJsonValue::from_object_id(ObjectID::from_str(admin.as_str())?),
                    SuiJsonValue::from_object_id(ObjectID::from_str(member.as_str())?),
                ],
                vec![],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn create_market(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let coin = args
            .get_one::<String>("coin")
            .ok_or_else(|| CliError::InvalidCliParams("coin".to_string()))?;
        let symbol = args
            .get_one::<String>("symbol")
            .ok_or_else(|| CliError::InvalidCliParams("symbol".to_string()))?;
        let icon = args
            .get_one::<String>("icon")
            .ok_or_else(|| CliError::InvalidCliParams("icon".to_string()))?;
        let description = args
            .get_one::<String>("description")
            .ok_or_else(|| CliError::InvalidCliParams("description".to_string()))?;
        let size = args
            .get_one::<u64>("size")
            .ok_or_else(|| CliError::InvalidCliParams("size".to_string()))?;
        let opening_price = args
            .get_one::<u64>("opening_price")
            .ok_or_else(|| CliError::InvalidCliParams("opening_price".to_string()))?;
        let pyth_id = args
            .get_one::<String>("pyth_id")
            .ok_or_else(|| CliError::InvalidCliParams("description".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_PACKAGE_NAME,
                "create_market",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_market_list_id),
                    SuiJsonValue::from_object_id(ObjectID::from_str(coin.as_str())?),
                    SuiJsonValue::new(json!(symbol.as_bytes()))?,
                    SuiJsonValue::new(json!(icon.as_bytes()))?,
                    SuiJsonValue::new(json!(description.as_bytes()))?,
                    SuiJsonValue::new(json!(size.to_string()))?,
                    SuiJsonValue::new(json!(opening_price.to_string()))?,
                    SuiJsonValue::from_object_id(ObjectID::from_str(pyth_id.as_str())?),
                ],
                vec![self.get_t()],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn update_max_leverage(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let admin = args
            .get_one::<String>("admin")
            .ok_or_else(|| CliError::InvalidCliParams("admin".to_string()))?;

        let market = args
            .get_one::<String>("market")
            .ok_or_else(|| CliError::InvalidCliParams("market".to_string()))?;
        let max_leverage = args
            .get_one::<u8>("max_leverage")
            .ok_or_else(|| CliError::InvalidCliParams("max_leverage".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_PACKAGE_NAME,
                "update_max_leverage",
                vec![
                    SuiJsonValue::from_object_id(ObjectID::from_str(admin.as_str())?),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_market_list_id),
                    SuiJsonValue::from_object_id(ObjectID::from_str(market.as_str())?),
                    SuiJsonValue::new(json!(max_leverage))?,
                ],
                vec![self.get_p(), self.get_t()],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn update_insurance_fee(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let admin = args
            .get_one::<String>("admin")
            .ok_or_else(|| CliError::InvalidCliParams("admin".to_string()))?;

        let market = args
            .get_one::<String>("market")
            .ok_or_else(|| CliError::InvalidCliParams("market".to_string()))?;
        let insurance_fee = args
            .get_one::<f64>("insurance_fee")
            .ok_or_else(|| CliError::InvalidCliParams("insurance_fee".to_string()))?;
        let insurance_fee = (insurance_fee * DENOMINATOR as f64) as u64;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_PACKAGE_NAME,
                "update_insurance_fee",
                vec![
                    SuiJsonValue::from_object_id(ObjectID::from_str(admin.as_str())?),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_market_list_id),
                    SuiJsonValue::from_object_id(ObjectID::from_str(market.as_str())?),
                    SuiJsonValue::new(json!(insurance_fee.to_string()))?,
                ],
                vec![self.get_p(), self.get_t()],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn update_margin_fee(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let admin = args
            .get_one::<String>("admin")
            .ok_or_else(|| CliError::InvalidCliParams("admin".to_string()))?;

        let market = args
            .get_one::<String>("market")
            .ok_or_else(|| CliError::InvalidCliParams("market".to_string()))?;
        let margin_fee = args
            .get_one::<f64>("margin_fee")
            .ok_or_else(|| CliError::InvalidCliParams("margin_fee".to_string()))?;
        let margin_fee = (margin_fee * DENOMINATOR as f64) as u64;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_PACKAGE_NAME,
                "update_margin_fee",
                vec![
                    SuiJsonValue::from_object_id(ObjectID::from_str(admin.as_str())?),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_market_list_id),
                    SuiJsonValue::from_object_id(ObjectID::from_str(market.as_str())?),
                    SuiJsonValue::new(json!(margin_fee.to_string()))?,
                ],
                vec![self.get_p(), self.get_t()],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn update_fund_fee(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let admin = args
            .get_one::<String>("admin")
            .ok_or_else(|| CliError::InvalidCliParams("admin".to_string()))?;

        let market = args
            .get_one::<String>("market")
            .ok_or_else(|| CliError::InvalidCliParams("market".to_string()))?;
        let fund_fee = args
            .get_one::<f64>("fund_fee")
            .ok_or_else(|| CliError::InvalidCliParams("fund_fee".to_string()))?;
        let fund_fee = (fund_fee * DENOMINATOR as f64) as u64;
        let manual = args
            .get_one::<bool>("manual")
            .ok_or_else(|| CliError::InvalidCliParams("manual".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_PACKAGE_NAME,
                "update_fund_fee",
                vec![
                    SuiJsonValue::from_object_id(ObjectID::from_str(admin.as_str())?),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_market_list_id),
                    SuiJsonValue::from_object_id(ObjectID::from_str(market.as_str())?),
                    SuiJsonValue::new(json!(fund_fee.to_string()))?,
                    SuiJsonValue::new(json!(manual))?,
                ],
                vec![self.get_p(), self.get_t()],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn update_status(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let admin = args
            .get_one::<String>("admin")
            .ok_or_else(|| CliError::InvalidCliParams("admin".to_string()))?;

        let market = args
            .get_one::<String>("market")
            .ok_or_else(|| CliError::InvalidCliParams("market".to_string()))?;
        let status = args
            .get_one::<u8>("status")
            .ok_or_else(|| CliError::InvalidCliParams("status".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_PACKAGE_NAME,
                "update_status",
                vec![
                    SuiJsonValue::from_object_id(ObjectID::from_str(admin.as_str())?),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_market_list_id),
                    SuiJsonValue::from_object_id(ObjectID::from_str(market.as_str())?),
                    SuiJsonValue::new(json!(status))?,
                ],
                vec![self.get_p(), self.get_t()],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn update_description(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let admin = args
            .get_one::<String>("admin")
            .ok_or_else(|| CliError::InvalidCliParams("admin".to_string()))?;

        let market = args
            .get_one::<String>("market")
            .ok_or_else(|| CliError::InvalidCliParams("market".to_string()))?;
        let description = args
            .get_one::<String>("description")
            .ok_or_else(|| CliError::InvalidCliParams("description".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_PACKAGE_NAME,
                "update_description",
                vec![
                    SuiJsonValue::from_object_id(ObjectID::from_str(admin.as_str())?),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_market_list_id),
                    SuiJsonValue::from_object_id(ObjectID::from_str(market.as_str())?),
                    SuiJsonValue::new(json!(description.as_bytes()))?,
                ],
                vec![self.get_p(), self.get_t()],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn update_spread_fee(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let admin = args
            .get_one::<String>("admin")
            .ok_or_else(|| CliError::InvalidCliParams("admin".to_string()))?;

        let market = args
            .get_one::<String>("market")
            .ok_or_else(|| CliError::InvalidCliParams("market".to_string()))?;
        let spread_fee = args
            .get_one::<f64>("spread_fee")
            .ok_or_else(|| CliError::InvalidCliParams("spread_fee".to_string()))?;
        let spread_fee = (spread_fee * DENOMINATOR as f64) as u64;
        let manual = args
            .get_one::<bool>("manual")
            .ok_or_else(|| CliError::InvalidCliParams("manual".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_PACKAGE_NAME,
                "update_spread_fee",
                vec![
                    SuiJsonValue::from_object_id(ObjectID::from_str(admin.as_str())?),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_market_list_id),
                    SuiJsonValue::from_object_id(ObjectID::from_str(market.as_str())?),
                    SuiJsonValue::new(json!(spread_fee.to_string()))?,
                    SuiJsonValue::new(json!(manual))?,
                ],
                vec![self.get_p(), self.get_t()],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn update_officer(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let market = args
            .get_one::<String>("market")
            .ok_or_else(|| CliError::InvalidCliParams("market".to_string()))?;
        let officer = args
            .get_one::<u8>("officer")
            .ok_or_else(|| CliError::InvalidCliParams("officer".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_PACKAGE_NAME,
                "update_officer",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_admin_cap_id),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_market_list_id),
                    SuiJsonValue::from_object_id(ObjectID::from_str(market.as_str())?),
                    SuiJsonValue::new(json!(officer))?,
                ],
                vec![self.get_p(), self.get_t()],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn add_factory_mould(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let name = args
            .get_one::<String>("name")
            .ok_or_else(|| CliError::InvalidCliParams("name".to_string()))?;
        let description = args
            .get_one::<String>("description")
            .ok_or_else(|| CliError::InvalidCliParams("description".to_string()))?;
        let url = args
            .get_one::<String>("url")
            .ok_or_else(|| CliError::InvalidCliParams("url".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_PACKAGE_NAME,
                "add_factory_mould",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_admin_cap_id),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_nft_factory_id),
                    SuiJsonValue::new(json!(name.as_bytes()))?,
                    SuiJsonValue::new(json!(description.as_bytes()))?,
                    SuiJsonValue::new(json!(url.as_bytes()))?,
                ],
                vec![],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn remove_factory_mould(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let name = args
            .get_one::<String>("name")
            .ok_or_else(|| CliError::InvalidCliParams("name".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_PACKAGE_NAME,
                "remove_factory_mould",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_admin_cap_id),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_nft_factory_id),
                    SuiJsonValue::new(json!(name.as_bytes()))?,
                ],
                vec![],
            )
            .await?;
        self.exec(transaction_data).await
    }
    pub async fn investment(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let market = args
            .get_one::<String>("market")
            .ok_or_else(|| CliError::InvalidCliParams("market".to_string()))?;
        let coins = args
            .get_many::<String>("coins")
            .ok_or_else(|| CliError::InvalidCliParams("coin".to_string()))?
            .map(|c| json!(c))
            .collect::<Vec<JsonValue>>();
        // let coins = coins.map(|c| c.as_str()).collect::<Vec<&str>>();
        let name = args
            .get_one::<String>("name")
            .ok_or_else(|| CliError::InvalidCliParams("name".to_string()))?;
        let amount = args
            .get_one::<u64>("amount")
            .ok_or_else(|| CliError::InvalidCliParams("amount".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_PACKAGE_NAME,
                "investment",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_market_list_id),
                    SuiJsonValue::from_object_id(ObjectID::from_str(market.as_str())?),
                    SuiJsonValue::new(json!(coins))?,
                    SuiJsonValue::from_object_id(self.ctx.config.scale_nft_factory_id),
                    SuiJsonValue::new(json!(name.as_bytes()))?,
                    SuiJsonValue::new(json!(amount.to_string()))?,
                ],
                vec![self.get_p(), self.get_t()],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn divestment(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let market = args
            .get_one::<String>("market")
            .ok_or_else(|| CliError::InvalidCliParams("market".to_string()))?;
        let nft = args
            .get_one::<String>("nft")
            .ok_or_else(|| CliError::InvalidCliParams("nft".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_PACKAGE_NAME,
                "divestment",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_market_list_id),
                    SuiJsonValue::from_object_id(ObjectID::from_str(market.as_str())?),
                    SuiJsonValue::from_object_id(ObjectID::from_str(nft.as_str())?),
                ],
                vec![self.get_p(), self.get_t()],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn trigger_update_opening_price(
        &self,
        args: &clap::ArgMatches,
    ) -> anyhow::Result<()> {
        let market = args
            .get_one::<String>("market")
            .ok_or_else(|| CliError::InvalidCliParams("market".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_PACKAGE_NAME,
                "trigger_update_opening_price",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_market_list_id),
                    SuiJsonValue::from_object_id(ObjectID::from_str(market.as_str())?),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_oracle_root_id),
                ],
                vec![self.get_p(), self.get_t()],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn generate_upgrade_move_token(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let market = args
            .get_one::<String>("market")
            .ok_or_else(|| CliError::InvalidCliParams("market".to_string()))?;
        let nft = args
            .get_one::<String>("nft")
            .ok_or_else(|| CliError::InvalidCliParams("nft".to_string()))?;
        let address = args
            .get_one::<String>("address")
            .ok_or_else(|| CliError::InvalidCliParams("address".to_string()))?;
        let expiration_time = args
            .get_one::<String>("expiration_time")
            .ok_or_else(|| CliError::InvalidCliParams("expiration_time".to_string()))?;
        let t = chrono::DateTime::parse_from_rfc3339(expiration_time.as_str())?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_PACKAGE_NAME,
                "generate_upgrade_move_token",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_admin_cap_id),
                    SuiJsonValue::from_object_id(ObjectID::from_str(nft.as_str())?),
                    SuiJsonValue::from_object_id(ObjectID::from_str(market.as_str())?),
                    SuiJsonValue::new(json!(t.timestamp().to_string()))?,
                    SuiJsonValue::from_object_id(ObjectID::from_str(address.as_str())?),
                ],
                vec![self.get_p(), self.get_t()],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn divestment_by_upgrade(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let market = args
            .get_one::<String>("market")
            .ok_or_else(|| CliError::InvalidCliParams("market".to_string()))?;
        let nft = args
            .get_one::<String>("nft")
            .ok_or_else(|| CliError::InvalidCliParams("nft".to_string()))?;
        let move_token = args
            .get_one::<String>("move_token")
            .ok_or_else(|| CliError::InvalidCliParams("move_token".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_PACKAGE_NAME,
                "divestment_by_upgrade",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_market_list_id),
                    SuiJsonValue::from_object_id(ObjectID::from_str(market.as_str())?),
                    SuiJsonValue::from_object_id(ObjectID::from_str(nft.as_str())?),
                    SuiJsonValue::from_object_id(ObjectID::from_str(move_token.as_str())?),
                ],
                vec![self.get_p(), self.get_t()],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn open_position(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let market = args
            .get_one::<String>("market")
            .ok_or_else(|| CliError::InvalidCliParams("market".to_string()))?;
        let account = args
            .get_one::<String>("account")
            .ok_or_else(|| CliError::InvalidCliParams("account".to_string()))?;
        let lot = args
            .get_one::<f64>("lot")
            .ok_or_else(|| CliError::InvalidCliParams("lot".to_string()))?;
        let lot = (lot * com::DENOMINATOR as f64) as u64;
        let leverage = args
            .get_one::<u8>("leverage")
            .ok_or_else(|| CliError::InvalidCliParams("leverage".to_string()))?;
        let position_type = args
            .get_one::<u8>("position_type")
            .ok_or_else(|| CliError::InvalidCliParams("position_type".to_string()))?;
        let direction = args
            .get_one::<u8>("direction")
            .ok_or_else(|| CliError::InvalidCliParams("direction".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_PACKAGE_NAME,
                "open_position",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_market_list_id),
                    SuiJsonValue::from_object_id(ObjectID::from_str(market.as_str())?),
                    SuiJsonValue::from_object_id(ObjectID::from_str(account.as_str())?),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_oracle_root_id),
                    SuiJsonValue::new(json!(lot.to_string()))?,
                    SuiJsonValue::new(json!(leverage))?,
                    SuiJsonValue::new(json!(position_type))?,
                    SuiJsonValue::new(json!(direction))?,
                ],
                vec![self.get_p(), self.get_t()],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn close_position(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let market = args
            .get_one::<String>("market")
            .ok_or_else(|| CliError::InvalidCliParams("market".to_string()))?;
        let account = args
            .get_one::<String>("account")
            .ok_or_else(|| CliError::InvalidCliParams("account".to_string()))?;
        let position = args
            .get_one::<String>("position")
            .ok_or_else(|| CliError::InvalidCliParams("position".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_PACKAGE_NAME,
                "close_position",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_market_list_id),
                    SuiJsonValue::from_object_id(ObjectID::from_str(market.as_str())?),
                    SuiJsonValue::from_object_id(ObjectID::from_str(account.as_str())?),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_oracle_root_id),
                    SuiJsonValue::from_object_id(ObjectID::from_str(position.as_str())?),
                ],
                vec![self.get_p(), self.get_t()],
            )
            .await?;
        self.exec(transaction_data).await
    }
}

#[async_trait]
impl MoveCall for Tool {
    async fn trigger_update_opening_price(&self, market_id: Address) -> anyhow::Result<()> {
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_PACKAGE_NAME,
                "trigger_update_opening_price",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_market_list_id),
                    SuiJsonValue::from_object_id(ObjectID::from_bytes(
                        market_id.to_vec().as_slice(),
                    )?),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_oracle_root_id),
                ],
                vec![self.get_p(), self.get_t()],
            )
            .await?;
        self.exec(transaction_data).await
    }

    async fn burst_position(
        &self,
        account_id: Address,
        position_id: Address,
    ) -> anyhow::Result<()> {
        let _transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_PACKAGE_NAME,
                "burst_position",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_market_list_id),
                    SuiJsonValue::from_object_id(ObjectID::from_bytes(
                        account_id.to_vec().as_slice(),
                    )?),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_oracle_root_id),
                    SuiJsonValue::from_object_id(ObjectID::from_bytes(
                        position_id.to_vec().as_slice(),
                    )?),
                ],
                vec![self.get_p(), self.get_t()],
            )
            .await?;
        return Ok(());

        // self.exec(transaction_data).await
    }

    async fn process_fund_fee(&self, account_id: Address) -> anyhow::Result<()> {
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_PACKAGE_NAME,
                "process_fund_fee",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_market_list_id),
                    SuiJsonValue::from_object_id(ObjectID::from_bytes(
                        account_id.to_vec().as_slice(),
                    )?),
                ],
                vec![self.get_t()],
            )
            .await?;
        self.exec(transaction_data).await
    }

    async fn update_price(&self, feed: &str, price: u64) -> anyhow::Result<()> {
        self.update_price_inner(feed, price).await
    }
}
