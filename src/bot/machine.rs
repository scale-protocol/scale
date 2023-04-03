use crate::bot::cron::Cron;
use crate::bot::state::{
    Account, Address, Direction, Market, MoveCall, Position, PositionStatus, PositionType, Price,
    State, Status, BURST_RATE,
};
use crate::bot::storage::{self, Storage};
use crate::bot::ws::{
    AccountDynamicData, PositionDynamicData, SupportedSymbol, WsServerState, WsSrvMessage,
};
use crate::com::{self, Task};
use chrono::Utc;
use dashmap::{DashMap, DashSet};
use log::*;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tokio::{
    sync::{
        mpsc::{self, UnboundedReceiver, UnboundedSender},
        oneshot,
    },
    time::{self as tokio_time, Duration as TokioDuration},
};

// key is market Address,value is market data
type DmMarket = DashMap<Address, Market>;
// key is account Address,value is account data.
type DmAccount = DashMap<Address, Account>;
// key is position Address,value is position  data
type DmPosition = DashMap<Address, Position>;
// key is market address ,value is price
type DmPrice = DashMap<Address, Price>;
// key is account address Address,value is position k-v map
type DmAccountPosition = DashMap<Address, DmPosition>;
// key is symbol,value is market address set
type DmIdxPriceMarket = DashMap<String, DashSet<Address>>;
type DmAccountDynamicData = DashMap<Address, AccountDynamicData>;
type DmPositionDynamicData = DashMap<Address, PositionDynamicData>;
#[derive(Clone)]
pub struct StateMap {
    pub market: DmMarket,
    pub account: DmAccount,
    pub position: DmAccountPosition,
    pub price_idx: DmPrice,
    pub price_market_idx: DmIdxPriceMarket,
    pub storage: Storage,
    pub ws_state: WsServerState,
    pub account_dynamic_data: DmAccountDynamicData,
    pub position_dynamic_data: DmPositionDynamicData,
}
impl StateMap {
    pub fn new(store_path: PathBuf, supported_symbol: SupportedSymbol) -> anyhow::Result<Self> {
        let storage = storage::Storage::new(store_path)?;
        let market: DmMarket = DashMap::new();
        let account: DmAccount = DashMap::new();
        let position: DmAccountPosition = DashMap::new();
        let price_idx: DmPrice = DashMap::new();
        let price_market_idx: DmIdxPriceMarket = DashMap::new();
        Ok(Self {
            market,
            account,
            position,
            storage,
            price_idx,
            price_market_idx,
            ws_state: WsServerState::new(supported_symbol),
            account_dynamic_data: DashMap::new(),
            position_dynamic_data: DashMap::new(),
        })
    }

    pub fn load_active_account_from_local(&mut self) -> anyhow::Result<()> {
        info!("start load active object from local!");
        let p = storage::Prefix::Active;
        let r = self.storage.scan_prefix(&p);
        for i in r {
            match i {
                Ok((k, v)) => {
                    let key = String::from_utf8(k.to_vec())
                        .map_err(|e| com::CliError::JsonError(e.to_string()))?;
                    let keys = storage::Keys::from_str(key.as_str())?;
                    debug!("load objects from db: {}", keys.get_storage_key());
                    let pk = keys.get_end();
                    debug!("load address from db : {}", pk);
                    let pbk = Address::from_str(pk.as_str())
                        .map_err(|e| com::CliError::CliError(e.to_string()))?;
                    let values: State = serde_json::from_slice(v.to_vec().as_slice())
                        .map_err(|e| com::CliError::JsonError(e.to_string()))?;
                    match values {
                        State::Market(market) => {
                            self.market.insert(pbk.clone(), market.clone());
                            match self.price_market_idx.get(&market.symbol) {
                                Some(p) => {
                                    p.value().insert(pbk.clone());
                                }
                                None => {
                                    let set = DashSet::new();
                                    set.insert(pbk.clone());
                                    self.price_market_idx.insert(market.symbol.clone(), set);
                                }
                            }
                        }
                        State::Account(account) => {
                            self.account.insert(pbk, account);
                        }
                        State::Position(position) => {
                            match self.position.get(&pbk) {
                                Some(p) => {
                                    p.insert(pbk.clone(), position.clone());
                                }
                                None => {
                                    let p: DmPosition = dashmap::DashMap::new();
                                    p.insert(pbk, position.clone());
                                    self.position.insert((&position.account_id).copy(), p);
                                }
                            };
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    debug!("{}", e);
                }
            }
        }
        info!("complete load active account from local!");
        Ok(())
    }
}
pub type SharedStateMap = Arc<StateMap>;

pub struct Watch {
    pub watch_tx: UnboundedSender<Message>,
    task: Task,
}
#[derive(Debug, Clone)]
pub struct Message {
    pub address: Address,
    pub state: State,
    pub status: Status,
}
impl Watch {
    pub async fn new(mp: SharedStateMap) -> Self {
        let (watch_tx, watch_rx) = mpsc::unbounded_channel::<Message>();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        Self {
            watch_tx,
            task: Task::new(
                "watch",
                shutdown_tx,
                tokio::spawn(watch_message(mp, watch_rx, shutdown_rx)),
            ),
        }
    }
    pub async fn shutdown(self) {
        self.task.shutdown().await;
    }
}

async fn watch_message(
    mp: SharedStateMap,
    mut watch_rx: UnboundedReceiver<Message>,
    mut shutdown_rx: oneshot::Receiver<()>,
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
                        keep_message(mp.clone(), msg).await;
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

async fn keep_message(mp: SharedStateMap, msg: Message) {
    let tag = msg.state.to_string();
    let keys = storage::Keys::new(storage::Prefix::Active);
    match msg.state {
        State::Market(market) => {
            let mut keys = keys.add(tag).add(msg.address.to_string());
            if msg.status == Status::Deleted {
                mp.market.remove(&msg.address);
                if let Some(p) = mp.price_market_idx.get(&market.symbol) {
                    p.value().remove(&msg.address);
                }
                save_as_history(mp, &mut keys, &State::Market(market))
            } else {
                mp.market.insert(msg.address.clone(), market.clone());
                match mp.price_market_idx.get(&market.symbol) {
                    Some(p) => {
                        p.value().insert(msg.address.clone());
                    }
                    None => {
                        let set = DashSet::new();
                        set.insert(msg.address.clone());
                        mp.price_market_idx.insert(market.symbol.clone(), set);
                    }
                }
                save_to_active(mp, &mut keys, &State::Market(market))
            }
        }
        State::Account(account) => {
            let mut keys = keys.add(tag).add(msg.address.to_string());

            if msg.status == Status::Deleted {
                mp.account.remove(&msg.address);
                save_as_history(mp.clone(), &mut keys, &State::Account(account.clone()))
            } else {
                mp.account.insert(msg.address.clone(), account.clone());
                save_to_active(mp.clone(), &mut keys, &State::Account(account.clone()))
            }
            if let Some(tx) = mp.ws_state.conns.get(&msg.address) {
                let mut account_data = AccountDynamicData::default();
                if let Some(data) = mp.account_dynamic_data.get(&msg.address) {
                    account_data = data.clone();
                } else {
                    account_data.equity = account.balance as i64;
                }
                account_data.balance = account.balance as i64;
                let r = tx
                    .value()
                    .send(WsSrvMessage::AccountUpdate(account_data.clone()))
                    .await;
                if let Err(e) = r {
                    error!("send account dynamic data to ws channel data error: {}", e);
                }
                debug!(
                    "send account dynamic data to ws channel data: {:?}",
                    account_data
                );
            }
        }
        State::Position(position) => {
            let mut keys = keys
                .add(tag)
                .add(position.account_id.to_string())
                .add(msg.address.to_string());
            let mut position_create = false;
            let mut position_close = false;
            let account_id = position.account_id.copy();
            let position_id = position.id.copy();
            if msg.status == Status::Deleted
                || position.status == PositionStatus::NormalClosing
                || position.status == PositionStatus::ForcedClosing
            {
                match mp.position.get(&position.account_id) {
                    Some(p) => {
                        p.remove(&msg.address);
                    }
                    None => {
                        // nothing to do
                    }
                };
                // close position
                position_close = true;
                save_as_history(mp.clone(), &mut keys, &State::Position(position))
            } else {
                match mp.position.get(&position.account_id) {
                    Some(p) => {
                        if let None = p.insert(msg.address.clone(), position.clone()) {
                            // create position
                            position_create = true;
                        }
                    }
                    None => {
                        let p: DmPosition = dashmap::DashMap::new();
                        p.insert(msg.address.clone(), position.clone());
                        if let None = mp.position.insert((&position.account_id).copy(), p) {
                            // create position
                            position_create = true;
                        }
                    }
                };
                save_to_active(mp.clone(), &mut keys, &State::Position(position))
            }
            // let position_data = mp.position_dynamic_data.get(&msg.address.clone());
            let mut position_data = PositionDynamicData::default();
            position_data.id = position_id;
            debug!(
                "position id: {:?}===>CREAte?{:?},close?{:?}",
                position_data.id.to_string(),
                position_create,
                position_close
            );
            if let Some(tx) = mp.ws_state.conns.get(&account_id) {
                let mut message: Option<WsSrvMessage> = None;
                if position_create {
                    message = Some(WsSrvMessage::PositionOpen(position_data));
                } else if position_close {
                    message = Some(WsSrvMessage::PositionClose(position_data));
                }
                if let Some(m) = message {
                    if let Err(e) = tx.value().send(m).await {
                        error!("send position dynamic data to ws channel data error: {}", e);
                    }
                }
            }
        }

        State::Price(org_price) => {
            let idx_set = &mp.price_market_idx;
            let market_mp = &mp.market;
            let price_mp = &mp.price_idx;
            match idx_set.get(&org_price.symbol) {
                Some(p) => {
                    for market in p.value().iter() {
                        if let Some(m) = market_mp.get(&market) {
                            if org_price.price <= 0 {
                                error!("got a danger price : {:?}", &org_price);
                                continue;
                            }
                            let price = m.get_price(org_price.price as u64);
                            price_mp.insert(m.id.clone(), price);
                        }
                    }
                }
                None => {
                    debug!("price market index not existence : {:?}", &org_price.symbol);
                }
            }
        }
        State::None => {
            debug!("got none data : {:?}", msg);
        }
    }
}

fn save_as_history(mp: SharedStateMap, ks: &mut storage::Keys, data: &State) {
    match mp.storage.save_as_history(ks, data) {
        Ok(()) => {
            debug!(
                "save a address as history success! key:{}",
                ks.get_storage_key()
            );
        }
        Err(e) => {
            error!(
                "save a address as history error:{}, key:{}",
                e,
                ks.get_storage_key()
            );
        }
    }
}

fn save_to_active(mp: SharedStateMap, ks: &mut storage::Keys, data: &State) {
    match mp.storage.save_to_active(ks, data) {
        Ok(()) => {
            debug!(
                "save a address as active success! key:{}",
                ks.get_storage_key()
            );
        }
        Err(e) => {
            error!(
                "save a account as active error: {}, key:{}",
                e,
                ks.get_storage_key()
            );
        }
    }
}

pub struct Liquidation {
    account_tasks: Task,
    position_tasks: Vec<Task>,
    cron: Cron,
}

impl Liquidation {
    pub async fn new<C>(mp: SharedStateMap, tasks: usize, call: Arc<C>) -> anyhow::Result<Self>
    where
        C: MoveCall + Send + Sync + 'static,
    {
        let mut tasks = tasks;
        if tasks < 2 {
            tasks = 2;
        }
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let (task_tx, task_rx) = flume::unbounded::<Address>();
        let (fund_task_tx, fund_task_rx) = flume::unbounded::<Address>();
        let cron = Cron::new().await?;
        // create fund fee cron
        let fund_fee_timer = cron.add_job("0 0 0 * * *").await?;
        // create opening price cron
        let opening_price_timer = cron.add_job("0 0 0,8,16 * * *").await?;
        let account_task = Task::new(
            "liquidation_account_task",
            shutdown_tx,
            tokio::spawn(loop_account_task(
                mp.clone(),
                task_tx,
                fund_task_tx,
                shutdown_rx,
                fund_fee_timer,
                opening_price_timer,
                call.clone(),
            )),
        );
        Ok(Self {
            account_tasks: account_task,
            position_tasks: loop_position_task(mp, tasks, task_rx, fund_task_rx, call).await?,
            cron,
        })
    }

    pub async fn shutdown(self) {
        debug!("start shutdown liquidation...");
        for task in self.position_tasks {
            task.shutdown().await;
        }
        self.account_tasks.shutdown().await;
        if let Err(e) = self.cron.shutdown().await {
            error!("shutdown cron error:{}", e);
        }
    }
}

async fn loop_account_task<C>(
    mp: SharedStateMap,
    task_tx: flume::Sender<Address>,
    fund_task_tx: flume::Sender<Address>,
    mut shutdown_rx: oneshot::Receiver<()>,
    mut fund_fee_timer: mpsc::Receiver<()>,
    mut opening_price_timer: mpsc::Receiver<()>,
    call: Arc<C>,
) -> anyhow::Result<()>
where
    C: MoveCall,
{
    let mut count: usize = 0;

    loop {
        tokio::select! {
            _ = (&mut shutdown_rx) => {
                info!("got shutdown signal,break loop account!");
                break;
            },
            _ = fund_fee_timer.recv() => {
                info!("Got fund fee timer signal , current time: {:?}",Utc::now());
                for v in & mp.account {
                    let address = v.key().clone();
                    if let Err(e) = fund_task_tx.send(address) {
                        error!("send address to fund task channel error: {}", e);
                    }
                }
            }
            _= opening_price_timer.recv() => {
                info!("Got opening price timer signal , current time: {:?}",Utc::now());
                update_opening_price(&mp.market,call.clone()).await;
            }
            // loop account
            _ = async {
               loop{
                    let now = tokio_time::Instant::now();
                    info!("Start a new round of liquidation... count: {}",count);
                    for v in & mp.account {
                        debug!("account id: {}",v.key());
                        let address = v.key().clone();
                        if let Err(e) = task_tx.send(address) {
                            error!("send address to task channel error: {}", e);
                        }
                    }
                    tokio_time::sleep(TokioDuration::from_secs(10)).await;
                    let t = now.elapsed();
                    count+=1;
                    info!("Complete a new round of liquidation... use time: {:?} , count: {}", t, count);
               }
            } => {
                info!("loop account task break!");
            }
        }
    }
    Ok(())
}

async fn update_opening_price<C>(dm_market: &DmMarket, call: Arc<C>)
where
    C: MoveCall,
{
    for v in dm_market {
        // todo update opening price
        if let Err(e) = call.trigger_update_opening_price(v.key().clone()).await {
            error!("update opening price error: {}", e);
        }
    }
}
async fn loop_position_task<C>(
    mp: SharedStateMap,
    tasks: usize,
    task_rx: flume::Receiver<Address>,
    fund_task_rx: flume::Receiver<Address>,
    call: Arc<C>,
) -> anyhow::Result<Vec<Task>>
where
    C: MoveCall + Send + Sync + 'static,
{
    debug!("start position task...");
    let mut workers: Vec<Task> = Vec::with_capacity(tasks);
    for t in 0..tasks {
        // let cfg = config.clone();
        let (task_shutdown_tx, task_shutdown_rx) = oneshot::channel::<()>();
        let task = tokio::spawn(loop_position_by_user(
            mp.clone(),
            task_rx.clone(),
            fund_task_rx.clone(),
            task_shutdown_rx,
            call.clone(),
        ));
        workers.push(Task::new(
            &format!("liquidation_position_task_{}", t),
            task_shutdown_tx,
            task,
        ));
    }
    Ok(workers)
}

async fn loop_position_by_user<C>(
    mp: SharedStateMap,
    task_rx: flume::Receiver<Address>,
    fund_task_rx: flume::Receiver<Address>,
    mut shutdown_rx: oneshot::Receiver<()>,
    call: Arc<C>,
) -> anyhow::Result<()>
where
    C: MoveCall + Send + Sync,
{
    loop {
        tokio::select! {
            _ = (&mut shutdown_rx) => {
                info!("got shutdown signal,break loop position!");
                break;
            },
            account_address = task_rx.recv_async() => {
                match account_address {
                    Ok(address) => {
                        debug!("got account address from task recv: {:?}",address.to_string());
                        let account = mp.account.get(&address);
                        match account {
                            Some(account) => {
                                compute_position(mp.clone(),&account,&address,call.clone()).await;
                            },
                            None => {
                                debug!("no account for state map : {:?}",address);
                            }
                        }
                    },
                    Err(e) => {
                        error!("recv account address error: {}",e);
                    }
                }
            },
            account_address = fund_task_rx.recv_async() => {
                match account_address {
                    Ok(address) => {
                        debug!("got account address from fund task recv: {:?}",address);
                        process_fund_fee(&address,call.clone()).await;
                    },
                    Err(e) => {
                        error!("recv account address error: {}",e);
                    }
                }
            },
        }
    }
    Ok(())
}

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
struct PositionSort {
    pub position_address: Address,
    pub profit: i64,
    pub direction: Direction,
    pub margin: u64,
    pub market_address: Option<Address>,
}

async fn process_fund_fee<C>(account_address: &Address, call: Arc<C>)
where
    C: MoveCall,
{
    // handle fund fee
    if let Err(e) = call.process_fund_fee(account_address.clone()).await {
        error!("process fund fee error: {}", e);
    }
}

async fn compute_position<C>(mp: SharedStateMap, account: &Account, address: &Address, call: Arc<C>)
where
    C: MoveCall,
{
    match mp.position.get(&address) {
        Some(positions) => {
            debug!("got position: {:?}", positions);
            compute_pl_all_position(mp.clone(), account, &positions, call).await;
        }
        None => {
            debug!("no position for state map : {:?}", address);
        }
    }
}

async fn compute_pl_all_position<C>(
    mp: SharedStateMap,
    account: &Account,
    dm_position: &DmPosition,
    // dm_price: &DmPrice,
    // ws_state: &WsServerState,
    call: Arc<C>,
) where
    C: MoveCall,
{
    let mut account_data = AccountDynamicData::default();
    account_data.balance = account.balance as i64;
    let mut position_sort: Vec<PositionSort> = Vec::with_capacity(account.full_position_idx.len());
    let mut pl_full = 0i64;
    for v in dm_position.iter() {
        let position = v.value();
        if position.status != PositionStatus::Normal {
            continue;
        }
        match mp.market.get(&position.market_id) {
            Some(market) => match mp.price_idx.get(&position.market_id) {
                Some(price) => {
                    let pl = position.get_pl(&price);
                    let fund_fee = position.get_position_fund_fee(&market);
                    let pl_and_fund_fee = pl + fund_fee;
                    account_data.profit += pl;
                    account_data.equity += pl_and_fund_fee;
                    if position.position_type == PositionType::Cross {
                        pl_full += pl_and_fund_fee;
                    } else {
                        if (pl_and_fund_fee as f64 / position.margin as f64) < BURST_RATE {
                            // close position force
                            // if let Err(e) = call
                            //     .burst_position(account.id.clone(), position.id.copy())
                            //     .await
                            // {
                            //     error!("burst position error: {}", e);
                            // }
                        }
                    }
                    let position_dynamic_data = PositionDynamicData {
                        id: position.id.copy(),
                        profit_rate: com::f64_round(pl as f64 / position.margin as f64),
                        profit: pl,
                    };
                    // send position dynamic data to ws
                    if let Some(tx) = mp.ws_state.conns.get(&account.id) {
                        let r = tx
                            .value()
                            .send(WsSrvMessage::PositionUpdate(position_dynamic_data.clone()))
                            .await;
                        if let Err(e) = r {
                            error!("send position dynamic data to ws channel data error: {}", e);
                        }
                        debug!(
                            "send position dynamic data to ws channel data,position id: {:?}",
                            position.id
                        );
                    }
                    mp.position_dynamic_data
                        .insert(position.id.copy(), position_dynamic_data);
                    position_sort.push(PositionSort {
                        profit: pl_and_fund_fee,
                        position_address: position.id.copy(),
                        direction: position.direction,
                        margin: position.margin,
                        market_address: Some(market.id.copy()),
                    });
                }
                None => {
                    error!("no price for position id: {}", position.id);
                    continue;
                }
            },
            None => {
                error!("no market for position id: {}", position.id);
                continue;
            }
        }
    }
    // check full position
    let mut margin_full_buy_total = account.margin_full_buy_total;
    let mut margin_full_sell_total = account.margin_full_sell_total;
    let equity = account.balance as i64 + pl_full;
    let margin_full_total = margin_full_buy_total.max(margin_full_sell_total);
    // Forced close
    if (equity as f64 / margin_full_total as f64) < BURST_RATE {
        // sort
        position_sort.sort_by(|a, b| b.profit.cmp(&a.profit).reverse());
        for p in position_sort {
            // close position
            // if let Err(e) = call
            //     .burst_position(account.id.clone(), p.position_address)
            //     .await
            // {
            //     error!("burst position error: {}", e);
            // }
            match p.direction {
                Direction::Buy => {
                    if margin_full_buy_total < p.margin {
                        warn!("margin_full_buy_total < p.margin");
                        continue;
                    }
                    margin_full_buy_total -= p.margin;
                }
                Direction::Sell => {
                    if margin_full_sell_total < p.margin {
                        warn!("margin_full_sell_total < p.margin");
                        continue;
                    }
                    margin_full_sell_total -= p.margin;
                }
                Direction::Flat => {}
            }
            // Reach the safety line of explosion
            if ((equity + p.profit) as f64
                / margin_full_buy_total.max(margin_full_sell_total) as f64)
                > BURST_RATE
            {
                break;
            }
        }
    }
    account_data.equity += account.balance as i64;
    if account.margin_total != 0 {
        account_data.margin_percentage =
            com::f64_round(account_data.equity as f64 / account.margin_total as f64);
        account_data.profit_rate =
            com::f64_round(account_data.profit as f64 / account.margin_total as f64);
    }
    mp.account_dynamic_data
        .insert(account.id.copy(), account_data.clone());
    // send to ws
    if let Some(tx) = mp.ws_state.conns.get(&account.id) {
        let r = tx
            .value()
            .send(WsSrvMessage::AccountUpdate(account_data.clone()))
            .await;
        if let Err(e) = r {
            error!("send account dynamic data to ws channel data error: {}", e);
        }
        debug!(
            "send account dynamic data to ws channel data: {:?}",
            account_data
        );
    }
}
