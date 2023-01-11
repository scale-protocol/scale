use thiserror::Error;

pub const SUI_SCALE_PUBLISH_TX: &str = "FkcB3LVmVLEuQ3M5froVnvo7NVu8oMeYut2FME1cJG81";
pub const SUI_COIN_PUBLISH_TX: &str = "4UjNEzcvZEJSRhDWKBvTgLmE13bSTMP9T3cY9ofJrejZ";

pub const DECIMALS: u64 = 1000000;

#[derive(Error, Debug, PartialEq)]
pub enum CliError {
    #[error("Unknown error: {0}")]
    Unknown(String),
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
    #[error("Error in websocket:{0}")]
    WebSocketError(String),
    #[error("Error in price subscribe,unknown symbol")]
    UnknownSymbol,
}
pub fn f64_round(f: f64) -> f64 {
    (f * 100.0).round() / 100.0
}
