use crate::bot::state::OrgPrice;
use crate::bot::{state::MoveCall, ws::PriceWatchRx};
use crate::com::Task;
use chrono::Utc;
use dashmap::DashMap;
use log::*;
use std::sync::Arc;
use tokio::{
    sync::oneshot,
    time::{self, Duration},
};

// key: symbol , value: price
pub type DmPriceFeed = DashMap<String, PriceFeed>;
pub struct PriceOracle {
    task: Task,
}
pub struct PriceFeed {
    pub feed_address: String,
    pub price: i64,
    pub timestamp: i64,
}

impl PriceOracle {
    pub async fn new<C>(
        dpf: Arc<DmPriceFeed>,
        price_ws_rx: PriceWatchRx,
        duration: Duration,
        call: Arc<C>,
    ) -> Self
    where
        C: MoveCall + Send + Sync + 'static,
    {
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let task = Task::new(
            "price oracle",
            shutdown_tx,
            tokio::spawn(update_price(dpf, price_ws_rx, duration, shutdown_rx, call)),
        );
        Self { task }
    }
    pub async fn shutdown(self) {
        self.task.shutdown().await;
    }
}

async fn update_price<C>(
    dpf: Arc<DmPriceFeed>,
    mut price_ws_rx: PriceWatchRx,
    duration: Duration,
    mut shutdown_rx: oneshot::Receiver<()>,
    call: Arc<C>,
) -> anyhow::Result<()>
where
    C: MoveCall + Send + Sync + 'static,
{
    let mut timer = time::interval(duration);
    loop {
        tokio::select! {
            _ = (&mut shutdown_rx) => {
                info!("got shutdown signal , break price broadcast!");
                break;
            },
            Ok(price) = price_ws_rx.recv() => {
                debug!("oracle recv price: {:?}", price);
                if let Err(e) = recv_price(dpf.clone(),&price) {
                    error!("receiver and save oracle price error: {}", e);
                }
            }
            _ = timer.tick() => {
                if let Err(e) = update_price_to_oracle(dpf.clone(), call.clone()).await {
                    error!("update price status error: {}", e);
                }
            }
        }
    }
    Ok(())
}

fn recv_price(dpf: Arc<DmPriceFeed>, org_price: &OrgPrice) -> anyhow::Result<()> {
    let record = dpf.get(&org_price.symbol);
    if let Some(record) = record {
        let price_feed = record.value();
        dpf.insert(
            org_price.symbol.clone(),
            PriceFeed {
                feed_address: price_feed.feed_address.clone(),
                price: org_price.price,
                timestamp: org_price.update_time,
            },
        );
    }
    Ok(())
}

async fn update_price_to_oracle<C>(dpf: Arc<DmPriceFeed>, call: Arc<C>) -> anyhow::Result<()>
where
    C: MoveCall + Send + Sync + 'static,
{
    for feed in dpf.iter() {
        debug!(
            "update price to oracle {:?} to {:?}",
            feed.key(),
            feed.value().price
        );
        if feed.value().price == 0 {
            warn!("price is 0, skip it: {:?}", feed.key());
            continue;
        }
        if Utc::now().timestamp() - feed.value().timestamp > 300 {
            warn!("price is too old, skip it: {:?}", feed.key());
            continue;
        }
        call.update_price(
            feed.value().feed_address.as_str(),
            feed.value().price as u64,
        )
        .await?;
    }
    Ok(())
}
