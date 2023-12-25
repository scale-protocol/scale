use crate::{
    bot::state::{Account, List, Market, MessageSender, Event,Message, Position, State, Storage},
    bot::storage::entity::{DbAccount, DbList, DbMarket, DbPosition},
    com::ClientError,
    config::SqlDbConfig,
};
use anyhow::Ok;
use async_trait::async_trait;
use sqlx::postgres::{PgPool, PgPoolOptions};
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
        self.load_all_list(send.clone()).await?;
        self.load_all_market(send.clone()).await?;
        self.load_all_account(send.clone()).await?;
        self.load_all_position(send.clone()).await?;
        Ok(())
    }
}

impl PG {
    pub async fn save_list(&self, data: List) -> anyhow::Result<()> {
        let ins: DbList = data.into();
        sqlx::query!(
            r#"
            INSERT INTO tb_list (id,total,officer,vault_supply,vault_balance,profit_balance,insurance_balance,spread_profit,epoch_profit)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8,$9)
            ON CONFLICT (id) DO UPDATE SET total = $2, officer = $3, vault_supply = $4, vault_balance = $5, profit_balance = $6, insurance_balance = $7,spread_profit = $8, epoch_profit = $9
            "#,
            ins.id,
            ins.total,
            ins.officer,
            ins.vault_supply,
            ins.vault_balance,
            ins.profit_balance,
            ins.insurance_balance,
            ins.spread_profit,
            ins.epoch_profit
        )
        .fetch_one(&self.db)
        .await?;
        Ok(())
    }
    pub async fn save_market(&self, data: Market) -> anyhow::Result<()> {
        let ins: DbMarket = data.into();
        sqlx::query!(
            r#"
            INSERT INTO tb_market (id, max_leverage, insurance_fee, margin_fee, fund_fee, fund_fee_manual, spread_fee, spread_fee_manual, status, long_position_total, short_position_total, symbol, symbol_short, icon, description, unit_size, opening_price, list_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10,$11,$12,$13,$14,$15,$16,$17,$18)
            ON CONFLICT (id) DO UPDATE SET max_leverage = $2, insurance_fee = $3, margin_fee = $4, fund_fee = $5, fund_fee_manual = $6, spread_fee = $7, spread_fee_manual = $8, status = $9, long_position_total = $10, short_position_total = $11, symbol = $12, symbol_short = $13, icon = $14, description = $15, unit_size = $16, opening_price = $17
            "#,
            ins.id,
            ins.max_leverage,
            ins.insurance_fee,
            ins.margin_fee,
            ins.fund_fee,
            ins.fund_fee_manual,
            ins.spread_fee,
            ins.spread_fee_manual, 
            ins.status,
            ins.long_position_total,
            ins.short_position_total,
            ins.symbol,
            ins.symbol_short,
            ins.icon,
            ins.description,
            ins.unit_size,
            ins.opening_price,
            ins.list_id
        ).fetch_one(&self.db).await?;
        Ok(())
    }
    pub async fn save_account(&self, data: Account) -> anyhow::Result<()> {
        let ins: DbAccount = data.into();
        sqlx::query!(
            r#"
            INSERT INTO tb_account (id, owner, offset_idx, balance, isolated_balance, profit, margin_total, margin_cross_total, margin_isolated_total, margin_cross_buy_total, margin_cross_sell_total, margin_isolated_buy_total, margin_isolated_sell_total, cross_position_idx, isolated_position_idx)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10,$11,$12,$13,$14,$15)
            ON CONFLICT (id) DO UPDATE SET owner = $2, offset_idx = $3, balance = $4, isolated_balance = $5, profit = $6, margin_total = $7, margin_cross_total = $8, margin_isolated_total = $9, margin_cross_buy_total = $10, margin_cross_sell_total = $11, margin_isolated_buy_total = $12, margin_isolated_sell_total = $13, cross_position_idx = $14, isolated_position_idx = $15
            "#,
            ins.id,
            ins.owner,
            ins.offset_idx,
            ins.balance,
            ins.isolated_balance,
            ins.profit,
            ins.margin_total,
            ins.margin_cross_total,
            ins.margin_isolated_total,
            ins.margin_cross_buy_total,
            ins.margin_cross_sell_total,
            ins.margin_isolated_buy_total,
            ins.margin_isolated_sell_total,
            ins.cross_position_idx,
            &ins.isolated_position_idx
        ).fetch_one(&self.db).await?;
        Ok(())
    }
    pub async fn save_position(&self, data: Position) -> anyhow::Result<()> {
        let ins: DbPosition = data.into();
        sqlx::query!(
            r#"
            INSERT INTO tb_position (id, offset_idx, margin, margin_balance, leverage, position_type, status, direction, unit_size, lot, open_price, open_spread, open_real_price, close_price, close_spread, close_real_price, profit, stop_surplus_price, stop_loss_price, create_time, open_time, close_time, open_operator, close_operator, market_id, account_id, symbol, symbol_short, icon)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,$25,$26,$27,$28,$29)
            ON CONFLICT (id) DO UPDATE SET offset_idx = $2, margin = $3, margin_balance = $4, leverage = $5, position_type = $6, status = $7, direction = $8, unit_size = $9, lot = $10, open_price = $11, open_spread = $12, open_real_price = $13, close_price = $14, close_spread = $15, close_real_price = $16, profit = $17, stop_surplus_price = $18, stop_loss_price = $19, create_time = $20, open_time = $21, close_time = $22, open_operator = $23, close_operator = $24, market_id = $25, account_id = $26, symbol = $27, symbol_short = $28, icon = $29
            "#,
            ins.id,
            ins.offset_idx,
            ins.margin,
            ins.margin_balance,
            ins.leverage,
            ins.position_type,
            ins.status,
            ins.direction,
            ins.unit_size,
            ins.lot,
            ins.open_price,
            ins.open_spread,
            ins.open_real_price,
            ins.close_price,
            ins.close_spread,
            ins.close_real_price,
            ins.profit,
            ins.stop_surplus_price,
            ins.stop_loss_price,
            ins.create_time,
            ins.open_time,
            ins.close_time,
            ins.open_operator,
            ins.close_operator,
            ins.market_id,
            ins.account_id,
            ins.symbol,
            ins.symbol_short,
            ins.icon
        ).fetch_one(&self.db).await?;
        Ok(())
    }
    async fn load_all_list(&self, send: MessageSender) -> anyhow::Result<()> {
        let limit = 100;
        let mut offset = 0;
        loop {
            let list = sqlx::query_as!(
                DbList,
                r#"
                SELECT *
                FROM tb_list
                ORDER BY id
                LIMIT $1 OFFSET $2
                "#,
                limit,
                offset
            ).fetch_all(&self.db).await?;
            if list.len() == 0 {
                break;
            }
            for item in list {
                let data: List = item.into();
                send.send(Message{
                    state: State::List(data),
                    event: Event::None
                })?;
            }
            offset += limit;
        }
        Ok(())
    }
    async fn load_all_market(&self, send: MessageSender) -> anyhow::Result<()> {
        let limit = 100;
        let mut offset = 0;
        loop {
            let list = sqlx::query_as!(
                DbMarket,
                r#"
                SELECT *
                FROM tb_market
                ORDER BY id
                LIMIT $1 OFFSET $2
                "#,
                limit,
                offset
            ).fetch_all(&self.db).await?;
            if list.len() == 0 {
                break;
            }
            for item in list {
                let data: Market = item.into();
                send.send(Message{
                    state: State::Market(data),
                    event: Event::None
                })?;
            }
            offset += limit;
        }
        Ok(())
    }
    async fn load_all_account(&self, send: MessageSender) -> anyhow::Result<()> {
        let limit = 100;
        let mut offset = 0;
        loop {
            let list = sqlx::query_as!(
                DbAccount,
                r#"
                SELECT *
                FROM tb_account
                ORDER BY id
                LIMIT $1 OFFSET $2
                "#,
                limit,
                offset
            ).fetch_all(&self.db).await?;
            if list.len() == 0 {
                break;
            }
            for item in list {
                let data: Account = item.into();
                send.send(Message{
                    state: State::Account(data),
                    event: Event::None
                })?;
            }
            offset += limit;
        }
        Ok(())
    }
    async fn load_all_position(&self, send: MessageSender) -> anyhow::Result<()> {
        let limit = 100;
        let mut offset = 0;
        loop {
            let list = sqlx::query_as!(
                DbPosition,
                r#"
                SELECT *
                FROM tb_position
                ORDER BY id
                LIMIT $1 OFFSET $2
                "#,
                limit,
                offset
            ).fetch_all(&self.db).await?;
            if list.len() == 0 {
                break;
            }
            for item in list {
                let data: Position = item.into();
                send.send(Message{
                    state: State::Position(data),
                    event: Event::None
                })?;
            }
            offset += limit;
        }
        Ok(())
    }
}
