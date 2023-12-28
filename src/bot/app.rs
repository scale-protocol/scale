use crate::app::App;
use crate::bot::oracle::{DmPriceFeed, PriceFeed, PriceOracle};
use crate::bot::{
    influxdb, machine, price,
    state::MoveCall,
    storage::{local, postgres},
    ws::{self, new_shared_dm_symbol_id},
};
use crate::com::ClientError;
use crate::config::{self, Config};
use crate::http::router::HttpServer;
use crate::sui::config::{Config as SuiConfig, Context as SuiContext};
use crate::sui::subscribe;
use crate::sui::tool::Tool;
use log::*;
use std::net::ToSocketAddrs;
use std::path::PathBuf;
use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tokio::{runtime::Builder, runtime::Runtime, signal, sync::mpsc};

use super::storage;

#[derive(Debug, Clone)]
pub struct Options {
    pub tasks: usize,
    pub socket_addr: Option<SocketAddr>,
    pub duration: i64,
    pub full_node: bool,
    pub gas_budget: u64,
    pub config_file: Option<PathBuf>,
}

pub fn run(
    app: App,
    config_file: Option<PathBuf>,
    args: &clap::ArgMatches,
    gas_budget: u64,
) -> anyhow::Result<()> {
    let tasks = match args.get_one::<usize>("tasks") {
        Some(t) => *t,
        None => 2,
    };
    let port = match args.get_one::<u64>("port") {
        Some(p) => *p,
        None => 3000,
    };
    let ip = match args.get_one::<String>("ip") {
        Some(i) => i.to_string(),
        None => "127.0.0.1".to_string(),
    };
    let mut duration: i64 = match args.get_one::<i64>("duration") {
        Some(d) => *d,
        None => -1,
    };
    if duration < 0 {
        duration = -1;
    }
    let mut opt = Options {
        tasks,
        socket_addr: None,
        duration,
        full_node: *args.get_one::<bool>("full_node").unwrap_or(&false),
        gas_budget,
        config_file,
    };
    let address = format!("{}:{}", ip, port);
    if port > 0 {
        let addr = address
            .to_socket_addrs()
            .map_err(|e| ClientError::HttpServerError(e.to_string()))?
            .next()
            .ok_or(ClientError::HttpServerError("parsing none".to_string()))?;
        opt.socket_addr = Some(addr);
    }
    let mut builder = Builder::new_multi_thread();
    match args.get_one::<usize>("threads") {
        Some(t) => {
            builder.worker_threads(*t);
        }
        None => {}
    }
    let runtime = builder
        .thread_name_fn(|| {
            static ATOMIC_ID: AtomicUsize = AtomicUsize::new(0);
            let id = ATOMIC_ID.fetch_add(1, Ordering::Relaxed);
            format!("scale-robot-{}", id)
        })
        .enable_all()
        .build()
        .map_err(|e| ClientError::TokioRuntimeCreateField(e.to_string()))?;
    if app == App::Sui {
        let x = run_sui_app(runtime, opt);
        return x;
    } else if app == App::Aptos {
        return run_aptos_app(runtime, opt);
    } else {
        return Err(ClientError::ClientError(app.to_string()).into());
    }
}

fn new_symbol_id_vec(cfg: &SuiConfig) -> Vec<ws::SymbolId> {
    cfg.price_config
        .pyth_symbol
        .iter()
        .map(|s| ws::SymbolId {
            symbol: s.symbol.clone(),
            id: s.pyth_feed.clone(),
        })
        .collect::<Vec<ws::SymbolId>>()
}

fn new_price_feed_map(cfg: &SuiConfig) -> Arc<DmPriceFeed> {
    let price_feed = DmPriceFeed::new();
    for s in cfg.price_config.pyth_symbol.iter() {
        price_feed.insert(
            s.symbol.clone(),
            PriceFeed {
                feed_address: s.pyth_feed.clone(),
                price: 0,
                timestamp: 0,
            },
        );
    }
    Arc::new(price_feed)
}

fn run_sui_app(runtime: Runtime, mut opt: Options) -> anyhow::Result<()> {
    let mut conf = SuiConfig::default();
    config::config(&mut conf, opt.config_file.clone())?;
    let (sds, supported_symbol) = new_shared_dm_symbol_id(new_symbol_id_vec(&conf));
    let price_feed = new_price_feed_map(&conf);
    let mut state_mp = machine::StateMap::new(supported_symbol)?;
    // try load local state data
    // state_ssm.load_active_account_from_local()?;
    let mp: machine::SharedStateMap = Arc::new(state_mp);
    runtime.block_on(async move {
        let tool: Tool;
        match Tool::new(conf.clone(), opt.gas_budget).await {
            Ok(t) => {
                tool = t;
            }
            Err(e) => {
                error!("tool init error: {}", e);
                return;
            }
        }
        let run = run_bot(opt, Arc::new(conf.clone()), Arc::new(tool)).await;
        // let (watch, liquidation, ws_client, http_server, oracle) = match run {
        //     Ok(r) => r,
        //     Err(e) => {
        //         error!("run bot error: {}", e);
        //         return;
        //     }
        // };
        // let ctx = SuiContext::new(conf).await.expect("sui context init error");
        // if let Err(e) = subscribe::sync_all_objects(ctx.clone(), watch.watch_tx.clone()).await {
        //     error!("sync all orders error: {}", e);
        // }
        // let (_sync_tx, sync_rx) = mpsc::unbounded_channel();
        // // start event task
        // let event_task =
        //     subscribe::EventSubscriber::new(ctx.clone(), watch.watch_tx.clone(), sync_rx).await;
        info!("bot start success");
        signal::ctrl_c().await.expect("failed to listen for event");
        info!("Ctrl-C received, shutting down");
        // event_task.shutdown().await;
        // if let Some(http_srv) = http_server {
        //     http_srv.shutdown().await;
        // }
        // ws_client.shutdown().await;
        // liquidation.shutdown().await;
        // if let Some(oracle) = oracle {
        //     oracle.shutdown().await;
        // }
    });
    return Ok(());
}

fn run_aptos_app(_runtime: Runtime, _opt: Options) -> anyhow::Result<()> {
    Ok(())
}

async fn run_bot<C, CF>(opt: Options, conf: Arc<CF>, call: Arc<C>) -> anyhow::Result<()>
where
    C: MoveCall + Send + Sync + 'static,
    CF: Config + Send + Sync + 'static,
{
    // let (event_ws_tx, event_ws_rx) = ws::new_event_channel(100);
    if opt.full_node {
        let db = postgres::new(conf.get_sql_db_config()).await?;
    } else {
        let db = local::Local::new(conf.get_storage_path())?;
        let influxdb = influxdb::Influxdb::new(conf.get_influxdb_config());
    }
    Ok(())
}
