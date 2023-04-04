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
#[derive(Debug, Clone)]
pub struct PriceFeed {
    pub feed_address: String,
    pub price: i64,
    pub timestamp: i64,
}

impl PriceOracle {
    pub async fn new<C>(
        dpf: Arc<DmPriceFeed>,
        price_ws_rx: PriceWatchRx,
        duration: i64,
        call: Arc<C>,
    ) -> Self
    where
        C: MoveCall + Send + Sync + 'static,
    {
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        if duration == 0 {
            let task = Task::new(
                "price now oracle",
                shutdown_tx,
                tokio::spawn(update_price_now(dpf, price_ws_rx, shutdown_rx, call)),
            );
            return Self { task };
        } else {
            let task = Task::new(
                "price interval oracle",
                shutdown_tx,
                tokio::spawn(update_price_interval(
                    dpf,
                    price_ws_rx,
                    Duration::from_secs(duration as u64),
                    shutdown_rx,
                    call,
                )),
            );
            return Self { task };
        }
    }
    pub async fn shutdown(self) {
        self.task.shutdown().await;
    }
}

async fn update_price_interval<C>(
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
    info!("start interval price oracle task");
    loop {
        tokio::select! {
            _ = (&mut shutdown_rx) => {
                info!("got shutdown signal , break price broadcast!");
                break;
            },
            Ok(price) = price_ws_rx.0.recv() => {
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

async fn update_price_now<C>(
    dpf: Arc<DmPriceFeed>,
    mut price_ws_rx: PriceWatchRx,
    mut shutdown_rx: oneshot::Receiver<()>,
    call: Arc<C>,
) -> anyhow::Result<()>
where
    C: MoveCall + Send + Sync + 'static,
{
    info!("start now price oracle task");
    loop {
        tokio::select! {
            _ = (&mut shutdown_rx) => {
                info!("got shutdown signal , break price broadcast!");
                break;
            },
            Ok(price) = price_ws_rx.0.recv() => {
                if let Err(e) = update_time_now(dpf.clone(),&price,call.clone()).await {
                    error!("update price status error: {}", e);
                }
            }
        }
    }
    Ok(())
}

async fn update_time_now<C>(
    dpf: Arc<DmPriceFeed>,
    org_price: &OrgPrice,
    call: Arc<C>,
) -> anyhow::Result<()>
where
    C: MoveCall + Send + Sync + 'static,
{
    let record = dpf.get(&org_price.symbol);
    debug!("oracle update price now: {:?}", org_price);
    if let Some(record) = record {
        let mut price_feed = record.value().clone();
        price_feed.price = org_price.price;
        price_feed.timestamp = org_price.update_time;
        if price_feed.price == 0 {
            warn!("price is 0, skip it: {:?}", org_price.symbol);
            return Ok(());
        }
        call.update_price(price_feed.feed_address.as_str(), price_feed.price as u64)
            .await?;
    } else {
        debug!(
            "symbol {} ,cannot found price feed record",
            org_price.symbol
        );
    }
    Ok(())
}

fn recv_price(dpf: Arc<DmPriceFeed>, org_price: &OrgPrice) -> anyhow::Result<()> {
    let record = dpf.get_mut(&org_price.symbol);
    debug!("oracle recv price: {:?}", org_price);
    if let Some(mut record) = record {
        let price_feed = record.value_mut();
        price_feed.price = org_price.price;
        price_feed.timestamp = org_price.update_time;
    } else {
        debug!(
            "symbol {} ,cannot found price feed record",
            org_price.symbol
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
