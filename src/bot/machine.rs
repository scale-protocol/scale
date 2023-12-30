use crate::bot::cron::Cron;
use crate::bot::state::{
    Account, Address, Direction, Event, List, Market, Message, MessageReceiver, MessageSender,
    MoveCall, Position, PositionStatus, PositionType, Price, State, Storage, BURST_RATE,
};
use crate::bot::storage::local::{self, Local};
use crate::bot::ws::{
    AccountDynamicData, PositionDynamicData, SpreadData, SupportedSymbol, WsServerState,
    WsSrvMessage, WsWatchTx,
};
use crate::com::{self, Task, TaskStopRx};
use chrono::Utc;
use dashmap::{DashMap, DashSet};
use log::*;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use tokio::{
    sync::{mpsc, oneshot},
    time::{self as tokio_time, Duration as TokioDuration},
};

use super::state;
// key is market Addrsymboless,value is market data
type DmMarket = DashMap<String, Market>;
// key is account Address,value is account data.
type DmAccount = DashMap<Address, Account>;
// key is position Address,value is position  data
type DmPosition = DashMap<Address, Position>;
// key is market symbol ,value is price
type DmPrice = DashMap<String, Price>;
// key is account address Address,value is position k-v map
type DmAccountPosition = DashMap<Address, DmPosition>;
// key is symbol,value is market address set
type DmAccountDynamicData = DashMap<Address, AccountDynamicData>;
type DmPositionDynamicData = DashMap<Address, PositionDynamicData>;
#[derive(Clone)]
pub struct StateMap {
    pub list: Arc<RwLock<List>>,
    pub market: DmMarket,
    pub account: DmAccount,
    pub position: DmAccountPosition,
    pub price: DmPrice,
    pub ws_state: WsServerState,
    pub account_dynamic_data: DmAccountDynamicData,
    pub position_dynamic_data: DmPositionDynamicData,
}
impl StateMap {
    pub fn new(supported_symbol: SupportedSymbol) -> anyhow::Result<Self> {
        let list = Arc::new(RwLock::new(List::default()));
        let market: DmMarket = DashMap::new();
        let account: DmAccount = DashMap::new();
        let position: DmAccountPosition = DashMap::new();
        let price: DmPrice = DashMap::new();
        Ok(Self {
            list,
            market,
            account,
            position,
            price,
            ws_state: WsServerState::new(supported_symbol),
            account_dynamic_data: DashMap::new(),
            position_dynamic_data: DashMap::new(),
        })
    }

    pub fn load_active_account_from_local(&mut self) -> anyhow::Result<()> {
        info!("start load active object from local!");

        Ok(())
    }
}
pub type SharedStateMap = Arc<StateMap>;
pub struct Watch {
    pub watch_tx: MessageSender,
    task: Task,
}
impl Watch {
    pub async fn new<S>(
        ssm: SharedStateMap,
        storage: Arc<S>,
        event_ws_tx: WsWatchTx,
        is_write_ws_event: bool,
    ) -> Self
    where
        S: Storage + Send + Sync + 'static,
    {
        let (watch_tx, watch_rx) = state::new_message_channel();
        let (shutdown_tx, shutdown_rx) = Task::new_shutdown_channel();
        Self {
            watch_tx,
            task: Task::new(
                "watch",
                shutdown_tx,
                tokio::spawn(watch_message(
                    ssm,
                    storage,
                    watch_rx,
                    shutdown_rx,
                    event_ws_tx,
                    is_write_ws_event,
                )),
            ),
        }
    }
    pub async fn shutdown(self) {
        self.task.shutdown().await;
    }
}

async fn watch_message<S>(
    ssm: SharedStateMap,
    storage: Arc<S>,
    mut watch_rx: MessageReceiver,
    mut shutdown_rx: TaskStopRx,
    event_ws_tx: WsWatchTx,
    is_write_spread: bool,
) -> anyhow::Result<()>
where
    S: Storage + Send + Sync + 'static,
{
    info!("start scale data watch ...");
    loop {
        tokio::select! {
            r = &mut shutdown_rx => {
                match r {
                    Ok(_) => {
                        info!("got shutdown signal {:?}, break price broadcast!",r);
                    }
                    Err(e) => {
                        error!("shutdown channel error: {}", e);
                    }
                }
                break;
            }
            r = watch_rx.recv()=> {
                match r {
                    Some(msg)=>{
                        // debug!("data channel got data : {:?}",msg);
                        keep_message(ssm.clone(),storage.clone(), msg,event_ws_tx.clone(),is_write_spread).await;
                    }
                    None=>{
                        debug!("data channel got none : {:?}",r);
                    }
                }
            }
        }
    }
    Ok(())
}

async fn keep_message<S>(
    ssm: SharedStateMap,
    storage: Arc<S>,
    msg: Message,
    event_ws_tx: WsWatchTx,
    is_write_ws_event: bool,
) where
    S: Storage + Send + Sync + 'static,
{
    match msg.state {
        State::List(list) => {
            info!("got list data : {:?}", list);
            let mut l = ssm.list.write().unwrap();
            *l = list;
        }
        State::Market(market) => {
            if msg.event == Event::Deleted {
                ssm.market.remove(&market.symbol);
            } else {
                ssm.market.insert(market.symbol.clone(), market.clone());
            }
            if let Err(e) = storage.save_one(State::Market(market)).await {
                error!("save market error: {}", e);
            }
        }
        State::Account(account) => {
            if msg.event == Event::Deleted {
                ssm.account.remove(&account.id);
            } else {
                ssm.account.insert(account.id.copy(), account.clone());
            }
            if let Err(e) = storage.save_one(State::Account(account)).await {
                error!("save account error: {}", e);
            }
        }
        State::Position(position) => {
            if msg.event == Event::Deleted
                || position.status != PositionStatus::Normal
                || position.status != PositionStatus::Pending
            {
                match ssm.position.get(&position.account_id) {
                    Some(p) => {
                        p.remove(&position.id);
                    }
                    None => {
                        // nothing to do
                    }
                };
            } else {
                match ssm.position.get(&position.account_id) {
                    Some(p) => {
                        p.insert(position.id.clone(), position.clone());
                    }
                    None => {
                        let p: DmPosition = dashmap::DashMap::new();
                        p.insert(position.id.clone(), position.clone());
                        ssm.position.insert((&position.account_id).copy(), p);
                    }
                };
            }
            if let Err(e) = storage.save_one(State::Position(position)).await {
                error!("save position error: {}", e);
            }
        }
        State::Price(org_price) => {
            match ssm.market.get(&org_price.symbol) {
                Some(m) => {
                    let price = m.get_price(org_price.price as u64);
                    ssm.price.insert(org_price.symbol.clone(), price);
                    if is_write_ws_event {
                        let spread_data = SpreadData {
                            symbol: org_price.symbol.clone(),
                            id: m.id.clone(),
                            spread: price.spread / com::DENOMINATOR,
                        };
                        if let Err(e) = event_ws_tx.0.send(WsSrvMessage::SpreadUpdate(spread_data))
                        {
                            error!("send spread data error: {}", e);
                        }
                    }
                }
                None => {
                    error!(
                        "got price data but market not found : {:?}",
                        org_price.symbol
                    );
                    return;
                }
            };
        }
        State::None => {
            debug!("got none data : {:?}", msg);
        }
    }
}
pub struct Liquidation {
    // account_tasks: Task,
    // position_tasks: Vec<Task>,
    // cron: Cron,
}

impl Liquidation {
    pub async fn new<C>(
        ssm: SharedStateMap,
        tasks: usize,
        event_ws_tx: WsWatchTx,
        is_write_ws_event: bool,
        call: Arc<C>,
    ) -> anyhow::Result<Self>
    where
        C: MoveCall + Send + Sync + 'static,
    {
        Ok(Self {})
    }

    pub async fn shutdown(self) {
        debug!("start shutdown liquidation...");
    }
}
