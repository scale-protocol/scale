use std::collections::HashMap;

use chrono::NaiveDateTime;
// use chrono::prelude::*;
use fastcrypto::encoding::{Base58, Base64, Encoding, Hex};
use num_enum::TryFromPrimitive;
use serde::{Deserialize, Serialize};
use std::fmt;

pub const DENOMINATOR: u64 = 10000;
// ID or address of the contract
#[derive(Debug, Deserialize, Serialize, Eq, Default, PartialEq, Ord, PartialOrd, Clone, Hash)]
pub struct Address(Vec<u8>);

impl Address {
    pub fn new(address: Vec<u8>) -> Self {
        Self(address)
    }
}
impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#x}", self)
    }
}
impl fmt::LowerHex for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            write!(f, "0x")?;
        }
        write!(f, "{}", Hex::encode(self))
    }
}
impl AsRef<[u8]> for Address {
    fn as_ref(&self) -> &[u8] {
        &self.0[..]
    }
}
#[derive(Debug)]
pub enum State {
    Market(Market),
    Account(Account),
    Position(Position),
    Price(OrgPrice),
    None,
}
impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let t = match *self {
            Self::Market(_) => "market",
            Self::Account(_) => "account",
            Self::Position(_) => "position",
            Self::Price(_) => "price",
            Self::None => "none",
        };
        write!(f, "{}", t)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Normal,
    Deleted,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Pool {
    // The original supply of the liquidity pool represents
    // the liquidity funds obtained through the issuance of NFT bonds
    pub vault_supply: u64,
    // Token balance of basic current fund.
    pub vault_balance: u64,
    // Token balance of profit and loss fund
    pub profit_balance: u64,
    // Insurance fund token balance
    pub insurance_balance: u64,
    // Spread benefits, to prevent robot cheating and provide benefits to sponsors
    pub spread_profit: u64,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Market {
    pub id: Address,
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
    pub pyth_id: Address,
}
impl Market {
    pub fn get_price(&self, real_price: u64) -> Price {
        let spread = self.get_spread_fee(real_price) * real_price / DENOMINATOR;
        // To increase the calculation accuracy
        let half_spread = spread * DENOMINATOR / 2;
        Price {
            buy_price: (real_price * DENOMINATOR + half_spread) / DENOMINATOR,
            sell_price: (real_price * DENOMINATOR - half_spread) / DENOMINATOR,
            real_price: real_price,
            spread: spread as u64,
            update_time: chrono::Utc::now().timestamp(),
        }
    }
    pub fn get_spread_fee(&self, real_price: u64) -> u64 {
        if self.spread_fee_manual {
            return self.spread_fee;
        };
        let change_price = real_price.max(self.opening_price) - real_price.min(self.opening_price);
        let change = change_price / self.opening_price * DENOMINATOR;
        if change <= 300 {
            return 30;
        };
        if change > 300 && change <= 1000 {
            return change / 10;
        };
        return 150;
    }
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
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Account {
    pub id: Address,
    pub owner: Address,
    /// The position offset.
    /// like order id
    pub offset: u64,
    /// Balance of user account (maintain the deposit,
    /// and the balance here will be deducted when the deposit used in the full position mode is deducted)
    pub balance: u64,
    /// User settled profit
    pub profit: i64,
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
    pub full_position_idx: HashMap<Vec<u8>, Address>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Position {
    pub id: Address,
    pub offset: u64,
    /// Initial position margin
    pub margin: u64,
    /// Current actual margin balance of independent
    pub margin_balance: u64,
    /// leverage size
    pub leverage: u8,
    /// 1 full position mode, 2 independent position modes.
    #[serde(rename = "type")]
    pub position_type: PositionType,
    /// Position status: 1 normal, 2 normal closing, 3 Forced closing, 4 pending.
    pub status: PositionStatus,
    /// 1 buy long, 2 sell short.
    pub direction: Direction,
    /// the position size
    pub size: u64,
    /// lot size
    pub lot: u64,
    /// Opening quotation (expected opening price under the listing mode)
    pub open_price: u64,
    /// Point difference data on which the quotation is based
    pub open_spread: u64,
    // Actual quotation currently obtained
    pub open_real_price: u64,
    /// Closing quotation
    pub close_price: u64,
    /// Point difference data on which the quotation is based
    pub close_spread: u64,
    // Actual quotation currently obtained
    pub close_real_price: u64,
    // PL
    pub profit: i64,
    /// Automatic profit stop price
    pub stop_surplus_price: u64,
    /// Automatic stop loss price
    pub stop_loss_price: u64,
    /// Order creation time
    pub create_time: u64,
    pub open_time: u64,
    pub close_time: u64,
    /// The effective time of the order.
    /// If the position is not opened successfully after this time in the order listing mode,
    /// the order will be closed directly
    pub validity_time: u64,
    /// Opening operator (the user manually, or the clearing robot in the listing mode)
    pub open_operator: Address,
    /// Account number of warehouse closing operator (user manual, or clearing robot)
    pub close_operator: Address,
    /// Market account number of the position
    pub market_id: Address,
    pub account_id: Address,
}
#[derive(Clone, Debug, TryFromPrimitive, PartialEq, Deserialize, Serialize)]
#[repr(u8)]
pub enum PositionStatus {
    Normal = 1,
    NormalClosing,
    ForcedClosing,
    Pending,
}
#[derive(Clone, Debug, TryFromPrimitive, PartialEq, Deserialize, Serialize)]
#[repr(u8)]
pub enum PositionType {
    Full = 1,
    Independent,
}
#[derive(
    Clone, Debug, TryFromPrimitive, PartialEq, Deserialize, Serialize, Eq, Ord, PartialOrd,
)]
#[repr(u8)]
pub enum Direction {
    Buy = 1,
    Sell,
}
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct Price {
    pub buy_price: u64,
    pub sell_price: u64,
    pub real_price: u64,
    pub spread: u64,
    pub update_time: i64,
}
#[derive(Debug, Clone, Copy)]
pub struct OrgPrice {
    pub price: u64,
}
