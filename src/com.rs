use log::*;
use thiserror::Error;
use tokio::{
    runtime::Builder,
    sync::oneshot,
    task::JoinHandle,
    time::{self, Duration},
};

pub const SUI_COIN_PUBLISH_TX: &str = "9QTWwT2zjLenoSpcFwtLyHUL2RK2Pdws4c1GnEjXDBh3";
pub const SUI_ORACLE_PUBLISH_TX: &str = "GyYXXoBsPMhTApHXdsiDMVnZohgrmzhZuKR2L95rDNHV";
pub const SUI_NFT_PUBLISH_TX: &str = "2yby3YtAvvkGkTiez31tNHEmSpsP2iA9Y5wfA3kom8xL";
pub const SUI_SCALE_PUBLISH_TX: &str = "54dffpfTUAYJftCqi27ph7adswTMnN4UztZLFsbdnV69";

pub const DECIMALS: u64 = 1000000;
pub const DENOMINATOR: u64 = 10000;
pub const DENOMINATOR128: u64 = 10000;

#[derive(Error, Debug, PartialEq)]
pub enum CliError {
    #[error("Unknown error: {0}")]
    CliError(String),
    #[error("Invalid cli params: {0}")]
    InvalidCliParams(String),
    #[error("can not load scale config: {0}")]
    CanNotLoadScaleConfig(String),
    #[error("sui active address not found")]
    ActiveEnvNotFound,
    #[error("http server error: {0}")]
    HttpServerError(String),
    #[error("tokio runtime create field: {0}")]
    TokioRuntimeCreateField(String),
    #[error("websocket connection closed")]
    WebSocketConnectionClosed,
    #[error("db error: {0}")]
    DBError(String),
    #[error("cron start error: {0}")]
    CronError(String),
    #[error("Error in json parsing:{0}")]
    JsonError(String),
    #[error("Error in rpc:{0}")]
    RpcError(String),
    #[error("Error in websocket:{0}")]
    WebSocketError(String),
    #[error("unknown symbol params")]
    UnknownSymbol,
    #[error("invalid range params")]
    InvalidRange,
    #[error("invalid ws address signer")]
    InvalidWsAddressSigner,
    #[error("Get object error: {0}")]
    GetObjectError(String),
}

pub fn f64_round(f: f64) -> f64 {
    (f * 100.0).round() / 100.0
}

pub fn f64_round_4(f: f64) -> f64 {
    (f * 10000.0).round() / 10000.0
}

pub fn new_tokio_one_thread() -> tokio::runtime::Runtime {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime")
}

pub struct Task {
    shutdown_tx: oneshot::Sender<()>,
    job: JoinHandle<anyhow::Result<()>>,
    name: String,
}

impl Task {
    pub fn new(
        name: &str,
        shutdown_tx: oneshot::Sender<()>,
        job: JoinHandle<anyhow::Result<()>>,
    ) -> Self {
        Self {
            shutdown_tx,
            job,
            name: name.to_string(),
        }
    }

    pub async fn shutdown(self) {
        debug!("shutdown task {} ...", self.name);
        let _ = self.shutdown_tx.send(());
        if let Err(e) = time::timeout(Duration::from_micros(100), async {
            if let Err(e) = self.job.await {
                error!("task shutdown error: {:?}", e);
            }
        })
        .await
        {
            error!(
                "task shutdown await timeout: {:?}, error: {:?}",
                self.name, e
            );
        }
    }
}
