use thiserror::Error;
use tokio;

pub const SUI_SCALE_PUBLISH_TX: &str = "BKjZ49tyXBPKDq8wU9Wq6R8d5h1dMXoUMKd1zX1qnAhJ";
pub const SUI_COIN_PUBLISH_TX: &str = "2WAXGjqr9aqpMgwM8juwFRLy7UNyL4yquuVTawwLoj2U";
pub const SUI_ORACLE_PUBLISH_TX: &str = "FdaFZ7isCRBcDsspfv9RWYyEt24QAH2np4WiNiX9Fb9h";

pub const DECIMALS: u64 = 1000000;

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
}

pub fn f64_round(f: f64) -> f64 {
    (f * 100.0).round() / 100.0
}

pub fn new_tokio_one_thread() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime")
}
