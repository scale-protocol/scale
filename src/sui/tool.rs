use crate::{
    com::CliError,
    sui::config::{Config, Context, Ctx},
};
use serde_json::{json, Number, Value};
use sui_adapter::execution_mode::Normal;
use sui_sdk::json::SuiJsonValue;
const COIN_PACKAGE_NAME: &str = "scale";
pub struct Tool {
    ctx: Ctx,
}

impl Tool {
    pub async fn new(conf: Config) -> anyhow::Result<Self> {
        let ctx = Context::new(conf).await?;
        Ok(Self { ctx })
    }
    pub async fn coin_set(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let ratio = args
            .get_one::<u64>("ratio")
            .ok_or_else(|| CliError::InvalidCliParams("ratio".to_string()))?;
        let rs = self
            .ctx
            .client
            .transaction_builder()
            .move_call::<Normal>(
                self.ctx.config.sui_config.active_address.ok_or_else(|| {
                    CliError::InvalidCliParams("active address not found".to_string())
                })?,
                self.ctx.config.scale_coin_package_id,
                COIN_PACKAGE_NAME,
                "set",
                vec![],
                vec![SuiJsonValue::new(json!(*ratio))?],
                None,
                10000,
            )
            .await?;
        println!("{:?}", rs);
        Ok(())
    }
    pub async fn coin_burn(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        Ok(())
    }
    pub async fn coin_airdrop(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        Ok(())
    }
}
