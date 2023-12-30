use crate::app::App;
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

use crate::bot::{machine::Watch, ws::WsClient};

#[derive(Debug, Clone)]
pub struct Options {
    pub tasks: usize,
    pub socket_addr: Option<SocketAddr>,
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
    let mut opt = Options {
        tasks,
        socket_addr: None,
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

fn run_sui_app(runtime: Runtime, opt: Options) -> anyhow::Result<()> {
    let mut conf = SuiConfig::default();
    config::config(&mut conf, opt.config_file.clone())?;
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
        let (watch, ws_client) = match run {
            Ok(r) => r,
            Err(e) => {
                error!("run bot error: {}", e);
                return;
            }
        };
        let ctx = SuiContext::new(conf).await.expect("sui context init error");
        if let Err(e) = subscribe::sync_all_objects(ctx.clone(), watch.watch_tx.clone()).await {
            error!("sync all orders error: {}", e);
        }
        let (sync_tx, sync_rx) = mpsc::unbounded_channel();
        // // start event task
        let event_task =
            subscribe::EventSubscriber::new(ctx.clone(), watch.watch_tx.clone(), sync_rx).await;
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

async fn run_bot<C, CF>(
    opt: Options,
    conf: Arc<CF>,
    call: Arc<C>,
) -> anyhow::Result<(Watch, WsClient)>
where
    C: MoveCall + Send + Sync + 'static,
    CF: Config + Send + Sync + 'static,
{
    let (sds, supported_symbol) = new_shared_dm_symbol_id(conf.get_price_config().pyth_symbol);
    // let price_feed = new_price_feed_map(&conf);
    let mut state_mp = machine::StateMap::new(supported_symbol)?;
    // try load local state data
    // state_ssm.load_active_account_from_local()?;
    let ssm: machine::SharedStateMap = Arc::new(state_mp);
    let (event_ws_tx, event_ws_rx) = ws::new_event_channel(100);
    let influxdb = influxdb::Influxdb::new(conf.get_influxdb_config());
    let mut watch: Watch;
    let ws_client: WsClient;
    if opt.full_node {
        let db = Arc::new(postgres::new(conf.get_sql_db_config()).await?);
        watch = machine::Watch::new(ssm.clone(), db.clone(), event_ws_tx.clone(), true).await;
        ws_client = price::sub_price(
            watch.watch_tx.clone(),
            conf.get_price_config().ws_url.clone(),
            influxdb,
            sds.clone(),
            opt.full_node,
        )
        .await?;
    } else {
        let db = Arc::new(local::Local::new(conf.get_storage_path())?);
        watch = machine::Watch::new(ssm.clone(), db.clone(), event_ws_tx.clone(), false).await;
        ws_client = price::sub_price(
            watch.watch_tx.clone(),
            conf.get_price_config().ws_url.clone(),
            influxdb,
            sds.clone(),
            opt.full_node,
        )
        .await?;
    }
    Ok((watch, ws_client))
}
