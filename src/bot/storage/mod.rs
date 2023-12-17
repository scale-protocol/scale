use std::fmt;
pub mod local;
pub mod postgres;

pub enum DbType {
    Local,
    Postgres,
}

impl<'a> From<&'a str> for DbType {
    fn from(value: &'a str) -> Self {
        match value {
            "local" => Self::Local,
            "postgres" => Self::Postgres,
            _ => Self::Postgres,
        }
    }
}

impl fmt::Display for DbType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let t = match *self {
            Self::Local => "local",
            Self::Postgres => "postgres",
        };
        write!(f, "{}", t)
    }
}
