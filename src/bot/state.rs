use num_enum::TryFromPrimitive;
use serde::{Deserialize, Serialize};
use sui_sdk::types::base_types::{ObjectID, SuiAddress, TransactionDigest};
#[derive(Debug, Deserialize, Serialize)]
pub struct AccountAddress([u8; AccountAddress::LENGTH]);
impl AccountAddress {
    pub const LENGTH: usize = 32;
    pub fn new(address: [u8; AccountAddress::LENGTH]) -> Self {
        Self(address)
    }
    pub fn to_bytes(&self) -> [u8; AccountAddress::LENGTH] {
        self.0
    }
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != AccountAddress::LENGTH {
            return None;
        }
        let mut address = [0u8; AccountAddress::LENGTH];
        address.copy_from_slice(bytes);
        Some(Self(address))
    }
}
#[derive(Debug, Deserialize, Serialize)]
pub struct Pool {
    // The original supply of the liquidity pool represents
    // the liquidity funds obtained through the issuance of NFT bonds
    vault_supply: u64,
    // Token balance of basic current fund.
    vault_balance: u64,
    // Token balance of profit and loss fund
    profit_balance: u64,
    // Insurance fund token balance
    insurance_balance: u64,
    // Spread benefits, to prevent robot cheating and provide benefits to sponsors
    spread_profit: u64,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct Market {
    pub id: AccountAddress,
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
    pub pyth_id: AccountAddress,
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
pub struct Account {
    pub id: AccountAddress,
    pub owner: AccountAddress,
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
    pub full_position_idx: Vec<Entry>,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct PFK {
    pub market_id: AccountAddress,
    pub account_id: AccountAddress,
    pub direction: u8,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct Entry {
    pub key: PFK,
    pub value: AccountAddress,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Position {}

#[derive(Debug, Deserialize, Serialize)]
pub struct UserAccount {
    pub id: SuiAddress,
    pub owner: SuiAddress,
    pub account_id: TypedID,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct TypedID {
    pub type_id: u8,
    pub id: SuiAddress,
}
