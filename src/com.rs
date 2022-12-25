use thiserror::Error;

pub const SUI_SCALE_PUBLISH_TX: &str = "B2qPue9NUeUU7AaPQubLPRCfBcRMVtD5SgDw7hLHEhTL";
pub const SUI_COIN_PUBLISH_TX: &str = "xy81qUbWxbZEtmSpU5du2RdGiNzNx1e951GUqx36oek";

#[derive(Error, Debug)]
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
}
