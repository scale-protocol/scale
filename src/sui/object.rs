use crate::bot::machine::Message;
use crate::bot::state::{
    Account, Address, Direction, Event, Market, MarketStatus, Officer, Pool, Position,
    PositionStatus, PositionType, State,
};
use crate::com::CliError;
use crate::sui::config::Ctx;
use log::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;
use sui_sdk::rpc_types::{SuiObjectDataOptions, SuiObjectResponse, SuiRawData};
use sui_sdk::types::{
    balance::{Balance, Supply},
    base_types::{ObjectID, SuiAddress},
    id::{ID, UID},
};
use tokio::sync::mpsc::UnboundedSender;
extern crate serde;

const OBJECT_MAX_REQUEST_LIMIT: usize = 100;
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum ObjectType {
    Market,
    Account,
    Position,
    None,
}
impl<'a> From<&'a str> for ObjectType {
    fn from(value: &'a str) -> Self {
        if value.contains("Market") {
            Self::Market
        } else if value.contains("Account") {
            Self::Account
        } else if value.contains("Position") {
            Self::Position
        } else {
            Self::None
        }
    }
}
impl fmt::Display for ObjectType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let t = match *self {
            Self::Market => "market",
            Self::Account => "account",
            Self::Position => "position",
            Self::None => "None",
        };
        write!(f, "{}", t)
    }
}

pub async fn pull_objects_and_send(
    ctx: Ctx,
    mut ids: Vec<ObjectID>,
    event: Event,
    watch_tx: UnboundedSender<Message>,
) -> anyhow::Result<()> {
    while ids.len() > OBJECT_MAX_REQUEST_LIMIT {
        let ids_new = ids.split_off(OBJECT_MAX_REQUEST_LIMIT);
        pull_objects_whith_limit_and_send(ctx.clone(), ids_new, event.clone(), watch_tx.clone())
            .await?;
    }
    pull_objects_whith_limit_and_send(ctx, ids, event.clone(), watch_tx).await?;
    Ok(())
}

pub async fn pull_objects_whith_limit_and_send(
    ctx: Ctx,
    ids: Vec<ObjectID>,
    event: Event,
    watch_tx: UnboundedSender<Message>,
) -> anyhow::Result<()> {
    if ids.len() > OBJECT_MAX_REQUEST_LIMIT || ids.len() == 0 {
        return Ok(());
    }
    let rs = ctx
        .client
        .read_api()
        .multi_get_object_with_options(
            ids,
            SuiObjectDataOptions {
                show_type: false,
                show_owner: false,
                show_previous_transaction: false,
                show_display: false,
                show_content: false,
                show_bcs: true,
                show_storage_rebate: false,
            },
        )
        .await?;
    for r in rs {
        let mut ev = prase_object_response(r).await?;
        ev.event = event.clone();
        if let Err(e) = watch_tx.send(ev) {
            error!("send message error: {:?}", e);
        }
    }
    Ok(())
}

pub async fn pull_object(ctx: Ctx, id: ObjectID) -> anyhow::Result<Message> {
    let opt = SuiObjectDataOptions {
        show_type: false,
        show_owner: false,
        show_previous_transaction: false,
        show_display: false,
        show_content: false,
        show_bcs: true,
        show_storage_rebate: false,
    };
    let rs = ctx
        .client
        .read_api()
        .get_object_with_options(id, opt)
        .await?;
    prase_object_response(rs).await
}

pub async fn prase_object_response(rs: SuiObjectResponse) -> anyhow::Result<Message> {
    if let Some(e) = rs.error {
        error!("get object error: {:?}", e);
        return Err(CliError::GetObjectError(e.to_string()).into());
    }
    debug!("get object: {:?}", rs);
    if let Some(data) = rs.data {
        if let Some(bcs) = data.bcs {
            match bcs {
                SuiRawData::MoveObject(m) => {
                    let t: ObjectType = m.type_.clone().name.into_string().as_str().into();
                    debug!("got move object data type: {:?}", t);
                    match t {
                        ObjectType::Market => {
                            let sui_market: SuiMarket = m.deserialize()?;
                            debug!("market: {:?}", sui_market);
                            let market = Market::from(sui_market);
                            return Ok(Message {
                                address: market.id.clone(),
                                state: State::Market(market),
                                event: Event::None,
                            });
                        }
                        ObjectType::Account => {
                            let sui_account: SuiAccount = m.deserialize()?;
                            debug!("account: {:?}", sui_account);
                            let account = Account::from(sui_account);
                            return Ok(Message {
                                address: account.id.clone(),
                                state: State::Account(account),
                                event: Event::None,
                            });
                        }
                        ObjectType::Position => {
                            let sui_position: SuiPosition = m.deserialize()?;
                            debug!("position: {:?}", sui_position);
                            let position = Position::from(sui_position);
                            return Ok(Message {
                                address: position.id.clone(),
                                state: State::Position(position),
                                event: Event::None,
                            });
                        }
                        ObjectType::None => {
                            error!("got none object type");
                        }
                    }
                }
                _ => {
                    error!("got none move object data");
                }
            }
        }
    }
    return Err(CliError::GetObjectError("Unresolved object information".to_string()).into());
}
#[derive(Debug, Deserialize, Serialize)]
pub struct SuiMarket {
    pub id: UID,
    /// Maximum allowable leverage ratio
    pub max_leverage: u8,
    /// insurance rate
    pub insurance_fee: u64,
    /// margin rate,Current constant positioning 100%
    pub margin_fee: u64,
    /// The position fund rate will be calculated automatically according to the rules,
    /// and this value will be used when manually set
    pub fund_fee: u64,
    /// Take the value of fund_fee when this value is true
    pub fund_fee_manual: bool,
    /// Point difference (can be understood as slip point),
    /// deviation between the executed quotation and the actual quotation
    pub spread_fee: u64,
    /// Take the value of spread_fee when this value is true
    pub spread_fee_manual: bool,
    /// Market status:
    /// 1 Normal;
    /// 2. Lock the market, allow closing settlement and not open positions;
    /// 3 The market is frozen, and opening and closing positions are not allowed.
    pub status: u8,
    /// Total amount of long positions in the market
    pub long_position_total: u64,
    /// Total amount of short positions in the market
    pub short_position_total: u64,
    /// Transaction pair (token type, such as BTC, ETH)
    /// len: 4+20
    pub symbol: String,
    pub icon: String,
    /// market description
    pub description: String,
    /// Market operator,
    /// 1 project team
    /// 2 Certified Third Party
    /// 3 Community
    pub officer: u8,
    /// coin pool of the market
    pub pool: SuiPool,
    /// Basic size of transaction pair contract
    /// Constant 1 in the field of encryption
    pub unit_size: u64,
    /// The price at 0 o'clock in the utc of the current day, which is used to calculate the spread_fee
    pub opening_price: u64,
    pub pyth_id: ID,
}

impl From<SuiMarket> for Market {
    fn from(m: SuiMarket) -> Self {
        Self {
            id: Address::new(m.id.id.bytes.to_vec()),
            max_leverage: m.max_leverage,
            insurance_fee: m.insurance_fee,
            margin_fee: m.margin_fee,
            fund_fee: m.fund_fee,
            fund_fee_manual: m.fund_fee_manual,
            spread_fee: m.spread_fee,
            spread_fee_manual: m.spread_fee_manual,
            status: MarketStatus::try_from(m.status).unwrap(),
            long_position_total: m.long_position_total,
            short_position_total: m.short_position_total,
            symbol: m.symbol.clone(),
            symbol_short: m.symbol.replace("Crypto.", "").replace("/", "-"),
            icon: m.icon,
            description: m.description,
            officer: Officer::try_from(m.officer).unwrap(),
            pool: m.pool.into(),
            unit_size: m.unit_size,
            opening_price: m.opening_price,
            pyth_id: Address::new(m.pyth_id.bytes.to_vec()),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SuiPool {
    // The original supply of the liquidity pool represents
    // the liquidity funds obtained through the issuance of NFT bonds
    vault_supply: Supply,
    // Token balance of basic current fund.
    vault_balance: Balance,
    // Token balance of profit and loss fund
    profit_balance: Balance,
    // Insurance fund token balance
    insurance_balance: Balance,
    // Spread benefits, to prevent robot cheating and provide benefits to sponsors
    spread_profit: Balance,
}

impl From<SuiPool> for Pool {
    fn from(p: SuiPool) -> Self {
        Self {
            vault_supply: p.vault_supply.value,
            vault_balance: p.vault_balance.value(),
            profit_balance: p.profit_balance.value(),
            insurance_balance: p.insurance_balance.value(),
            spread_profit: p.spread_profit.value(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SuiUserAccount {
    pub id: UID,
    pub owner: SuiAddress,
    pub account_id: TypedID,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TypedID {
    pub id: ID,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct SuiAccount {
    pub id: UID,
    pub owner: SuiAddress,
    /// The position offset.
    /// like order id
    pub offset: u64,
    /// Balance of user account (maintain the deposit,
    /// and the balance here will be deducted when the deposit used in the full position mode is deducted)
    pub balance: u64,
    /// User settled profit
    pub profit: I64,
    /// Total amount of margin used.
    pub margin_total: u64,
    /// Total amount of used margin in full warehouse mode.
    pub margin_full_total: u64,
    /// Total amount of used margin in isolated position mode.
    pub margin_isolated_total: u64,
    pub margin_full_buy_total: u64,
    pub margin_full_sell_total: u64,
    pub margin_isolated_buy_total: u64,
    pub margin_isolated_sell_total: u64,
    pub full_position_idx: Vec<Entry>,
    pub isolated_position_idx: Vec<SuiAddress>,
}

impl From<SuiAccount> for Account {
    fn from(a: SuiAccount) -> Self {
        let mut full_position_idx: HashMap<String, Address> = HashMap::new();
        for e in a.full_position_idx {
            let (key, value): (String, Address) = e.into();
            full_position_idx.insert(key, value);
        }
        Self {
            id: Address::new(a.id.id.bytes.to_vec()),
            owner: Address::new(a.owner.to_vec()),
            offset: a.offset,
            balance: a.balance,
            profit: a.profit.into(),
            margin_total: a.margin_total,
            margin_full_total: a.margin_full_total,
            margin_isolated_total: a.margin_isolated_total,
            margin_full_buy_total: a.margin_full_buy_total,
            margin_full_sell_total: a.margin_full_sell_total,
            margin_isolated_buy_total: a.margin_isolated_buy_total,
            margin_isolated_sell_total: a.margin_isolated_sell_total,
            full_position_idx,
        }
    }
}
#[derive(Debug, Deserialize, Serialize)]
pub struct PFK {
    pub market_id: ID,
    pub account_id: ID,
    pub direction: u8,
}

impl From<PFK> for String {
    fn from(p: PFK) -> Self {
        format!(
            "{}-{}-{}",
            p.market_id.bytes.to_string(),
            p.account_id.bytes.to_string(),
            p.direction
        )
    }
}
#[derive(Debug, Deserialize, Serialize)]
pub struct Entry {
    pub key: PFK,
    pub value: ID,
}

impl From<Entry> for (String, Address) {
    fn from(e: Entry) -> Self {
        (e.key.into(), Address::new(e.value.bytes.to_vec()))
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct I64 {
    pub negative: bool,
    pub value: u64,
}
impl From<I64> for i64 {
    fn from(i: I64) -> Self {
        if i.negative {
            -(i.value as i64)
        } else {
            i.value as i64
        }
    }
}
#[derive(Debug, Deserialize, Serialize)]
pub struct SuiPosition {
    id: UID,
    offset: u64,
    /// Initial position margin
    margin: u64,
    /// Current actual margin balance of isolated
    margin_balance: Balance,
    /// leverage size
    leverage: u8,
    /// 1 full position mode, 2 isolated position modes.
    #[serde(alias = "type")]
    position_type: u8,
    /// Position status: 1 normal, 2 normal closing, 3 Forced closing, 4 pending.
    status: u8,
    /// 1 buy long, 2 sell short.
    direction: u8,
    /// the position size
    unit_size: u64,
    /// lot size
    lot: u64,
    /// Opening quotation (expected opening price under the listing mode)
    open_price: u64,
    /// Point difference data on which the quotation is based
    open_spread: u64,
    // Actual quotation currently obtained
    open_real_price: u64,
    /// Closing quotation
    close_price: u64,
    /// Point difference data on which the quotation is based
    close_spread: u64,
    // Actual quotation currently obtained
    close_real_price: u64,
    // PL
    profit: I64,
    /// Automatic profit stop price
    stop_surplus_price: u64,
    /// Automatic stop loss price
    stop_loss_price: u64,
    /// Order creation time
    create_time: u64,
    open_time: u64,
    close_time: u64,
    /// The effective time of the order.
    /// If the position is not opened successfully after this time in the order listing mode,
    /// the order will be closed directly
    validity_time: u64,
    /// Opening operator (the user manually, or the clearing robot in the listing mode)
    open_operator: SuiAddress,
    /// Account number of warehouse closing operator (user manual, or clearing robot)
    close_operator: SuiAddress,
    /// Market account number of the position
    market_id: ID,
    account_id: ID,
}
impl From<SuiPosition> for Position {
    fn from(p: SuiPosition) -> Self {
        Self {
            id: Address::new(p.id.id.bytes.to_vec()),
            offset: p.offset,
            margin: p.margin,
            margin_balance: p.margin_balance.value(),
            leverage: p.leverage,
            position_type: PositionType::try_from(p.position_type).unwrap(),
            status: PositionStatus::try_from(p.status).unwrap(),
            direction: Direction::try_from(p.direction).unwrap(),
            unit_size: p.unit_size,
            lot: p.lot,
            open_price: p.open_price,
            open_spread: p.open_spread,
            open_real_price: p.open_real_price,
            close_price: p.close_price,
            close_spread: p.close_spread,
            close_real_price: p.close_real_price,
            profit: p.profit.into(),
            stop_surplus_price: p.stop_surplus_price,
            stop_loss_price: p.stop_loss_price,
            create_time: p.create_time,
            open_time: p.open_time,
            close_time: p.close_time,
            validity_time: p.validity_time,
            open_operator: Address::new(p.open_operator.to_vec()),
            close_operator: Address::new(p.close_operator.to_vec()),
            market_id: Address::new(p.market_id.bytes.to_vec()),
            account_id: Address::new(p.account_id.bytes.to_vec()),
            symbol: "".to_string(),
            symbol_short: "".to_string(),
            icon: "".to_string(),
        }
    }
}
