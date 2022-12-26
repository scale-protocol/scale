use async_trait::async_trait;
use std::fmt;
#[derive(Clone, Debug, PartialEq)]
pub enum App {
    Sui,
    Aptos,
    None,
}

impl<'a> From<&'a str> for App {
    fn from(value: &'a str) -> Self {
        let c = value.as_bytes();
        match c {
            b"sui" => Self::Sui,
            b"aptos" => Self::Aptos,
            _ => Self::None,
        }
    }
}
impl fmt::Display for App {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let t = match *self {
            Self::Sui => "sui",
            Self::Aptos => "aptos",
            Self::None => "None",
        };
        write!(f, "{}", t)
    }
}
#[async_trait]
pub trait Task {
    async fn stop(self) -> anyhow::Result<()>;
}
