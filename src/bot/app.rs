use crate::app::App;
use crate::bot::oracle::{DmPriceFeed, PriceFeed, PriceOracle};
use crate::bot::{
    influxdb, machine, price,
    state::MoveCall,
    ws::{self, new_shared_dm_symbol_id},
};
use crate::com::CliError;
use crate::config;
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
use tokio::time::Duration;
use tokio::{runtime::Builder, runtime::Runtime, signal};

pub fn run(app: App, config_file: Option<&PathBuf>, args: &clap::ArgMatches) -> anyhow::Result<()> {
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
    let duration = match args.get_one::<u64>("duration") {
        Some(d) => Duration::from_secs(*d),
        None => Duration::from_secs(0),
    };
    let address = format!("{}:{}", ip, port);
    let mut socket_addr: Option<SocketAddr> = None;
    if port > 0 {
        let addr = address
            .to_socket_addrs()
            .map_err(|e| CliError::HttpServerError(e.to_string()))?
            .next()
            .ok_or(CliError::HttpServerError("parsing none".to_string()))?;
        socket_addr = Some(addr);
    }
    let is_write_to_db = args.get_one::<bool>("write_price_to_db").unwrap_or(&true);
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
        .map_err(|e| CliError::TokioRuntimeCreateField(e.to_string()))?;
    if app == App::Sui {
        return run_sui_app(
            config_file,
            runtime,
            socket_addr,
            *is_write_to_db,
            duration,
            tasks,
        );
    } else if app == App::Aptos {
        return run_aptos_app(config_file, socket_addr, tasks, runtime);
    } else {
        return Err(CliError::CliError(app.to_string()).into());
    }
}

fn new_symbol_id_vec(cfg: &SuiConfig) -> Vec<ws::SymbolId> {
    cfg.price_config
        .pyth_symbol
        .iter()
        .map(|s| ws::SymbolId {
            symbol: s.symbol.clone(),
            id: s.id.clone(),
        })
        .collect::<Vec<ws::SymbolId>>()
}

fn new_price_feed_map(cfg: &SuiConfig) -> Arc<DmPriceFeed> {
    let price_feed = DmPriceFeed::new();
    for s in cfg.price_config.pyth_symbol.iter() {
        if let Some(p) = s.oracle_feed_address.clone() {
            price_feed.insert(
                s.symbol.clone(),
                PriceFeed {
                    feed_address: p,
                    price: 0,
                    timestamp: 0,
                },
            );
        }
    }
    Arc::new(price_feed)
}

fn run_sui_app(
    config_file: Option<&PathBuf>,
    runtime: Runtime,
    socket_addr: Option<SocketAddr>,
    is_write_price_db: bool,
    duration: Duration,
    tasks: usize,
) -> anyhow::Result<()> {
    let mut conf = SuiConfig::default();
    config::config(&mut conf, config_file)?;

    let (sds, supported_symbol) = new_shared_dm_symbol_id(new_symbol_id_vec(&conf));
    let price_feed = new_price_feed_map(&conf);
    let mut state_mp = machine::StateMap::new(conf.scale_store_path.clone(), supported_symbol)?;
    // try load local state data
    state_mp.load_active_account_from_local()?;
    let mp: machine::SharedStateMap = Arc::new(state_mp);
    runtime.block_on(async move {
        let tool = Tool::new(conf.clone()).await.expect("tool init error");
        let run = run_bot(
            mp.clone(),
            price_feed.clone(),
            sds.clone(),
            socket_addr,
            influxdb::InfluxdbConfig {
                url: conf.price_config.db.url.clone(),
                org: conf.price_config.db.org.clone(),
                bucket: conf.price_config.db.bucket.clone(),
                token: conf.price_config.db.token.clone(),
            },
            &conf.price_config.ws_url,
            is_write_price_db,
            tasks,
            duration,
            Arc::new(tool),
        )
        .await;
        let (watch, liquidation, ws_client, http_server, oracle) = match run {
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
        // start event task
        let event_task = subscribe::EventSubscriber::new(ctx.clone(), watch.watch_tx.clone()).await;
        signal::ctrl_c().await.expect("failed to listen for event");
        info!("Ctrl-C received, shutting down");
        let _ = event_task.shutdown().await;
        if let Some(http_srv) = http_server {
            let _ = http_srv.shutdown().await;
        }
        let _ = ws_client.shutdown().await;
        let _ = liquidation.shutdown().await;
        if let Some(oracle) = oracle {
            let _ = oracle.shutdown().await;
        }
    });
    Ok(())
}

fn run_aptos_app(
    _config_file: Option<&PathBuf>,
    _socket_addr: Option<SocketAddr>,
    _tasks: usize,
    _runtime: Runtime,
) -> anyhow::Result<()> {
    Ok(())
}

async fn run_bot<C>(
    mp: machine::SharedStateMap,
    dpf: Arc<DmPriceFeed>,
    sds: ws::SharedDmSymbolId,
    socket_addr: Option<SocketAddr>,
    ic: influxdb::InfluxdbConfig,
    price_ws_url: &str,
    is_write_db: bool,
    tasks: usize,
    duration: Duration,
    call: Arc<C>,
) -> anyhow::Result<(
    machine::Watch,
    machine::Liquidation,
    ws::WsClient,
    Option<HttpServer>,
    Option<PriceOracle>,
)>
where
    C: MoveCall + Send + Sync + 'static,
{
    let watch = machine::Watch::new(mp.clone()).await;
    let liquidation = machine::Liquidation::new(mp.clone(), tasks, call.clone()).await?;
    let db = influxdb::Influxdb::new(ic);

    let (price_ws_client, price_ws_rx) = price::sub_price(
        watch.watch_tx.clone(),
        price_ws_url.to_string(),
        db.clone(),
        sds.clone(),
        is_write_db,
        socket_addr.is_some() || duration.as_secs() > 0,
    )
    .await?;
    let http_srv = if let Some(addr) = socket_addr {
        Some(
            HttpServer::new(
                &addr,
                mp.clone(),
                Arc::new(db),
                sds,
                price_ws_rx.resubscribe(),
            )
            .await,
        )
    } else {
        None
    };
    let oracle = if duration.as_secs() > 0 {
        Some(PriceOracle::new(dpf.clone(), price_ws_rx, duration, call).await)
    } else {
        None
    };
    Ok((watch, liquidation, price_ws_client, http_srv, oracle))
}
