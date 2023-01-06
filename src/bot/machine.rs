use crate::bot::cron::Cron;
use crate::bot::state::{
    Account, Address, Direction, Market, Position, PositionStatus, PositionType, Price, State,
    Status,
};
use crate::bot::storage::{self, Keys, Storage};
use crate::com::CliError;
use chrono::{Datelike, NaiveDate, Utc};
use dashmap::DashMap;
use log::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::{
    sync::{
        mpsc::{self, UnboundedReceiver, UnboundedSender},
        oneshot, watch,
    },
    task::JoinHandle,
    time::{self as tokio_time, Duration as TokioDuration},
};
use tokio_cron_scheduler::{Job, JobScheduler, JobToRun};

struct Task(oneshot::Sender<()>, JoinHandle<anyhow::Result<()>>);

// key is market Address,value is market data
type DmMarket = DashMap<Address, Market>;
// key is user account Address,value is user account data.
type DmAccount = DashMap<Address, Account>;
// key is position account Address,value is position account data
type DmPosition = DashMap<Address, Position>;
// key is price account key ,value is price
type DmPrice = DashMap<Address, Price>;
// key is user account Address,value is position k-v map
type DmAccountPosition = DashMap<Address, DmPosition>;
// key is price account,value is market account
type DmIdxPriceMarket = DashMap<Address, Address>;
// key is user account Address
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
    pub price_account: DmPrice,
    pub price_account_idx: DmIdxPriceMarket,
    pub user_dynamic_idx: DmAccountDynamicData,
    pub position_dynamic_idx: DmPositionDynamicData,
    pub storage: Storage,
}
impl StateMap {
    pub fn new(store_path: PathBuf) -> anyhow::Result<Self> {
        let storage = storage::Storage::new(store_path)?;
        let market: DmMarket = DashMap::new();
        let account: DmAccount = DashMap::new();
        let position: DmAccountPosition = DashMap::new();
        let price_account: DmPrice = DashMap::new();
        let price_account_idx: DmIdxPriceMarket = DashMap::new();
        let user_dynamic_idx: DmAccountDynamicData = DashMap::new();
        let position_dynamic_idx: DmPositionDynamicData = DashMap::new();
        Ok(Self {
            market,
            account,
            position,
            storage,
            price_account,
            price_account_idx,
            user_dynamic_idx,
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
#[derive(Debug)]
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
                save_as_history(mp, &mut keys, &msg.address)
            } else {
                mp.market.insert(msg.address.clone(), market);
                save_to_active(mp, &mut keys, &msg.address)
            }
        }
        State::Account(account) => {
            let mut keys = keys.add(tag).add(msg.address.to_string());

            if msg.status == Status::Deleted {
                mp.account.remove(&msg.address);
                save_as_history(mp, &mut keys, &msg.address)
            } else {
                mp.account.insert(msg.address.clone(), account);
                save_to_active(mp, &mut keys, &msg.address)
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
                save_as_history(mp, &mut keys, &msg.address)
            } else {
                match mp.position.get(&msg.address) {
                    Some(p) => {
                        p.insert(msg.address.clone(), position.clone());
                    }
                    None => {
                        let p: DmPosition = dashmap::DashMap::new();
                        p.insert(msg.address.clone(), position.clone());
                        mp.position.insert(position.account_id, p);
                    }
                };
                save_to_active(mp, &mut keys, &msg.address)
            }
        }
        State::Price(price) => {
            // let market = mp.
            // mp.price_account.insert(msg.address, price);
        }
        State::None => {
            debug!("got none data : {:?}", msg);
        }
    }
}

fn save_as_history(mp: SharedStateMap, ks: &mut storage::Keys, address: &Address) {
    match mp.storage.save_as_history(ks, address) {
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

fn save_to_active(mp: SharedStateMap, ks: &mut storage::Keys, address: &Address) {
    match mp.storage.save_to_active(ks, address) {
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
                shutdown_rx,
                fund_fee_timer,
                opening_price_timer,
            )),
        );
        Ok(Self {
            account_tasks: account_task,
            position_tasks: loop_position_task(mp, tasks, task_rx).await?,
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
            }
            _= opening_price_timer.recv() => {
                info!("Got opening price timer signal , current time: {:?}",Utc::now());
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

async fn loop_position_task(
    mp: SharedStateMap,
    tasks: usize,
    task_rx: flume::Receiver<Address>,
) -> anyhow::Result<Vec<Task>> {
    debug!("start position task...");
    let mut workers: Vec<Task> = Vec::with_capacity(tasks);
    for _ in 0..tasks {
        // let cfg = config.clone();
        let (task_shutdown_tx, task_shutdown_rx) = oneshot::channel::<()>();
        let task = tokio::spawn(loop_position_by_user(
            mp.clone(),
            task_rx.clone(),
            task_shutdown_rx,
        ));
        workers.push(Task(task_shutdown_tx, task));
    }
    Ok(workers)
}

async fn loop_position_by_user(
    mp: SharedStateMap,
    task_rx: flume::Receiver<Address>,
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
                                debug!("got account from state map: {:?}",account);
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
    pub offset: u32,
    pub profit: i64,
    pub direction: Direction,
    pub margin: u64,
    pub market_account: Option<Address>,
}
fn compute_position(mp: SharedStateMap, account: &Account, address: &Address) {
    match mp.position.get(&address) {
        Some(positions) => {
            debug!("got position: {:?}", positions);
            compute_pl_all_position(account, &positions);
        }
        None => {
            debug!("no position for state map: {:?}", address);
        }
    }
}
fn compute_pl_all_position(account: &Account, positions: &DmPosition) {
    let mut total_pl: f64 = 0.0;
    // let headers = account.full_position_idx;
    let mut data = AccountDynamicData::default();
    let mut position_sort: Vec<PositionSort> = Vec::with_capacity(account.full_position_idx.len());
    for v in positions.iter() {
        let position = v.value();
    }
}
