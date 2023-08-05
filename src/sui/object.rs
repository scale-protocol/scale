use crate::bot::machine::Message;
use crate::bot::state::{
    Account, Address, Direction, Event, Market, MarketStatus, Officer, Pool, Position,
    PositionStatus, PositionType, State,
};
use crate::com::CliError;
use crate::sui::config::Ctx;
use log::*;
use move_core_types::language_storage::StructTag;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;
use std::str::FromStr;
use sui_json_rpc_types::{SuiObjectData, SuiObjectDataFilter, SuiObjectResponseQuery};
use sui_sdk::rpc_types::{SuiObjectDataOptions, SuiObjectResponse, SuiRawData};
use sui_sdk::types::{
    balance::{Balance, Supply},
    base_types::{ObjectID, ObjectRef, SuiAddress},
    id::{ID, UID},
    object::{Object, Owner},
    transaction::{Argument, CallArg, ObjectArg, TransactionData, TransactionKind},
};
// use sui_types::gas_coin::GasCoin;
use tokio::sync::mpsc::UnboundedSender;
extern crate serde;

const OBJECT_MAX_REQUEST_LIMIT: usize = 100;
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum ObjectType {
    Market,
    Account,
    Position,
    PythPriceUpdate,
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
        } else if value.contains("PriceFeedUpdateEvent") {
            Self::PythPriceUpdate
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
            Self::PythPriceUpdate => "price_update",
            Self::None => "None",
        };
        write!(f, "{}", t)
    }
}

pub async fn get_own_objects_whith_type(
    ctx: Ctx,
    address: SuiAddress,
    t: &str,
) -> anyhow::Result<Vec<SuiObjectResponse>> {
    let mut objects: Vec<SuiObjectResponse> = Vec::new();
    let mut cursor = None;
    loop {
        let response = ctx
            .client
            .read_api()
            .get_owned_objects(
                address,
                Some(SuiObjectResponseQuery::new(
                    Some(SuiObjectDataFilter::StructType(StructTag::from_str(t)?)),
                    Some(SuiObjectDataOptions::full_content()),
                )),
                cursor,
                None,
            )
            .await?;
        objects.extend(response.data);
        if response.has_next_page {
            cursor = response.next_cursor;
        } else {
            break;
        }
    }
    Ok(objects)
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
pub struct ObjectParams(pub BTreeMap<ObjectID, Object>);
impl ObjectParams {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }
    pub fn get_obj(&self, id: ObjectID) -> anyhow::Result<&Object> {
        self.0
            .get(&id)
            .ok_or(CliError::ObjectNotFound(id.to_string()).into())
    }
    pub fn get_ref(&self, id: ObjectID) -> anyhow::Result<ObjectRef> {
        let obj = self.get_obj(id)?;
        Ok(obj.compute_object_reference())
    }
    pub fn get_obj_arg(&self, id: ObjectID, is_mutable_ref: bool) -> anyhow::Result<ObjectArg> {
        let obj = self.get_obj(id)?;
        let owner = obj.owner;
        Ok(match owner {
            Owner::Shared {
                initial_shared_version,
            } => ObjectArg::SharedObject {
                id,
                initial_shared_version,
                mutable: is_mutable_ref,
            },
            Owner::AddressOwner(_) | Owner::ObjectOwner(_) | Owner::Immutable => {
                ObjectArg::ImmOrOwnedObject(obj.compute_object_reference())
            }
        })
    }
}
pub async fn get_object_args(ctx: Ctx, ids: Vec<ObjectID>) -> anyhow::Result<ObjectParams> {
    let rs = ctx
        .client
        .read_api()
        .multi_get_object_with_options(ids, SuiObjectDataOptions::bcs_lossless())
        .await?;
    let mut pm = ObjectParams(BTreeMap::new());
    for r in rs {
        let obj: Object = r.into_object()?.try_into()?;
        let id = obj.id();
        pm.0.insert(id, obj);
    }
    Ok(pm)
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
                        ObjectType::PythPriceUpdate => {}
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
pub struct SuiList {
    pub id: UID,
    /// Market operator,
    /// 1 project team
    /// 2 Certified Third Party
    /// 3 Community
    pub officer: u8,
    /// coin pool of the market
    pub pool: SuiPool,
    pub total: u64,
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
            // pool: m.pool.into(),
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
    /// Current actual margin balance of isolated
    margin_balance: Balance,
    info: Info,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct Info {
    offset: u64,
    /// Initial position margin
    margin: u64,
    /// Current actual margin balance of isolated
    /// leverage size
    leverage: u8,
    /// 1 cross position mode, 2 isolated position modes.
    #[serde(alias = "type")]
    position_type: u8,
    /// Position status: 1 normal, 2 normal closing, 3 Forced closing, 4 pending , 5 partial closeing , 6 auto closing
    status: u8,
    /// 1 buy long, 2 sell short.
    direction: u8,
    /// the position size
    unit_size: u64,
    /// lot size
    lot: u64,
    /// Opening quotation (expected opening price under the listing mode)
    open_price: u64,
    /// Point difference data on which the quotation is based, scale 10000
    open_spread: u64,
    // Actual quotation currently obtained
    open_real_price: u64,
    /// Closing quotation
    close_price: u64,
    /// Point difference data on which the quotation is based , scale 10000
    close_spread: u64,
    // Actual quotation currently obtained
    close_real_price: u64,
    // PL
    profit: I64,
    auto_open_price: u64,
    /// Automatic profit stop price
    stop_surplus_price: u64,
    /// Automatic stop loss price
    stop_loss_price: u64,
    /// Order creation time
    create_time: u64,
    open_time: u64,
    close_time: u64,
    /// The effective time of the order.
    /// If the position is not opened success crossy after this time in the order listing mode,
    /// the order will be closed directly
    validity_time: u64,
    /// Opening operator (the user manually, or the clearing robot in the listing mode)
    open_operator: SuiAddress,
    /// Account number of warehouse closing operator (user manual, or clearing robot Qiangping)
    close_operator: SuiAddress,
    symbol: Vec<u8>,
    /// Market account number of the position
    market_id: ID,
    account_id: ID,
}
impl From<SuiPosition> for Position {
    fn from(p: SuiPosition) -> Self {
        Self {
            id: Address::new(p.id.id.bytes.to_vec()),
            offset: p.info.offset,
            margin: p.info.margin,
            margin_balance: p.margin_balance.value(),
            leverage: p.info.leverage,
            position_type: PositionType::try_from(p.info.position_type).unwrap(),
            status: PositionStatus::try_from(p.info.status).unwrap(),
            direction: Direction::try_from(p.info.direction).unwrap(),
            unit_size: p.info.unit_size,
            lot: p.info.lot,
            open_price: p.info.open_price,
            open_spread: p.info.open_spread,
            open_real_price: p.info.open_real_price,
            close_price: p.info.close_price,
            close_spread: p.info.close_spread,
            close_real_price: p.info.close_real_price,
            profit: p.info.profit.into(),
            stop_surplus_price: p.info.stop_surplus_price,
            stop_loss_price: p.info.stop_loss_price,
            create_time: p.info.create_time,
            open_time: p.info.open_time,
            close_time: p.info.close_time,
            validity_time: p.info.validity_time,
            open_operator: Address::new(p.info.open_operator.to_vec()),
            close_operator: Address::new(p.info.close_operator.to_vec()),
            market_id: Address::new(p.info.market_id.bytes.to_vec()),
            account_id: Address::new(p.info.account_id.bytes.to_vec()),
            symbol: "".to_string(),
            symbol_short: "".to_string(),
            icon: "".to_string(),
        }
    }
}
#[derive(Debug, Deserialize, Serialize)]
pub struct PriceIdentifier {
    bytes: Vec<u8>,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct PriceFeed {
    price_identifier: PriceIdentifier,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct PriceFeedUpdateEvent {
    price_feed: PriceFeed,
    timestamp: u64,
}
