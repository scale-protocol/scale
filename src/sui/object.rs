use crate::bot::state::{self, Account, Market, Position, UserAccount};
use crate::sui::config::Context;
use crate::{app::Task, com::CliError};
use log::*;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;
use std::time::Duration;
use sui_sdk::rpc_types::{SuiEvent, SuiEventFilter, SuiObjectRead, SuiRawData};
use sui_sdk::types::{
    base_types::{ObjectID, SuiAddress},
    id::{ID, UID},
};
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
        debug!("value-------------------->: {}", value);
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
                        let market: Market = m.deserialize()?;
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
                        let position: Position = m.deserialize()?;
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
