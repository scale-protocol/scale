use crate::bot::cron::Cron;
use crate::bot::state::{
    Account, Address, Direction, Event, Market, Message, MessageReceiver, MessageSender, MoveCall,
    Position, PositionStatus, PositionType, Price, State, Storage, BURST_RATE,
};
use crate::bot::storage::local::{self, Local};
use crate::bot::ws::{
    AccountDynamicData, PositionDynamicData, SpreadData, SupportedSymbol, WsServerState,
    WsSrvMessage, WsWatchTx,
};
use crate::com::{self, Task};
use chrono::Utc;
use dashmap::{DashMap, DashSet};
use log::*;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tokio::{
    sync::{mpsc, oneshot},
    time::{self as tokio_time, Duration as TokioDuration},
};

// key is market Address,value is market data
type DmMarket = DashMap<Address, Market>;
// key is account Address,value is account data.
type DmAccount = DashMap<Address, Account>;
// key is position Address,value is position  data
type DmPosition = DashMap<Address, Position>;
// key is market address ,value is price
type DmPrice = DashMap<String, Price>;
// key is account address Address,value is position k-v map
type DmAccountPosition = DashMap<Address, DmPosition>;
// key is symbol,value is market address set
type DmAccountDynamicData = DashMap<Address, AccountDynamicData>;
type DmPositionDynamicData = DashMap<Address, PositionDynamicData>;
#[derive(Clone)]
pub struct StateMap {
    pub market: DmMarket,
    pub account: DmAccount,
    pub position: DmAccountPosition,
    pub price: DmPrice,
    pub ws_state: WsServerState,
    pub account_dynamic_data: DmAccountDynamicData,
    pub position_dynamic_data: DmPositionDynamicData,
}
impl StateMap {
    pub fn new(store_path: PathBuf, supported_symbol: SupportedSymbol) -> anyhow::Result<Self> {
        let storage = local::Local::new(store_path)?;
        let market: DmMarket = DashMap::new();
        let account: DmAccount = DashMap::new();
        let position: DmAccountPosition = DashMap::new();
        let price: DmPrice = DashMap::new();
        Ok(Self {
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
    pub async fn new(mp: SharedStateMap, event_ws_tx: WsWatchTx, is_write_ws_event: bool) -> Self {
        let (watch_tx, watch_rx) = mpsc::unbounded_channel::<Message>();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        Self {
            watch_tx,
            task: Task::new(
                "watch",
                shutdown_tx,
                tokio::spawn(watch_message(
                    mp,
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

async fn watch_message(
    mp: SharedStateMap,
    mut watch_rx: MessageReceiver,
    mut shutdown_rx: oneshot::Receiver<()>,
    event_ws_tx: WsWatchTx,
    is_write_spread: bool,
) -> anyhow::Result<()> {
    info!("start scale data watch ...");
    loop {
        tokio::select! {
            _ = (&mut shutdown_rx) => {
                info!("got shutdown signal,break watch data!");
                break;
            },
            r = watch_rx.recv()=> {
                match r {
                    Some(msg)=>{
                        // debug!("data channel got data : {:?}",msg);
                        keep_message(mp.clone(), msg,event_ws_tx.clone(),is_write_spread).await;
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

async fn keep_message(
    mp: SharedStateMap,
    msg: Message,
    event_ws_tx: WsWatchTx,
    is_write_ws_event: bool,
) {
    let tag = msg.state.to_string();
    let keys = local::Keys::new(local::Prefix::Active);
    match msg.state {
        State::List(list) => {}
        State::Market(market) => {}
        State::Account(account) => {}
        State::Position(mut position) => {}
        State::Price(org_price) => {}
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
        mp: SharedStateMap,
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
