// use super::{
//     machine::{self, Liquidation},
//     sub,
// };
// use crate::{
//     com,
//     http::router::{self, HttpServer},
// };
use crate::app::App;
use crate::aptos::config::Config as AptosConfig;
use crate::com::{self, CliError};
use crate::config;
use crate::sui::config::Config as SuiConfig;
use crate::sui::object;
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
use tokio::{runtime::Builder, signal, sync::mpsc};

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
        let mut conf = SuiConfig::default();
        config::config(&mut conf, config_file)?;
        runtime.spawn(async move {
            object::sub_sui_events(Arc::new(conf))
                .await
                .expect("sub sui events error");
        });
    } else {
        let mut conf = AptosConfig::default();
        config::config(&mut conf, config_file)?;
    }

    let s = runtime.block_on(async { signal::ctrl_c().await });
    match s {
        Ok(()) => {
            info!("got exit signal...start execution.")
        }
        Err(err) => {
            error!("Unable to listen for shutdown signal: {}", err);
        }
    }
    Ok(())
}
