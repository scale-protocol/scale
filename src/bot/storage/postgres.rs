use crate::{
    bot::state::{Address, MessageSender, Position, State, Storage},
    com::ClientError,
    config::SqlDbConfig,
};
use async_trait::async_trait;
use sea_orm::*;
use std::time::Duration;

pub struct PG {
    db: DatabaseConnection,
}

pub async fn new_pg(conf: SqlDbConfig) -> anyhow::Result<PG> {
    let mut opt = ConnectOptions::new(conf.db_url.as_str());
    opt.max_connections(conf.pool_max_conn)
        .min_connections(conf.pool_min_conn)
        .connect_timeout(Duration::from_secs(8))
        .acquire_timeout(Duration::from_secs(8))
        .idle_timeout(Duration::from_secs(8))
        .max_lifetime(Duration::from_secs(8))
        .sqlx_logging(true)
        .sqlx_logging_level(log::LevelFilter::Info);
    // .set_schema_search_path("my_schema"); // Setting default PostgreSQL schema
    let db = Database::connect(opt).await?;
    Ok(PG { db })
}
#[async_trait]
impl Storage for PG {
    async fn save_one(&self, state: State) -> anyhow::Result<()> {
        Ok(())
    }
    async fn load_all(&self, send: MessageSender) -> anyhow::Result<()> {
        Ok(())
    }
}
