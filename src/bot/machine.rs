use crate::bot::cron::Cron;
use crate::bot::state::{
    Account, Address, Direction, Market, Position, PositionStatus, PositionType, Price, State,
    Status, BURST_RATE,
};
use crate::bot::storage::{self, Storage};
use crate::com::CliError;
use chrono::{Datelike, NaiveDate, Utc};
use dashmap::{DashMap, DashSet};
use log::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tokio::{
    sync::{
        mpsc::{self, UnboundedReceiver, UnboundedSender},
        oneshot,
    },
    task::JoinHandle,
    time::{self as tokio_time, Duration as TokioDuration},
};

struct Task(oneshot::Sender<()>, JoinHandle<anyhow::Result<()>>);

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
// key is user address Address
type DmAccountDynamicData = DashMap<Address, AccountDynamicData>;
type DmPositionDynamicData = DashMap<Address, PositionDynamicData>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountDynamicData {
    pub profit: f64,
    pub margin_percentage: f64,
    pub equity: f64,
    pub profit_rate: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionDynamicData {
    pub profit_rate: f64,
}

impl Default for AccountDynamicData {
    fn default() -> Self {
        AccountDynamicData {
            profit: 0.0,
            margin_percentage: 0.0,
            equity: 0.0,
            profit_rate: 0.0,
        }
    }
}
impl Default for PositionDynamicData {
    fn default() -> Self {
        PositionDynamicData { profit_rate: 0.0 }
    }
}

#[derive(Clone)]
pub struct StateMap {
    pub market: DmMarket,
    pub account: DmAccount,
    pub position: DmAccountPosition,
    pub price_idx: DmPrice,
    pub price_market_idx: DmIdxPriceMarket,
    pub account_dynamic_idx: DmAccountDynamicData,
    pub position_dynamic_idx: DmPositionDynamicData,
    pub storage: Storage,
}
impl StateMap {
    pub fn new(store_path: PathBuf) -> anyhow::Result<Self> {
        let storage = storage::Storage::new(store_path)?;
        let market: DmMarket = DashMap::new();
        let account: DmAccount = DashMap::new();
        let position: DmAccountPosition = DashMap::new();
        let price_idx: DmPrice = DashMap::new();
        let price_market_idx: DmIdxPriceMarket = DashMap::new();
        let account_dynamic_idx: DmAccountDynamicData = DashMap::new();
        let position_dynamic_idx: DmPositionDynamicData = DashMap::new();
        Ok(Self {
            market,
            account,
            position,
            storage,
            price_idx,
            price_market_idx,
            account_dynamic_idx,
            position_dynamic_idx,
        })
    }
}
pub type SharedStateMap = Arc<StateMap>;

pub struct Watch {
    shutdown_tx: oneshot::Sender<()>,
    pub watch_tx: UnboundedSender<Message>,
    task: JoinHandle<anyhow::Result<()>>,
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
            shutdown_tx,
            watch_tx,
            task: tokio::spawn(watch_message(mp, watch_rx, shutdown_rx)),
        }
    }
    pub async fn shutdown(self) -> anyhow::Result<()> {
        debug!("shutdown watch ...");
        let _ = self.shutdown_tx.send(());
        self.task.await??;
        Ok(())
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
                        keep_message(mp.clone(), msg);
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

fn keep_message(mp: SharedStateMap, msg: Message) {
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
                save_as_history(mp, &mut keys, &State::Account(account))
            } else {
                mp.account.insert(msg.address.clone(), account.clone());
                save_to_active(mp, &mut keys, &State::Account(account))
            }
        }
        State::Position(position) => {
            let mut keys = keys
                .add(tag)
                .add(position.account_id.to_string())
                .add(msg.address.to_string());
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
                save_as_history(mp, &mut keys, &State::Position(position))
            } else {
                match mp.position.get(&msg.address) {
                    Some(p) => {
                        p.insert(msg.address.clone(), position.clone());
                    }
                    None => {
                        let p: DmPosition = dashmap::DashMap::new();
                        p.insert(msg.address.clone(), position.clone());
                        mp.position.insert((&position.account_id).copy(), p);
                    }
                };
                save_to_active(mp, &mut keys, &State::Position(position))
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
                "save a account as active error:{}, key:{}",
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
    pub async fn new(mp: SharedStateMap, tasks: usize) -> anyhow::Result<Self> {
        let mut tasks = tasks;
        if tasks < 2 {
            tasks = 2;
        }
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let (task_tx, task_rx) = flume::bounded::<Address>(tasks);
        let (fund_task_tx, fund_task_rx) = flume::bounded::<Address>(tasks);
        let cron = Cron::new().await?;
        // create fund fee cron
        let fund_fee_timer = cron.add_job("1/2 * * * * *").await?;
        // create opening price cron
        let opening_price_timer = cron.add_job("1/2 * * * * *").await?;
        let account_task = Task(
            shutdown_tx,
            tokio::spawn(loop_account_task(
                mp.clone(),
                task_tx,
                fund_task_tx,
                shutdown_rx,
                fund_fee_timer,
                opening_price_timer,
            )),
        );
        Ok(Self {
            account_tasks: account_task,
            position_tasks: loop_position_task(mp, tasks, task_rx, fund_task_rx).await?,
            cron,
        })
    }

    pub async fn shutdown(self) -> anyhow::Result<()> {
        debug!("start shutdown liquidation...");
        for task in self.position_tasks {
            let _ = task.0.send(());
            task.1.await??;
        }
        let _ = self.account_tasks.0.send(());
        self.account_tasks.1.await??;
        self.cron.shutdown().await?;
        Ok(())
    }
}

async fn loop_account_task(
    mp: SharedStateMap,
    task_tx: flume::Sender<Address>,
    fund_task_tx: flume::Sender<Address>,
    mut shutdown_rx: oneshot::Receiver<()>,
    mut fund_fee_timer: mpsc::Receiver<()>,
    mut opening_price_timer: mpsc::Receiver<()>,
) -> anyhow::Result<()> {
    let mut count: usize = 0;

    loop {
        tokio::select! {
            _ = (&mut shutdown_rx) => {
                info!("Got shutdown signal,break loop account!");
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
                update_opening_price(&mp.market).await;
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
async fn update_opening_price(dm_market: &DmMarket) {
    for v in dm_market {
        // todo update opening price
        // todo!();
    }
}
async fn loop_position_task(
    mp: SharedStateMap,
    tasks: usize,
    task_rx: flume::Receiver<Address>,
    fund_task_rx: flume::Receiver<Address>,
) -> anyhow::Result<Vec<Task>> {
    debug!("start position task...");
    let mut workers: Vec<Task> = Vec::with_capacity(tasks);
    for _ in 0..tasks {
        // let cfg = config.clone();
        let (task_shutdown_tx, task_shutdown_rx) = oneshot::channel::<()>();
        let task = tokio::spawn(loop_position_by_user(
            mp.clone(),
            task_rx.clone(),
            fund_task_rx.clone(),
            task_shutdown_rx,
        ));
        workers.push(Task(task_shutdown_tx, task));
    }
    Ok(workers)
}

async fn loop_position_by_user(
    mp: SharedStateMap,
    task_rx: flume::Receiver<Address>,
    fund_task_rx: flume::Receiver<Address>,
    mut shutdown_rx: oneshot::Receiver<()>,
) -> anyhow::Result<()> {
    loop {
        tokio::select! {
            _ = (&mut shutdown_rx) => {
                info!("got shutdown signal,break loop position!");
                break;
            },
            account_address = task_rx.recv_async() => {
                match account_address {
                    Ok(address) => {
                        debug!("got account address from task recv: {:?}",address);
                        let account = mp.account.get(&address);
                        match account {
                            Some(account) => {
                                handle_fund_fee(mp.clone(),&account,&address);
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
                        let account = mp.account.get(&address);
                        match account {
                            Some(account) => {
                                compute_position(mp.clone(),&account,&address);
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

fn handle_fund_fee(mp: SharedStateMap, account: &Account, address: &Address) {
    // todo handle fund fee
    // todo!();
}

fn compute_position(mp: SharedStateMap, account: &Account, address: &Address) {
    match mp.position.get(&address) {
        Some(positions) => {
            debug!("got position: {:?}", positions);
            compute_pl_all_position(
                &mp.market,
                account,
                &positions,
                &mp.price_idx,
                &mp.account_dynamic_idx,
                &mp.position_dynamic_idx,
            );
        }
        None => {
            debug!("no position for state map: {:?}", address);
        }
    }
}

fn compute_pl_all_position(
    dm_market: &DmMarket,
    account: &Account,
    dm_position: &DmPosition,
    dm_price: &DmPrice,
    account_dynamic_idx_mp: &DmAccountDynamicData,
    position_dynamic_idx_mp: &DmPositionDynamicData,
) {
    let mut account_data = AccountDynamicData::default();
    let mut position_sort: Vec<PositionSort> = Vec::with_capacity(account.full_position_idx.len());
    let mut pl_full = 0i64;
    for v in dm_position.iter() {
        let position = v.value();
        match dm_market.get(&position.market_id) {
            Some(market) => match dm_price.get(&position.market_id) {
                Some(price) => {
                    let pl = position.get_pl(&price);
                    let fund_fee = position.get_position_fund_fee(&market);
                    let pl_and_fund_fee = pl + fund_fee;
                    account_data.profit += pl as f64;
                    account_data.equity += pl_and_fund_fee as f64;
                    if position.position_type == PositionType::Full {
                        pl_full += pl_and_fund_fee;
                    } else {
                        if (pl_and_fund_fee as f64 / position.margin as f64) < BURST_RATE {
                            // todo BURST position
                            todo!();
                        }
                    }
                    position_dynamic_idx_mp.insert(
                        position.id.copy(),
                        PositionDynamicData {
                            profit_rate: pl as f64 / position.margin as f64,
                        },
                    );
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
            // todo close position
            {
                // todo!();
            }
            match p.direction {
                Direction::Buy => {
                    margin_full_buy_total -= p.margin;
                }
                Direction::Sell => {
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
    account_data.equity += account.balance as f64;
    account_data.margin_percentage = account_data.equity as f64 / account.margin_total as f64;
    account_data.profit_rate = account_data.profit as f64 / account.margin_total as f64;
    account_dynamic_idx_mp.insert(account.id.copy(), account_data);
}
