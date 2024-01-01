use crate::bot::state::{
    Account, Address, Direction, List, Market, MarketStatus, Officer, Pool, Position,
    PositionStatus, PositionType,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::str::FromStr;
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DbList {
    pub id: String,
    pub total: i32,
    /// Market operator,
    /// 1 project team
    /// 2 Certified Third Party
    /// 3 Community
    pub officer: i16,
    // The original supply of the liquidity pool represents
    // the liquidity funds obtained through the issuance of NFT bonds
    pub vault_supply: Decimal,
    // Token balance of basic current fund.
    pub vault_balance: Decimal,
    // Token balance of profit and loss fund
    pub profit_balance: Decimal,
    // Insurance fund token balance
    pub insurance_balance: Decimal,
    // Spread benefits, to prevent robot cheating and provide benefits to sponsors
    pub spread_profit: Decimal,
    pub epoch_profit: JsonValue,
}
impl From<List> for DbList {
    fn from(value: List) -> Self {
        DbList {
            id: value.id.to_string(),
            total: value.total as i32,
            officer: value.officer as i16,
            vault_supply: Decimal::from_i128_with_scale(value.pool.vault_supply as i128, 0),
            vault_balance: Decimal::from_i128_with_scale(value.pool.vault_balance as i128, 0),
            profit_balance: Decimal::from_i128_with_scale(value.pool.profit_balance as i128, 0),
            insurance_balance: Decimal::from_i128_with_scale(
                value.pool.insurance_balance as i128,
                0,
            ),
            spread_profit: Decimal::from_i128_with_scale(value.pool.spread_profit as i128, 0),
            epoch_profit: json!(value.pool.epoch_profit),
        }
    }
}
impl From<DbList> for List {
    fn from(value: DbList) -> Self {
        let epoch_profit: HashMap<u64, u64> = serde_json::from_value(value.epoch_profit).unwrap();
        List {
            id: Address::from_str(value.id.as_str()).unwrap(),
            total: value.total as u64,
            officer: Officer::try_from(value.officer as u8).unwrap(),
            pool: Pool {
                vault_supply: value.vault_supply.mantissa() as u64,
                vault_balance: value.vault_balance.mantissa() as u64,
                profit_balance: value.profit_balance.mantissa() as u64,
                insurance_balance: value.insurance_balance.mantissa() as u64,
                spread_profit: value.spread_profit.mantissa() as u64,
                epoch_profit: epoch_profit,
            },
        }
    }
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DbMarket {
    pub id: String,
    /// Maximum allowable leverage ratio
    pub max_leverage: i16,
    /// insurance rate
    pub insurance_fee: i64,
    /// margin rate,Current constant positioning 100%
    pub margin_fee: i64,
    /// The position fund rate will be calculated automatically according to the rules,
    /// and this value will be used when manually set
    pub fund_fee: i64,
    /// Take the value of fund_fee when this value is true
    pub fund_fee_manual: bool,
    /// Point difference (can be understood as slip point),
    /// deviation between the executed quotation and the actual quotation
    pub spread_fee: i64,
    /// Take the value of spread_fee when this value is true
    pub spread_fee_manual: bool,
    /// Market status:
    /// 1 Normal;
    /// 2. Lock the market, allow closing settlement and not open positions;
    /// 3 The market is frozen, and opening and closing positions are not allowed.
    pub status: i16,
    /// Total amount of long positions in the market
    pub long_position_total: Decimal,
    /// Total amount of short positions in the market
    pub short_position_total: Decimal,
    /// Transaction pair (token type, such as BTC, ETH)
    pub symbol: String,
    pub symbol_short: String,
    pub icon: String,
    /// market description
    pub description: String,
    /// Basic size of transaction pair contract
    /// Constant 1 in the field of encryption
    pub unit_size: i64,
    /// The price at 0 o'clock in the utc of the current day, which is used to calculate the spread_fee
    pub opening_price: i64,
    pub list_id: String,
}
impl From<Market> for DbMarket {
    fn from(value: Market) -> Self {
        DbMarket {
            id: value.id.to_string(),
            max_leverage: value.max_leverage as i16,
            insurance_fee: value.insurance_fee as i64,
            margin_fee: value.margin_fee as i64,
            fund_fee: value.fund_fee as i64,
            fund_fee_manual: value.fund_fee_manual,
            spread_fee: value.spread_fee as i64,
            spread_fee_manual: value.spread_fee_manual,
            status: value.status as i16,
            long_position_total: Decimal::from_i128_with_scale(
                value.long_position_total as i128,
                0,
            ),
            short_position_total: Decimal::from_i128_with_scale(
                value.short_position_total as i128,
                0,
            ),
            symbol: value.symbol,
            symbol_short: value.symbol_short,
            icon: value.icon,
            description: value.description,
            unit_size: value.unit_size as i64,
            opening_price: value.opening_price as i64,
            list_id: value.list_id.to_string(),
        }
    }
}
impl From<DbMarket> for Market {
    fn from(value: DbMarket) -> Self {
        Market {
            id: Address::from_str(value.id.as_str()).unwrap(),
            max_leverage: value.max_leverage as u8,
            insurance_fee: value.insurance_fee as u64,
            margin_fee: value.margin_fee as u64,
            fund_fee: value.fund_fee as u64,
            fund_fee_manual: value.fund_fee_manual,
            spread_fee: value.spread_fee as u64,
            spread_fee_manual: value.spread_fee_manual,
            status: MarketStatus::try_from(value.status as u8).unwrap(),
            long_position_total: value.long_position_total.mantissa() as u64,
            short_position_total: value.short_position_total.mantissa() as u64,
            symbol: value.symbol,
            symbol_short: value.symbol_short,
            icon: value.icon,
            description: value.description,
            unit_size: value.unit_size as u64,
            opening_price: value.opening_price as u64,
            list_id: Address::from_str(value.list_id.as_str()).unwrap(),
        }
    }
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DbAccount {
    pub id: String,
    pub owner: String,
    /// The position offset.
    /// like order id
    pub offset_idx: i64,
    /// Balance of user account (maintain the deposit,
    /// and the balance here will be deducted when the deposit used in the full position mode is deducted)
    pub balance: Decimal,
    pub isolated_balance: Decimal,
    /// User settled profit
    pub profit: Decimal,
    /// Total amount of margin used.
    pub margin_total: Decimal,
    /// Total amount of used margin in cross warehouse mode.
    pub margin_cross_total: Decimal,
    /// Total amount of used margin in isolated position mode.
    pub margin_isolated_total: Decimal,
    pub margin_cross_buy_total: Decimal,
    pub margin_cross_sell_total: Decimal,
    pub margin_isolated_buy_total: Decimal,
    pub margin_isolated_sell_total: Decimal,
    pub cross_position_idx: JsonValue,
    pub isolated_position_idx: JsonValue,
}

impl From<Account> for DbAccount {
    fn from(value: Account) -> Self {
        DbAccount {
            id: value.id.to_string(),
            owner: value.owner.to_string(),
            offset_idx: value.offset as i64,
            balance: Decimal::from_i128_with_scale(value.balance as i128, 0),
            isolated_balance: Decimal::from_i128_with_scale(value.isolated_balance as i128, 0),
            profit: Decimal::from_i128_with_scale(value.profit as i128, 0),
            margin_total: Decimal::from_i128_with_scale(value.margin_total as i128, 0),
            margin_cross_total: Decimal::from_i128_with_scale(value.margin_cross_total as i128, 0),
            margin_isolated_total: Decimal::from_i128_with_scale(
                value.margin_isolated_total as i128,
                0,
            ),
            margin_cross_buy_total: Decimal::from_i128_with_scale(
                value.margin_cross_buy_total as i128,
                0,
            ),
            margin_cross_sell_total: Decimal::from_i128_with_scale(
                value.margin_cross_sell_total as i128,
                0,
            ),
            margin_isolated_buy_total: Decimal::from_i128_with_scale(
                value.margin_isolated_buy_total as i128,
                0,
            ),
            margin_isolated_sell_total: Decimal::from_i128_with_scale(
                value.margin_isolated_sell_total as i128,
                0,
            ),
            cross_position_idx: json!(value.cross_position_idx),
            isolated_position_idx: json!(value.isolated_position_idx),
        }
    }
}
impl From<DbAccount> for Account {
    fn from(value: DbAccount) -> Self {
        let cross_position_idx: HashMap<String, Address> =
            serde_json::from_value(value.cross_position_idx).unwrap();
        let isolated_position_idx: Vec<Address> =
            serde_json::from_value(value.isolated_position_idx).unwrap();
        Account {
            id: Address::from_str(value.id.as_str()).unwrap(),
            owner: Address::from_str(value.owner.as_str()).unwrap(),
            offset: value.offset_idx as u64,
            balance: value.balance.mantissa() as u64,
            isolated_balance: value.isolated_balance.mantissa() as u64,
            profit: value.profit.mantissa() as i64,
            margin_total: value.margin_total.mantissa() as u64,
            margin_cross_total: value.margin_cross_total.mantissa() as u64,
            margin_isolated_total: value.margin_isolated_total.mantissa() as u64,
            margin_cross_buy_total: value.margin_cross_buy_total.mantissa() as u64,
            margin_cross_sell_total: value.margin_cross_sell_total.mantissa() as u64,
            margin_isolated_buy_total: value.margin_isolated_buy_total.mantissa() as u64,
            margin_isolated_sell_total: value.margin_isolated_sell_total.mantissa() as u64,
            cross_position_idx: cross_position_idx,
            isolated_position_idx: isolated_position_idx,
        }
    }
}
#[derive(Debug, Clone, Deserialize, Serialize, sqlx::FromRow)]
pub struct DbPosition {
    pub id: String,
    pub offset_idx: i64,
    /// Initial position margin
    pub margin: Decimal,
    /// Current actual margin balance of isolated
    pub margin_balance: Decimal,
    /// leverage size
    pub leverage: i16,
    /// 1 cross position mode, 2 isolated position modes.
    #[serde(rename = "type")]
    pub position_type: i16,
    /// Position status: 1 normal, 2 normal closing, 3 Forced closing, 4 pending , 5 partial closeing , 6 auto closing , 7 merge close
    pub status: i16,
    /// 1 buy long, 2 sell short.
    pub direction: i16,
    /// the position size
    pub unit_size: i64,
    /// lot size
    pub lot: i64,
    /// Opening quotation (expected opening price under the listing mode)
    pub open_price: i64,
    /// Point difference data on which the quotation is based
    pub open_spread: i64,
    // Actual quotation currently obtained
    pub open_real_price: i64,
    /// Closing quotation
    pub close_price: i64,
    /// Point difference data on which the quotation is based
    pub close_spread: i64,
    // Actual quotation currently obtained
    pub close_real_price: i64,
    // PL
    pub profit: i64,
    /// Automatic profit stop price
    pub stop_surplus_price: i64,
    /// Automatic stop loss price
    pub stop_loss_price: i64,
    /// Order creation time
    pub create_time: i64,
    pub open_time: i64,
    pub close_time: i64,
    /// Opening operator (the user manually, or the clearing robot in the listing mode)
    pub open_operator: String,
    /// Account number of warehouse closing operator (user manual, or clearing robot)
    pub close_operator: String,
    /// Market account number of the position
    pub market_id: String,
    pub account_id: String,
    pub symbol: String,
    pub force_close_price: i64,
}
impl From<Position> for DbPosition {
    fn from(value: Position) -> Self {
        DbPosition {
            id: value.id.to_string(),
            offset_idx: value.offset as i64,
            margin: Decimal::from_i128_with_scale(value.margin as i128, 0),
            margin_balance: Decimal::from_i128_with_scale(value.margin_balance as i128, 0),
            leverage: value.leverage as i16,
            position_type: value.position_type as i16,
            status: value.status as i16,
            direction: value.direction as i16,
            unit_size: value.unit_size as i64,
            lot: value.lot as i64,
            open_price: value.open_price as i64,
            open_spread: value.open_spread as i64,
            open_real_price: value.open_real_price as i64,
            close_price: value.close_price as i64,
            close_spread: value.close_spread as i64,
            close_real_price: value.close_real_price as i64,
            profit: value.profit as i64,
            stop_surplus_price: value.stop_surplus_price as i64,
            stop_loss_price: value.stop_loss_price as i64,
            create_time: value.create_time as i64,
            open_time: value.open_time as i64,
            close_time: value.close_time as i64,
            open_operator: value.open_operator.to_string(),
            close_operator: value.close_operator.to_string(),
            market_id: value.market_id.to_string(),
            account_id: value.account_id.to_string(),
            symbol: value.symbol,
            force_close_price: value.force_close_price,
        }
    }
}

impl From<DbPosition> for Position {
    fn from(value: DbPosition) -> Self {
        Position {
            id: Address::from_str(value.id.as_str()).unwrap(),
            offset: value.offset_idx as u64,
            margin: value.margin.mantissa() as u64,
            margin_balance: value.margin_balance.mantissa() as u64,
            leverage: value.leverage as u8,
            position_type: PositionType::try_from(value.position_type as u8).unwrap(),
            status: PositionStatus::try_from(value.status as u8).unwrap(),
            direction: Direction::try_from(value.direction as u8).unwrap(),
            unit_size: value.unit_size as u64,
            lot: value.lot as u64,
            open_price: value.open_price as u64,
            open_spread: value.open_spread as u64,
            open_real_price: value.open_real_price as u64,
            close_price: value.close_price as u64,
            close_spread: value.close_spread as u64,
            close_real_price: value.close_real_price as u64,
            profit: value.profit,
            stop_surplus_price: value.stop_surplus_price as u64,
            stop_loss_price: value.stop_loss_price as u64,
            create_time: value.create_time as u64,
            open_time: value.open_time as u64,
            close_time: value.close_time as u64,
            open_operator: Address::from_str(value.open_operator.as_str()).unwrap(),
            close_operator: Address::from_str(value.close_operator.as_str()).unwrap(),
            market_id: Address::from_str(value.market_id.as_str()).unwrap(),
            account_id: Address::from_str(value.account_id.as_str()).unwrap(),
            symbol: value.symbol,
            force_close_price: value.force_close_price,
        }
    }
}
