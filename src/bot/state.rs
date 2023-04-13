use crate::com;
use anyhow::anyhow;
use async_trait::async_trait;
use fastcrypto::encoding::{decode_bytes_hex, Encoding, Hex};
use num_enum::TryFromPrimitive;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{self, Error};
use std::str::FromStr;

pub const DENOMINATOR: u64 = 10000;
pub const BURST_RATE: f64 = 0.5;

// ID or address of the contract
#[derive(Debug, Eq, Default, PartialEq, Ord, PartialOrd, Clone, Hash)]
pub struct Address(Vec<u8>);

impl Serialize for Address {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.to_string().as_str())
    }
}

impl<'de> Deserialize<'de> for Address {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Address::from_str(s.as_str()).map_err(|e| serde::de::Error::custom(e))
    }
}

impl Address {
    pub fn new(address: Vec<u8>) -> Self {
        Self(address)
    }
    pub fn copy(&self) -> Self {
        Self(self.0.clone())
    }
    pub fn to_vec(&self) -> Vec<u8> {
        self.0.clone()
    }
}

impl FromStr for Address {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // if s.split_at(2).0 == "0x" {}
        decode_bytes_hex(s).map_err(|e| anyhow!(e))
    }
}
impl TryFrom<Vec<u8>> for Address {
    type Error = anyhow::Error;

    fn try_from(bytes: Vec<u8>) -> Result<Address, anyhow::Error> {
        Ok(Self(bytes))
    }
}
impl TryFrom<&[u8]> for Address {
    type Error = Error;

    fn try_from(bytes: &[u8]) -> Result<Self, Error> {
        Ok(Self(bytes.to_vec()))
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
#[derive(Debug, Clone, Deserialize, Serialize)]
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
pub enum Event {
    Created,
    Updated,
    Deleted,
    None,
}
impl<'a> From<&'a str> for Event {
    fn from(value: &'a str) -> Self {
        if value.contains("Created") {
            Self::Created
        } else if value.contains("Updated") {
            Self::Updated
        } else if value.contains("Deleted") {
            Self::Deleted
        } else {
            Self::None
        }
    }
}
impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let t = match *self {
            Self::Created => "Created",
            Self::Updated => "Updated",
            Self::Deleted => "Deleted",
            Self::None => "None",
        };
        write!(f, "{}", t)
    }
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
    pub symbol: String,
    pub symbol_short: String,
    pub icon: String,
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
    pub unit_size: u64,
    /// The price at 0 o'clock in the utc of the current day, which is used to calculate the spread_fee
    pub opening_price: u64,
    pub pyth_id: Address,
}
impl Market {
    pub fn get_price(&self, real_price: u64) -> Price {
        let spread = self.get_spread_fee(real_price) * real_price;
        // To increase the calculation accuracy
        let half_spread = spread / 2;
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
        if self.opening_price == 0 {
            return 150;
        };
        let change = change_price * DENOMINATOR / self.opening_price;
        if change <= 300 {
            return 30;
        };
        if change > 300 && change <= 1000 {
            return change / 10;
        };
        return 150;
    }
    // 1 buy
    // 2 sell
    // 3 Flat
    pub fn get_dominant_direction(&self) -> Direction {
        if self.long_position_total == self.short_position_total {
            Direction::Flat
        } else if self.long_position_total > self.short_position_total {
            Direction::Buy
        } else {
            Direction::Sell
        }
    }
    pub fn get_exposure(&self) -> u64 {
        if self.short_position_total > self.long_position_total {
            self.short_position_total - self.long_position_total
        } else {
            self.long_position_total - self.short_position_total
        }
    }
    pub fn get_total_liquidity(&self) -> u64 {
        self.pool.vault_balance + self.pool.profit_balance
    }
    pub fn get_fund_fee(&self) -> u64 {
        if self.fund_fee_manual {
            return self.fund_fee;
        };
        let total_liquidity = self.get_total_liquidity();
        let exposure = self.get_exposure();
        if exposure == 0 || total_liquidity == 0 {
            return 0;
        };
        let exposure_rate = exposure * DENOMINATOR / total_liquidity;
        if exposure_rate <= 1000 {
            return 3;
        };
        if exposure_rate > 1000 && exposure_rate <= 2000 {
            return 5;
        };
        if exposure_rate > 2000 && exposure_rate <= 3000 {
            return 7;
        };
        if exposure_rate > 3000 && exposure_rate <= 4000 {
            return 10;
        };
        if exposure_rate > 4000 && exposure_rate <= 5000 {
            return 20;
        };
        if exposure_rate > 5000 && exposure_rate <= 6000 {
            return 40;
        };
        return 70;
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
    /// Total amount of used margin in isolated position mode.
    pub margin_isolated_total: u64,
    pub margin_full_buy_total: u64,
    pub margin_full_sell_total: u64,
    pub margin_isolated_buy_total: u64,
    pub margin_isolated_sell_total: u64,
    pub full_position_idx: HashMap<String, Address>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Position {
    pub id: Address,
    pub offset: u64,
    /// Initial position margin
    pub margin: u64,
    /// Current actual margin balance of isolated
    pub margin_balance: u64,
    /// leverage size
    pub leverage: u8,
    /// 1 cross position mode, 2 isolated position modes.
    #[serde(rename = "type")]
    pub position_type: PositionType,
    /// Position status: 1 normal, 2 normal closing, 3 Forced closing, 4 pending.
    pub status: PositionStatus,
    /// 1 buy long, 2 sell short.
    pub direction: Direction,
    /// the position size
    pub unit_size: u64,
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
    pub symbol: String,
    pub symbol_short: String,
    pub icon: String,
}

impl Position {
    pub fn get_fund_size(&self) -> u64 {
        Self::fund_size(self.unit_size, self.lot, self.open_real_price)
    }

    fn fund_size(size: u64, lot: u64, price: u64) -> u64 {
        size * (lot / com::DENOMINATOR128) * price
    }

    pub fn get_size(&self) -> u64 {
        Self::size(self.lot, self.unit_size)
    }

    fn size(lot: u64, size: u64) -> u64 {
        size * (lot / com::DENOMINATOR128)
    }

    pub fn get_margin_size(&self, market: &Market) -> u64 {
        Self::margin_size(
            self.get_fund_size(),
            self.leverage as u64,
            market.margin_fee,
        )
    }

    fn margin_size(fund_size: u64, leverage: u64, margin_fee: u64) -> u64 {
        fund_size / leverage * margin_fee / DENOMINATOR
    }

    /// get Floating P/L
    pub fn get_pl(&self, price: &Price) -> i64 {
        if self.direction == Direction::Buy {
            Self::fund_size(self.unit_size, self.lot, price.sell_price) as i64
                - self.get_fund_size() as i64
        } else {
            self.get_fund_size() as i64
                - Self::fund_size(self.unit_size, self.lot, price.buy_price) as i64
        }
    }

    pub fn get_position_fund_fee(&self, market: &Market) -> i64 {
        let dominant_direction = market.get_dominant_direction();
        if dominant_direction == Direction::Flat {
            return 0;
        };
        if self.direction == dominant_direction {
            -((self.get_fund_size() * market.get_fund_fee() / com::DENOMINATOR) as i64)
        } else {
            let max = market.long_position_total.max(market.short_position_total);
            let min = market.long_position_total.min(market.short_position_total);
            if min == 0 {
                return 0;
            }
            // todo check overflow
            let r = max * market.get_fund_fee() / com::DENOMINATOR * self.get_fund_size() / min;
            r as i64
        }
    }
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
    Cross = 1,
    Isolated,
}
#[derive(
    Clone, Debug, Copy, TryFromPrimitive, PartialEq, Deserialize, Serialize, Eq, Ord, PartialOrd,
)]
#[repr(u8)]
pub enum Direction {
    Buy = 1,
    Sell,
    #[serde(skip_serializing, skip_deserializing)]
    Flat,
}
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct Price {
    pub buy_price: u64,
    pub sell_price: u64,
    pub real_price: u64,
    pub spread: u64,
    pub update_time: i64,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OrgPrice {
    pub price: i64,
    pub update_time: i64,
    pub symbol: String,
}
#[async_trait]
pub trait MoveCall {
    async fn trigger_update_opening_price(&self, market_id: Address) -> anyhow::Result<()>;
    async fn burst_position(&self, account_id: Address, position_id: Address)
        -> anyhow::Result<()>;
    async fn process_fund_fee(&self, account_id: Address) -> anyhow::Result<()>;
    async fn update_price(&self, feed: &str, price: u64) -> anyhow::Result<()>;
}
#[cfg(test)]
mod tests {
    // use super::*;
    #[test]
    fn test_str_to_address() {}
}
