use crate::bot::state::{self, Account, Market, Position, UserAccount};
use crate::sui::config::Context;
use crate::{app::Task, com::CliError};
use log::*;
use num_enum::TryFromPrimitive;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;
use std::time::Duration;
use sui_sdk::rpc_types::{SuiEvent, SuiEventFilter, SuiObjectRead, SuiRawData};
use sui_sdk::types::{
    balance::{Balance, Supply},
    base_types::{ObjectID, SuiAddress},
    id::{ID, UID},
};
extern crate serde;
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum ObjectType {
    Market,
    UserAccount,
    Account,
    Position,
    None,
}
impl<'a> From<&'a str> for ObjectType {
    fn from(value: &'a str) -> Self {
        if value.starts_with("::market::Market") {
            Self::Market
        } else if value.contains("::account::Account") {
            Self::Account
        } else if value.contains("::account::UserAccount") {
            Self::UserAccount
        } else if value.contains("::position::Position") {
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
            Self::UserAccount => "user account",
            Self::Position => "position",
            Self::None => "None",
        };
        write!(f, "{}", t)
    }
}
pub async fn get_object(ctx: Arc<Context>, id: ObjectID) -> anyhow::Result<()> {
    let rs = ctx.client.read_api().get_object(id).await?;
    match rs {
        SuiObjectRead::Exists(o) => match o.data {
            SuiRawData::MoveObject(m) => {
                let t: ObjectType = m.type_.as_str().into();
                debug!("got move object data type: {:?}", t);
                match t {
                    ObjectType::Market => {
                        let market: SuiMarket = m.deserialize()?;
                        println!("market: {:?}", market);
                    }
                    ObjectType::Account => {
                        let account: SuiAccount = m.deserialize()?;
                        println!("account: {:?}", account);
                    }
                    ObjectType::UserAccount => {
                        let account: SuiUserAccount = m.deserialize()?;
                        println!("user account: {:?}", account);
                    }
                    ObjectType::Position => {
                        let position: SuiPosition = m.deserialize()?;
                        println!("position: {:?}", position);
                    }
                    ObjectType::None => {
                        println!("None");
                    }
                }
            }
            SuiRawData::Package(p) => {
                debug!("got package: {:?}", p);
            }
        },
        SuiObjectRead::NotExists(id) => {
            warn!("Object not exists: {:?}", id);
        }
        SuiObjectRead::Deleted(s) => {
            warn!("Object deleted: {:?}", s);
        }
    }
    Ok(())
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
    pub status: MarketStatus,
    /// Total amount of long positions in the market
    pub long_position_total: u64,
    /// Total amount of short positions in the market
    pub short_position_total: u64,
    /// Transaction pair (token type, such as BTC, ETH)
    /// len: 4+20
    pub name: String,
    /// market description
    pub description: String,
    /// Market operator,
    /// 1 project team
    /// 2 Certified Third Party
    /// 3 Community
    pub officer: Officer,
    /// coin pool of the market
    pub pool: Pool,
    /// Basic size of transaction pair contract
    /// Constant 1 in the field of encryption
    pub size: u64,
    /// The price at 0 o'clock in the utc of the current day, which is used to calculate the spread_fee
    pub opening_price: u64,
    pub pyth_id: ID,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct Pool {
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
#[derive(Clone, Debug, TryFromPrimitive, PartialEq, Deserialize, Serialize)]
#[repr(u8)]
pub enum MarketStatus {
    Normal = 1,
    Locked,
    Frozen,
}
#[derive(Clone, Debug, TryFromPrimitive, PartialEq, Deserialize, Serialize)]
#[repr(u8)]
pub enum Officer {
    ProjectTeam = 1,
    CertifiedThirdParty,
    Community,
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
    /// Total amount of used margin in independent position mode.
    pub margin_independent_total: u64,
    pub margin_full_buy_total: u64,
    pub margin_full_sell_total: u64,
    pub margin_independent_buy_total: u64,
    pub margin_independent_sell_total: u64,
    pub full_position_idx: Vec<Entry>,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct PFK {
    pub market_id: ID,
    pub account_id: ID,
    pub direction: u8,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct Entry {
    pub key: PFK,
    pub value: ID,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct I64 {
    pub negative: bool,
    pub value: u64,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct SuiPosition {
    id: UID,
    offset: u64,
    /// Initial position margin
    margin: u64,
    /// Current actual margin balance of independent
    margin_balance: Balance,
    /// leverage size
    leverage: u8,
    /// 1 full position mode, 2 independent position modes.
    #[serde(alias = "type")]
    position_type: u8,
    /// Position status: 1 normal, 2 normal closing, 3 Forced closing, 4 pending.
    status: u8,
    /// 1 buy long, 2 sell short.
    direction: u8,
    /// the position size
    size: u64,
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
