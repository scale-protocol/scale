use crate::{
    bot::state::{Account, Address, List, Market, MessageSender, Pool, Position, State, Storage},
    com::ClientError,
    config::SqlDbConfig,
};
use anyhow::Ok;
use async_trait::async_trait;
use chrono::offset;
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;
use tower::limit;

pub struct PG {
    db: PgPool,
}

pub async fn new(conf: SqlDbConfig) -> anyhow::Result<PG> {
    let db = PgPoolOptions::new()
        .max_connections(conf.pool_max_conn)
        .min_connections(conf.pool_min_conn)
        .connect(&conf.db_url.as_str())
        .await
        .map_err(|e| ClientError::DBError(e.to_string()))?;

    sqlx::migrate!("db/migrations").run(&db).await?;
    Ok(PG { db })
}
#[async_trait]
impl Storage for PG {
    async fn save_one(&self, state: State) -> anyhow::Result<()> {
        match state {
            State::List(data) => self.save_list(data).await?,
            State::Market(data) => self.save_market(data).await?,
            State::Account(data) => self.save_account(data).await?,
            State::Position(data) => self.save_position(data).await?,
            _ => {}
        }
        Ok(())
    }
    async fn load_all(&self, send: MessageSender) -> anyhow::Result<()> {
        Ok(())
    }
}

impl PG {
    pub async fn save_list(&self, data: List) -> anyhow::Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO tb_list (id,total,officer,vault_supply,vault_balance,profit_balance,insurance_balance,epoch_profit)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (id) DO UPDATE SET total = $2, officer = $3, vault_supply = $4, vault_balance = $5, profit_balance = $6, insurance_balance = $7, epoch_profit = $8
            "#,
            data.id.to_string(),
            data.total as i32,
            data.officer as i16,
            data.pool.vault_supply as i64,
            data.pool.vault_balance as i64,
            data.pool.profit_balance as i64,
            data.pool.insurance_balance as i64,
            serde_json::to_value(&data.pool.epoch_profit)?
        )
        .fetch_one(&self.db)
        .await?;
        Ok(())
    }
    pub async fn save_market(&self, data: Market) -> anyhow::Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO tb_market (id, max_leverage, insurance_fee, margin_fee, fund_fee, fund_fee_manual, spread_fee, spread_fee_manual, status, long_position_total, short_position_total, symbol, symbol_short, icon, description, unit_size, opening_price, list_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10,$11,$12,$13,$14,$15,$16,$17,$18)
            ON CONFLICT (id) DO UPDATE SET max_leverage = $2, insurance_fee = $3, margin_fee = $4, fund_fee = $5, fund_fee_manual = $6, spread_fee = $7, spread_fee_manual = $8, status = $9, long_position_total = $10, short_position_total = $11, symbol = $12, symbol_short = $13, icon = $14, description = $15, unit_size = $16, opening_price = $17
            "#,
            data.id.to_string(),
            data.max_leverage as i16,
            data.insurance_fee as i64,
            data.margin_fee as i64,
            data.fund_fee as i64,
            data.fund_fee_manual,
            data.spread_fee as i64,
            data.spread_fee_manual,
            data.status as i16,
            data.long_position_total as i64,
            data.short_position_total as i64,
            data.symbol,
            data.symbol_short,
            data.icon,
            data.description,
            data.unit_size as i64,
            data.opening_price as i64,
            data.list_id.to_string()
        ).fetch_one(&self.db).await?;
        Ok(())
    }
    pub async fn save_account(&self, data: Account) -> anyhow::Result<()> {
        let isolated_position_idx = data
            .isolated_position_idx
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<String>>();
        sqlx::query!(
            r#"
            INSERT INTO tb_account (id, owner, offset_idx, balance, isolated_balance, profit, margin_total, margin_cross_total, margin_isolated_total, margin_cross_buy_total, margin_cross_sell_total, margin_isolated_buy_total, margin_isolated_sell_total, cross_position_idx, isolated_position_idx)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10,$11,$12,$13,$14,$15)
            ON CONFLICT (id) DO UPDATE SET owner = $2, offset_idx = $3, balance = $4, isolated_balance = $5, profit = $6, margin_total = $7, margin_cross_total = $8, margin_isolated_total = $9, margin_cross_buy_total = $10, margin_cross_sell_total = $11, margin_isolated_buy_total = $12, margin_isolated_sell_total = $13, cross_position_idx = $14, isolated_position_idx = $15
            "#,
            data.id.to_string(),
            data.owner.to_string(),
            data.offset as i16,
            data.balance as i64,
            data.isolated_balance as i64,
            data.profit as i64,
            data.margin_total as i64,
            data.margin_cross_total as i64,
            data.margin_isolated_total as i64,
            data.margin_cross_buy_total as i64,
            data.margin_cross_sell_total as i64,
            data.margin_isolated_buy_total as i64,
            data.margin_isolated_sell_total as i64,
            serde_json::to_value(&data.cross_position_idx)?,
            isolated_position_idx.as_slice()
        ).fetch_one(&self.db).await?;
        Ok(())
    }
    pub async fn save_position(&self, data: Position) -> anyhow::Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO tb_position (id, offset_idx, margin, margin_balance, leverage, position_type, status, direction, unit_size, lot, open_price, open_spread, open_real_price, close_price, close_spread, close_real_price, profit, stop_surplus_price, stop_loss_price, create_time, open_time, close_time, open_operator, close_operator, market_id, account_id, symbol, symbol_short, icon)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,$25,$26,$27,$28,$29)
            ON CONFLICT (id) DO UPDATE SET offset_idx = $2, margin = $3, margin_balance = $4, leverage = $5, position_type = $6, status = $7, direction = $8, unit_size = $9, lot = $10, open_price = $11, open_spread = $12, open_real_price = $13, close_price = $14, close_spread = $15, close_real_price = $16, profit = $17, stop_surplus_price = $18, stop_loss_price = $19, create_time = $20, open_time = $21, close_time = $22, open_operator = $23, close_operator = $24, market_id = $25, account_id = $26, symbol = $27, symbol_short = $28, icon = $29
            "#,
            data.id.to_string(),
            data.offset as i16,
            data.margin as i64,
            data.margin_balance as i64,
            data.leverage as i16,
            data.position_type as i16,
            data.status as i16,
            data.direction as i16,
            data.unit_size as i64,
            data.lot as i64,
            data.open_price as i64,
            data.open_spread as i64,
            data.open_real_price as i64,
            data.close_price as i64,
            data.close_spread as i64,
            data.close_real_price as i64,
            data.profit as i64,
            data.stop_surplus_price as i64,
            data.stop_loss_price as i64,
            data.create_time as i64,
            data.open_time as i64,
            data.close_time as i64,
            data.open_operator.to_string(),
            data.close_operator.to_string(),
            data.market_id.to_string(),
            data.account_id.to_string(),
            data.symbol.to_string(),
            data.symbol_short.to_string(),
            data.icon.to_string()
        ).fetch_one(&self.db).await?;
        Ok(())
    }
    async fn load_all_list(&self, send: MessageSender) -> anyhow::Result<()> {
        let limit = 100;
        let offset = 0;
        let mut list = sqlx::query_as!(
            List,
            r#"
            SELECT id,total,officer,vault_supply,vault_balance,profit_balance,insurance_balance,epoch_profit
            FROM tb_list
            ORDER BY id
            LIMIT $1 OFFSET $2
            "#,
            limit,
            offset
        ).fetch_all(&self.db).await?;
        Ok(())
    }
}
