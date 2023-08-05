use crate::{
    bot::state::DENOMINATOR,
    bot::state::{Address, MoveCall},
    com,
    com::CliError,
    sui::{
        config::{Config, Context, Ctx},
        object::{self, ObjectParams},
    },
    utils,
};
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use log::*;
use move_core_types::{ident_str, identifier::IdentStr, language_storage::StructTag};
use serde_json::{json, Value as JsonValue};
use shared_crypto::intent::Intent;
use std::str::FromStr;
use sui_json_rpc_types::{
    MoveCallParams, RPCTransactionRequestParams, SuiExecutionStatus, SuiObjectData,
    SuiObjectDataFilter, SuiObjectDataOptions, SuiObjectResponse, SuiObjectResponseQuery,
    SuiTransactionBlockEffects, SuiTypeTag,
};
use sui_keys::keystore::AccountKeystore;
use sui_sdk::{
    json::SuiJsonValue,
    rpc_types::SuiTransactionBlockResponseOptions,
    types::{base_types::ObjectID, transaction::Transaction},
};
use sui_types::{
    coin::{self, Coin},
    crypto::Signature,
    programmable_transaction_builder::ProgrammableTransactionBuilder as PTB,
    quorum_driver_types::ExecuteTransactionRequestType,
    transaction::{Argument, CallArg, Command, ObjectArg, TransactionData, TransactionKind},
    Identifier, TypeTag, SUI_CLOCK_OBJECT_ID, SUI_FRAMEWORK_PACKAGE_ID,
};

const COIN_MODULE_NAME: &str = "scale";
const SCALE_MODULE_NAME: &str = "enter";
const NFT_MODULE_NAME: &str = "nft";
const ORACLE_MODULE_NAME: &str = "oracle";
const ORACLE_PYTH_MODULE_NAME: &str = "pyth_network";
const USDT_TYPE_TAG: &str = "xxx::USDT::USDT";

const WORM_VAA_MODULE: &IdentStr = ident_str!("vaa");
const PYTH_PYTH_MODULE: &IdentStr = ident_str!("pyth");
const PYTH_HOT_POTATO_MODULE: &IdentStr = ident_str!("hot_potato_vector");
const PYTH_NETEORK_MODULE: &IdentStr = ident_str!("pyth_network");
const PYTH_PRICE_UPDATE_FEE: u64 = 1;
pub struct Tool {
    ctx: Ctx,
    gas_budget: u64,
}
struct PTBCtx {
    object_ids: Vec<ObjectID>,
    objects: ObjectParams,
    tx: PTB,
    is_init: bool,
    gas_coins: Vec<(u64, SuiObjectData)>,
    ctx: Ctx,
    gas_budget: u64,
}
impl PTBCtx {
    pub fn new(ctx: Ctx, gas_budget: u64) -> Self {
        Self {
            object_ids: Vec::new(),
            objects: ObjectParams::new(),
            tx: PTB::new(),
            is_init: false,
            gas_coins: Vec::new(),
            ctx,
            gas_budget,
        }
    }
    // pub fn add_object_id(&mut self, object_id: ObjectID) {
    //     self.object_ids.push(object_id);
    // }
    pub fn add_object_ids(&mut self, object_ids: Vec<ObjectID>) {
        self.object_ids.extend(object_ids);
    }
    pub async fn init(&mut self, gas: Vec<(u64, SuiObjectData)>) -> anyhow::Result<()> {
        if self.is_init {
            return Ok(());
        }
        self.objects = object::get_object_args(self.ctx.clone(), self.object_ids.clone()).await?;
        self.gas_coins = gas;
        self.is_init = true;
        Ok(())
    }
    pub fn get_object_input(
        &mut self,
        id: ObjectID,
        is_mutable_ref: bool,
    ) -> anyhow::Result<Argument> {
        if !self.is_init {
            return Err(CliError::PTBCtxNotInit.into());
        }
        self.tx.input(CallArg::Object(
            self.objects.get_obj_arg(id, is_mutable_ref)?,
        ))
    }
    pub fn split_one_gas(&mut self, budget: u64) -> anyhow::Result<SuiObjectData> {
        if !self.is_init {
            return Err(CliError::PTBCtxNotInit.into());
        }
        if self.gas_coins.len() != 1 {
            return self.select_one_gas(budget);
        }
        // TODO if account has only one gas coin, split it
        let (_balance, gas_coin_data) = self.gas_coins.pop().ok_or(CliError::NoGasCoin)?;
        Ok(gas_coin_data)
    }
    pub fn select_one_gas(&mut self, budget: u64) -> anyhow::Result<SuiObjectData> {
        if !self.is_init {
            return Err(CliError::PTBCtxNotInit.into());
        }
        for i in 0..self.gas_coins.len() {
            let (balance, gas_coin_data) = self.gas_coins[i].clone();
            if balance >= budget {
                self.gas_coins.remove(i);
                return Ok(gas_coin_data);
            }
        }
        Err(CliError::NoGasCoin.into())
    }
    pub async fn get_transaction_data(mut px: PTBCtx) -> anyhow::Result<TransactionData> {
        if !px.is_init {
            return Err(CliError::PTBCtxNotInit.into());
        }
        let gas = px.select_one_gas(px.gas_budget)?;
        debug!("transaction gas: {:?}", gas.object_id);
        let PTBCtx {
            tx,
            gas_budget,
            ctx,
            ..
        } = px;
        let pt = tx.finish();

        let sender = ctx.get_active_address()?;
        let gas_price = ctx.client.read_api().get_reference_gas_price().await?;
        let tx_data = TransactionData::new(
            TransactionKind::ProgrammableTransaction(pt.to_owned()),
            sender,
            gas.object_ref(),
            gas_budget,
            gas_price.to_owned(),
        );
        Ok(tx_data)
    }
}
impl Tool {
    pub async fn new(conf: Config, gas_budget: u64) -> anyhow::Result<Self> {
        let ctx = Context::new(conf).await?;
        Ok(Self { ctx, gas_budget })
    }
    // package the infos of the update price transaction
    async fn init_price_update_transaction(
        &self,
        total_pyth_fee: u64,
        vaa_data: Vec<String>,
        price_info_object_ids: Vec<ObjectID>,
    ) -> anyhow::Result<PTBCtx> {
        if total_pyth_fee == 0 {
            return Err(CliError::InvalidCliParams("budget is zero".to_string()).into());
        }
        if vaa_data.len() == 0 {
            return Err(CliError::InvalidCliParams("vaa data is empty".to_string()).into());
        }
        let mut px = PTBCtx::new(self.ctx.clone(), self.gas_budget);
        let worm_package_id = self.ctx.get_worm_package_id()?;
        let pyth_package_id = self.ctx.get_pyth_package_id()?;

        let worm_state_id = self.ctx.get_worm_state_id()?;
        let pyth_state_id = self.ctx.get_pyth_state_id()?;
        px.add_object_ids(vec![
            worm_state_id,
            pyth_state_id,
            self.ctx.config.scale_oracle_package_id,
            self.ctx.config.scale_oracle_state_id,
            self.ctx.config.scale_oracle_pyth_state_id,
            SUI_CLOCK_OBJECT_ID,
        ]);
        px.add_object_ids(price_info_object_ids.clone());
        let gas = self.get_all_gas().await?;

        px.init(gas).await?;
        // init object input
        let worm_state_input = px.get_object_input(worm_state_id, false)?;
        let pyth_state_input = px.get_object_input(pyth_state_id, false)?;
        let clok_object_input = px.get_object_input(SUI_CLOCK_OBJECT_ID, false)?;
        let oracle_pyth_state_input =
            px.get_object_input(self.ctx.config.scale_oracle_pyth_state_id, false)?;
        let oracle_state_input =
            px.get_object_input(self.ctx.config.scale_oracle_state_id, true)?;
        // parse_and_verify
        let mut vaa_verified_datas = Vec::new();
        for d in vaa_data.iter() {
            if let Ok(b) = general_purpose::STANDARD.decode(d.as_str()) {
                let call_args = vec![
                    worm_state_input,
                    px.tx.input(CallArg::Pure(bcs::to_bytes(&b).unwrap()))?,
                    clok_object_input,
                ];
                let verified_vaa = px.tx.programmable_move_call(
                    worm_package_id,
                    WORM_VAA_MODULE.to_owned(),
                    Identifier::from_str("parse_and_verify")?,
                    vec![],
                    call_args,
                );
                vaa_verified_datas.push(verified_vaa);
            } else {
                return Err(
                    CliError::InvalidCliParams("vaa_data is not base64".to_string()).into(),
                );
            }
        }
        let vaas = px.tx.command(Command::MakeMoveVec(
            Some(self.ctx.get_worm_vaa_type()?),
            vaa_verified_datas,
        ));
        let mut hot = px.tx.programmable_move_call(
            pyth_package_id,
            PYTH_PYTH_MODULE.to_owned(),
            Identifier::from_str("create_price_infos_hot_potato")?,
            vec![],
            vec![pyth_state_input, vaas, clok_object_input],
        );
        let amount = px.tx.pure(PYTH_PRICE_UPDATE_FEE)?;
        let coin_token = px.split_one_gas(total_pyth_fee)?;
        debug!("transaction python pay gas: {:?}", coin_token.object_id);
        let coin_input = px
            .tx
            .obj(ObjectArg::ImmOrOwnedObject(coin_token.object_ref()))?;
        for info in price_info_object_ids {
            let c = px.tx.programmable_move_call(
                SUI_FRAMEWORK_PACKAGE_ID,
                coin::COIN_MODULE_NAME.to_owned(),
                Identifier::from_str("split")?,
                vec![self.get_sui_coin_type()?],
                vec![coin_input, amount],
            );
            let info_input = px.get_object_input(info, true)?;
            hot = px.tx.programmable_move_call(
                pyth_package_id,
                PYTH_PYTH_MODULE.to_owned(),
                Identifier::from_str("update_single_price_feed")?,
                vec![],
                vec![pyth_state_input, hot, info_input, c, clok_object_input],
            );
            px.tx.programmable_move_call(
                self.ctx.config.scale_oracle_package_id,
                PYTH_NETEORK_MODULE.to_owned(),
                Identifier::from_str("async_pyth_price")?,
                vec![],
                vec![
                    info_input,
                    pyth_state_input,
                    oracle_pyth_state_input,
                    oracle_state_input,
                    clok_object_input,
                ],
            );
        }
        px.tx.programmable_move_call(
            pyth_package_id,
            PYTH_HOT_POTATO_MODULE.to_owned(),
            Identifier::from_str("destroy")?,
            vec![self.ctx.get_price_info_type()?],
            vec![hot],
        );
        return Ok(px);
    }

    pub fn get_t_str(&self) -> String {
        if let Ok(env) = self.ctx.wallet.config.get_active_env() {
            if env.alias == "mainnet" {
                return USDT_TYPE_TAG.to_string();
            }
        }
        format!("{}::scale::SCALE", self.ctx.config.scale_coin_package_id)
    }

    pub fn get_t(&self) -> SuiTypeTag {
        SuiTypeTag::from(
            sui_types::parse_sui_type_tag(self.get_t_str().as_str())
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
    pub fn get_sui_coin_type(&self) -> anyhow::Result<TypeTag> {
        let t = format!(
            "{}::sui::SUI",
            SUI_FRAMEWORK_PACKAGE_ID.to_string().as_str()
        );
        Ok(TypeTag::from_str(t.as_str())?)
    }
    pub async fn get_gas(&self, budget: u64) -> anyhow::Result<Vec<(u64, SuiObjectData)>> {
        let active_address = self.ctx.get_active_address()?;
        let gas_objects = self.ctx.wallet.gas_objects(active_address).await?;
        let mut sui_objects = Vec::new();
        let mut amout = 0u64;
        for gas_object in gas_objects {
            amout += gas_object.0;
            sui_objects.push(gas_object);
            if amout >= budget {
                break;
            }
        }
        if amout < budget {
            let msg = format!(
                "Insufficient gas balance, need {} but only have {}",
                budget, amout
            );
            return Err(CliError::InsufficientGasBalance(msg).into());
        }
        Ok(sui_objects)
    }

    pub async fn get_all_gas(&self) -> anyhow::Result<Vec<(u64, SuiObjectData)>> {
        let active_address = self.ctx.get_active_address()?;
        let gas_objects = self.ctx.wallet.gas_objects(active_address).await?;
        let mut sui_objects = Vec::new();
        for gas_object in gas_objects {
            sui_objects.push(gas_object);
        }
        Ok(sui_objects)
    }

    pub async fn get_coin_object_whith_t(&self, budget: u64) -> anyhow::Result<Vec<ObjectID>> {
        let active_address = self.ctx.get_active_address()?;
        let mut coin_objects: Vec<SuiObjectResponse> = Vec::new();
        let mut cursor = None;
        loop {
            let response = self
                .ctx
                .client
                .read_api()
                .get_owned_objects(
                    active_address,
                    Some(SuiObjectResponseQuery::new(
                        Some(SuiObjectDataFilter::StructType(StructTag::from_str(
                            self.get_t_str().as_str(),
                        )?)),
                        Some(SuiObjectDataOptions::bcs_lossless()),
                    )),
                    cursor,
                    None,
                )
                .await?;
            coin_objects.extend(response.data);
            if response.has_next_page {
                cursor = response.next_cursor;
            } else {
                break;
            }
        }
        let mut token_objects = Vec::new();
        let mut amout = 0u64;
        for gas_object in coin_objects {
            if let Some(bcs) = gas_object.move_object_bcs() {
                let c = Coin::from_bcs_bytes(bcs)?;
                amout += c.value();
                token_objects.push(*c.id());
                if amout >= budget {
                    break;
                }
            }
        }
        if amout < budget {
            let msg = format!(
                "Insufficient gas balance, need {} but only have {}",
                budget, amout
            );
            return Err(CliError::InsufficientGasBalance(msg).into());
        }
        Ok(token_objects)
    }

    fn get_transaction_signature(&self, pm: &TransactionData) -> anyhow::Result<Signature> {
        let address = self.ctx.wallet.config.active_address.ok_or_else(|| {
            CliError::NoActiveAccount(
                "no active account, please use sui client command create it .".to_string(),
            )
        })?;
        let signature =
            self.ctx
                .wallet
                .config
                .keystore
                .sign_secure(&address, pm, Intent::sui_transaction())?;
        Ok(signature)
    }

    async fn exec(&self, pm: TransactionData) -> anyhow::Result<()> {
        let signature = self.get_transaction_signature(&pm)?;
        let opt = SuiTransactionBlockResponseOptions::default();
        let tx = self
            .ctx
            .client
            .quorum_driver_api()
            .execute_transaction_block(
                Transaction::from_data(pm.clone(), Intent::sui_transaction(), vec![signature]),
                opt,
                Some(ExecuteTransactionRequestType::WaitForLocalExecution),
            )
            .await?;

        if tx.errors.is_empty() {
            println!("exec: {:?} , success", tx.digest.to_string());
        } else {
            println!("exec: {:?} , error: {:?}", tx.digest.to_string(), tx.errors);
        }
        if let Some(e) = tx.effects {
            let SuiTransactionBlockEffects::V1(v) = e;
            match v.status {
                SuiExecutionStatus::Success => {
                    return Ok(());
                }
                SuiExecutionStatus::Failure { error } => {
                    println!(" error: {:?}", error);
                    return Err(CliError::TransactionExecutionFailure(error).into());
                }
            }
        }
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
                self.ctx.wallet.config.active_address.ok_or_else(|| {
                    CliError::NoActiveAccount(
                        "no active account, please use sui client command create it .".to_string(),
                    )
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
        let status = args
            .get_one::<u8>("status")
            .ok_or_else(|| CliError::InvalidCliParams("status".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_coin_package_id,
                COIN_MODULE_NAME,
                "set_staatus",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_coin_admin_id),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_coin_reserve_id),
                    SuiJsonValue::new(json!(status))?,
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
                COIN_MODULE_NAME,
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
        let amount = args
            .get_one::<u64>("amount")
            .ok_or_else(|| CliError::InvalidCliParams("amount".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_coin_package_id,
                COIN_MODULE_NAME,
                "airdrop",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_coin_reserve_id),
                    SuiJsonValue::new(json!(amount.to_string()))?,
                ],
                vec![],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn coin_mint(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let amount = args
            .get_one::<u64>("amount")
            .ok_or_else(|| CliError::InvalidCliParams("amount".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_coin_package_id,
                COIN_MODULE_NAME,
                "mint",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_coin_admin_id),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_coin_reserve_id),
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
                ORACLE_MODULE_NAME,
                "create_price_feed",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_oracle_admin_id),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_oracle_state_id),
                    SuiJsonValue::new(json!(symbol))?,
                ],
                vec![],
            )
            .await?;
        self.exec(transaction_data).await
    }

    async fn get_vaa_data_inner(&self) -> anyhow::Result<Vec<String>> {
        let feed_ids = self.ctx.config.price_config.get_feed_ids();
        utils::get_vaa_data(
            &self.ctx.http_client,
            self.ctx.config.price_config.price_server_url.clone(),
            feed_ids,
        )
        .await
    }

    pub async fn get_latest_vaas(&self, _args: &clap::ArgMatches) -> anyhow::Result<()> {
        println!("vaa data: {:?}", self.get_vaa_data_inner().await?);
        Ok(())
    }

    pub async fn update_symbol(&self, _args: &clap::ArgMatches) -> anyhow::Result<()> {
        let mut p = Vec::new();
        for s in &self.ctx.config.price_config.pyth_symbol {
            let ts = RPCTransactionRequestParams::MoveCallRequestParams(MoveCallParams {
                package_object_id: self.ctx.config.scale_oracle_package_id,
                module: ORACLE_PYTH_MODULE_NAME.to_string(),
                function: "update_symbol".to_string(),
                type_arguments: vec![],
                arguments: vec![
                    SuiJsonValue::new(json!(s.symbol.as_bytes()))?,
                    SuiJsonValue::from_object_id(ObjectID::from_str(
                        s.price_info_object_id.as_str(),
                    )?),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_oracle_pyth_state_id),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_oracle_admin_id),
                ],
            });
            p.push(ts);
        }
        let ts_data = self
            .ctx
            .client
            .transaction_builder()
            .batch_transaction(self.ctx.get_active_address()?, p, None, self.gas_budget)
            .await?;
        self.exec(ts_data).await
    }

    pub async fn update_pyth_price_auto(&self) -> anyhow::Result<()> {
        // let total_pyth_fee = price_info_object_ids.len() as u64 * PYTH_PRICE_UPDATE_FEE;
        // vaa_data = utils::get_vaa_data(
        //     &self.ctx.http_client,
        //     self.ctx.config.price_config.price_server_url.clone(),
        //     self.ctx.config.price_config.get_feed_ids(),
        // )
        // .await?;
        // debug!("vaa data: {:?}", vaa_data);
        // self.update_pyth_price_bat_inner(*budget, vaa_data, price_info_object_ids)
        //     .await
        Ok(())
    }
    pub async fn update_all_pyth_price(&self) -> anyhow::Result<()> {
        let vaa_data = utils::get_vaa_data(
            &self.ctx.http_client,
            self.ctx.config.price_config.price_server_url.clone(),
            self.ctx.config.price_config.get_feed_ids(),
        )
        .await?;

        let price_info_object_ids = self.ctx.get_price_info_object_ids()?;
        let total_pyth_fee = price_info_object_ids.len() as u64 * PYTH_PRICE_UPDATE_FEE;
        let px = self
            .init_price_update_transaction(total_pyth_fee, vaa_data, price_info_object_ids)
            .await?;

        let tx_data = PTBCtx::get_transaction_data(px).await?;
        // let response = self
        //     .ctx
        //     .client
        //     .read_api()
        //     .dry_run_transaction_block(tx_data)
        //     .await?;
        // println!("dry_run_transaction_block: {:?}", response.effects);
        // Ok(())
        self.exec(tx_data).await
    }
    pub async fn update_pyth_price_bat(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let vaa_data = args.get_many::<String>("data").unwrap_or_default();
        let mut vaa_data: Vec<String> = vaa_data.map(|d| d.to_string()).collect();
        if vaa_data.len() == 0 {
            vaa_data = utils::get_vaa_data(
                &self.ctx.http_client,
                self.ctx.config.price_config.price_server_url.clone(),
                self.ctx.config.price_config.get_feed_ids(),
            )
            .await?;
        }
        debug!("vaa data: {:?}", vaa_data);
        let update_fee = args
            .get_one::<u64>("update_fee")
            .ok_or_else(|| CliError::InvalidCliParams("update_fee".to_string()))?;

        let price_info_object_ids = self.ctx.get_price_info_object_ids()?;
        let total_pyth_fee = price_info_object_ids.len() as u64 * *update_fee;
        debug!("update pyth price bat");
        let px = self
            .init_price_update_transaction(total_pyth_fee, vaa_data, price_info_object_ids)
            .await?;
        let tx_data = PTBCtx::get_transaction_data(px).await?;
        self.exec(tx_data).await
    }

    async fn _get_coin_value(&self) -> anyhow::Result<()> {
        let mut pt_builder = PTB::new();
        let sender = self.ctx.get_active_address()?;

        let gas_price = self.ctx.client.read_api().get_reference_gas_price().await?;
        let mut gas_coins = self.get_gas(1000000000).await?;
        println!("gas_coins: {:?}", gas_coins);
        let richest_coin = gas_coins.pop().unwrap();
        let richest_coin_gas = gas_coins.first().unwrap();
        let original_coin_arg = ObjectArg::ImmOrOwnedObject(richest_coin.1.object_ref());
        let original_coin_arg = pt_builder.obj(original_coin_arg)?;
        let sui_coin_arg_type = TypeTag::from_str("0x2::sui::SUI")?;
        let value_function = Identifier::from_str("value")?;
        let initial_value_result = pt_builder.programmable_move_call(
            SUI_FRAMEWORK_PACKAGE_ID,
            coin::COIN_MODULE_NAME.to_owned(),
            value_function.to_owned(),
            vec![sui_coin_arg_type.to_owned()],
            vec![original_coin_arg],
        );
        let pt = pt_builder.finish();

        let tx_data = TransactionKind::ProgrammableTransaction(pt.to_owned());
        println!("initial_value_result: {:?}", initial_value_result);
        let response = self
            .ctx
            .client
            .read_api()
            .dry_run_transaction_block(TransactionData::new(
                tx_data,
                sender,
                richest_coin_gas.1.object_ref(),
                self.gas_budget,
                gas_price.to_owned(),
            ))
            .await?;
        println!("dry_run_transaction_block: {:?}", response.effects);
        Ok(())
    }

    async fn get_price_inner(&self, symbol: &str) -> anyhow::Result<()> {
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_oracle_package_id,
                ORACLE_MODULE_NAME,
                "get_price",
                vec![
                    SuiJsonValue::from_object_id(ObjectID::from_str(
                        self.ctx.config.price_config.pyth_state.as_str(),
                    )?),
                    SuiJsonValue::new(json!(symbol.as_bytes()))?,
                ],
                vec![],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn get_price(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let symbol = args
            .get_one::<String>("symbol")
            .ok_or_else(|| CliError::InvalidCliParams("symbol".to_string()))?;
        self.get_price_inner(symbol.as_str()).await
    }

    pub async fn create_account(&self, _args: &clap::ArgMatches) -> anyhow::Result<()> {
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_MODULE_NAME,
                "create_account",
                vec![],
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
                SCALE_MODULE_NAME,
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
                SCALE_MODULE_NAME,
                "withdrawal",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_market_list_id),
                    SuiJsonValue::from_object_id(ObjectID::from_str(account.as_str())?),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_oracle_state_id),
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
        debug!(
            "package: {},module: {} ,name: {}, description: {}, img_url: {}",
            self.ctx.config.scale_nft_package_id, NFT_MODULE_NAME, name, description, img_url
        );
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_nft_package_id,
                NFT_MODULE_NAME,
                "mint",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_nft_admin_id),
                    SuiJsonValue::new(json!(name.as_bytes()))?,
                    SuiJsonValue::new(json!(description.as_bytes()))?,
                    SuiJsonValue::new(json!(img_url.as_bytes()))?,
                ],
                vec![],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn burn(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
        let id = args
            .get_one::<String>("id")
            .ok_or_else(|| CliError::InvalidCliParams("id".to_string()))?;
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_nft_package_id,
                NFT_MODULE_NAME,
                "mint",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_nft_admin_id),
                    SuiJsonValue::from_object_id(ObjectID::from_str(id.as_str())?),
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
                self.ctx.config.scale_nft_package_id,
                NFT_MODULE_NAME,
                "mint_recipient",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_nft_admin_id),
                    SuiJsonValue::new(json!(name.as_bytes()))?,
                    SuiJsonValue::new(json!(description.as_bytes()))?,
                    SuiJsonValue::new(json!(img_url.as_bytes()))?,
                    SuiJsonValue::from_object_id(ObjectID::from_str(recipient.as_str())?),
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
                self.ctx.config.scale_nft_package_id,
                NFT_MODULE_NAME,
                "mint_multiple_recipient",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_nft_admin_id),
                    SuiJsonValue::new(json!(name.as_bytes()))?,
                    SuiJsonValue::new(json!(description.as_bytes()))?,
                    SuiJsonValue::new(json!(img_url.as_bytes()))?,
                    SuiJsonValue::new(json!(amount.to_string()))?,
                    SuiJsonValue::from_object_id(ObjectID::from_str(recipient.as_str())?),
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
                self.ctx.config.scale_nft_package_id,
                NFT_MODULE_NAME,
                "mint_multiple",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_nft_admin_id),
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
                SCALE_MODULE_NAME,
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
                SCALE_MODULE_NAME,
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
    pub async fn create_lsp(&self, _args: &clap::ArgMatches) -> anyhow::Result<()> {
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_MODULE_NAME,
                "create_lsp",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_admin_cap_id),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_publisher_id),
                ],
                vec![self.get_t()],
            )
            .await?;
        self.exec(transaction_data).await
    }

    pub async fn create_market(&self, args: &clap::ArgMatches) -> anyhow::Result<()> {
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
        let transaction_data = self
            .get_transaction_data(
                self.ctx.config.scale_package_id,
                SCALE_MODULE_NAME,
                "create_market",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_market_list_id),
                    SuiJsonValue::new(json!(symbol.as_bytes()))?,
                    SuiJsonValue::new(json!(icon.as_bytes()))?,
                    SuiJsonValue::new(json!(description.as_bytes()))?,
                    SuiJsonValue::new(json!(size.to_string()))?,
                    SuiJsonValue::new(json!(opening_price.to_string()))?,
                ],
                vec![self.get_p(), self.get_t()],
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
                SCALE_MODULE_NAME,
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
                SCALE_MODULE_NAME,
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
                SCALE_MODULE_NAME,
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
                SCALE_MODULE_NAME,
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
                SCALE_MODULE_NAME,
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
                SCALE_MODULE_NAME,
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
                SCALE_MODULE_NAME,
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
                SCALE_MODULE_NAME,
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
                SCALE_MODULE_NAME,
                "add_factory_mould",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_admin_cap_id),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_bond_factory_id),
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
                SCALE_MODULE_NAME,
                "remove_factory_mould",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_admin_cap_id),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_bond_factory_id),
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
                SCALE_MODULE_NAME,
                "investment",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_market_list_id),
                    SuiJsonValue::from_object_id(ObjectID::from_str(market.as_str())?),
                    SuiJsonValue::new(json!(coins))?,
                    SuiJsonValue::from_object_id(self.ctx.config.scale_bond_factory_id),
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
                SCALE_MODULE_NAME,
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
                SCALE_MODULE_NAME,
                "trigger_update_opening_price",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_market_list_id),
                    SuiJsonValue::from_object_id(ObjectID::from_str(market.as_str())?),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_oracle_state_id),
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
                SCALE_MODULE_NAME,
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
                SCALE_MODULE_NAME,
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
                SCALE_MODULE_NAME,
                "open_position",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_market_list_id),
                    SuiJsonValue::from_object_id(ObjectID::from_str(market.as_str())?),
                    SuiJsonValue::from_object_id(ObjectID::from_str(account.as_str())?),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_oracle_state_id),
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
                SCALE_MODULE_NAME,
                "close_position",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_market_list_id),
                    SuiJsonValue::from_object_id(ObjectID::from_str(market.as_str())?),
                    SuiJsonValue::from_object_id(ObjectID::from_str(account.as_str())?),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_oracle_state_id),
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
                SCALE_MODULE_NAME,
                "trigger_update_opening_price",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_market_list_id),
                    SuiJsonValue::from_object_id(ObjectID::from_bytes(
                        market_id.to_vec().as_slice(),
                    )?),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_oracle_state_id),
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
                SCALE_MODULE_NAME,
                "burst_position",
                vec![
                    SuiJsonValue::from_object_id(self.ctx.config.scale_market_list_id),
                    SuiJsonValue::from_object_id(ObjectID::from_bytes(
                        account_id.to_vec().as_slice(),
                    )?),
                    SuiJsonValue::from_object_id(self.ctx.config.scale_oracle_state_id),
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
                SCALE_MODULE_NAME,
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

    async fn get_price(&self, symbol: &str) -> anyhow::Result<()> {
        self.get_price_inner(symbol).await
    }
}
