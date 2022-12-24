use thiserror::Error;

pub const SUI_SCALE_PUBLISH_TX: &str = "B2qPue9NUeUU7AaPQubLPRCfBcRMVtD5SgDw7hLHEhTL";
pub const SUI_COIN_PUBLISH_TX: &str = "3GPNS9EESDtTJ22Sndf9hNRALzht4ARhY3eZjfCs1g2a";

#[derive(Error, Debug)]
pub enum CliError {
    #[error("Unknown error: {0}")]
    Unknown(String),
    #[error("can not load scale config: {0}")]
    CanNotLoadScaleConfig(String),
    #[error("sui active address not found")]
    ActiveEnvNotFound,
}
