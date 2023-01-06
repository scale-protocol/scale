use thiserror::Error;

pub const SUI_SCALE_PUBLISH_TX: &str = "FkcB3LVmVLEuQ3M5froVnvo7NVu8oMeYut2FME1cJG81";
pub const SUI_COIN_PUBLISH_TX: &str = "4UjNEzcvZEJSRhDWKBvTgLmE13bSTMP9T3cY9ofJrejZ";

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
}
