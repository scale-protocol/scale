use crate::sui::config::Config;
use crate::{app::Task, com::CliError};
use async_trait::async_trait;
use log::*;
use std::sync::Arc;
use std::time::Duration;
use sui_sdk::rpc_types::{SuiEvent, SuiEventEnvelope, SuiEventFilter};
use sui_sdk::types::event;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio_stream::StreamExt;
pub struct EventSubscriber {
    task: JoinHandle<anyhow::Result<()>>,
    close_tx: watch::Sender<bool>,
}

impl EventSubscriber {
    pub async fn new(config: Arc<Config>) -> Self {
        let (close_tx, close_rx) = watch::channel(false);
        Self {
            task: tokio::spawn(Self::run(config, close_rx)),
            close_tx,
        }
    }
    async fn run(config: Arc<Config>, close_rx: watch::Receiver<bool>) -> anyhow::Result<()> {
        let mut next_retrying_time = Duration::from_secs(1);
        while let Err(e) = Self::loop_event_message(config.clone(), close_rx.clone()).await {
            debug!("Error: {:?}", e);
            let err = e.root_cause().downcast_ref::<CliError>();
            if let Some(ce) = err {
                debug!("CliError: {:?}", ce);
                if *ce == CliError::WebSocketConnectionClosed {
                    error!(
                        "WebSocket connection closed, retrying in {:?} seconds",
                        next_retrying_time
                    );
                    tokio::time::sleep(next_retrying_time).await;
                    next_retrying_time = next_retrying_time * 2;
                } else {
                    return Err(e);
                }
            } else {
                return Err(e);
            }
        }
        Ok(())
    }
    async fn loop_event_message(
        config: Arc<Config>,
        mut close_rx: watch::Receiver<bool>,
    ) -> anyhow::Result<()> {
        let sui_client_config = config.get_sui_config()?;
        let client = sui_client_config
            .get_active_env()?
            .create_rpc_client(Some(Duration::from_secs(1000)))
            .await?;
        let filter = SuiEventFilter::All(vec![
            SuiEventFilter::Package(config.scale_package_id),
            SuiEventFilter::Module("in".to_string()),
        ]);
        let mut sub = client.event_api().subscribe_event(filter).await?;
        loop {
            tokio::select! {
                _ = close_rx.changed() => {
                    return Ok(());
                }
                rs = sub.next() => {
                    debug!("event sub got result: {:?}", rs);
                    match rs {
                        Some(Ok(event)) => {
                            debug!("event sub got event: {:?}", event);
                            Self::handle_event(event);
                        }
                        Some(Err(e)) => {
                            error!("event sub got error: {:?}", e);
                            return Err(e.into());
                        }
                        None => {
                            debug!("event sub got None");
                            return Ok(());
                        }
                    }
                }
            }
        }
    }
    fn handle_event(event: SuiEventEnvelope) {
        if let SuiEvent::NewObject {
            package_id,
            transaction_module,
            sender,
            recipient,
            object_type,
            object_id,
            version,
        } = event.event
        {
            println!("New object: {:?}", object_id);
        }
    }
}

#[async_trait]
impl Task for EventSubscriber {
    async fn stop(self) -> anyhow::Result<()> {
        self.close_tx.send(true);
        self.task.await?;
        info!("EventSubscriber stopped successfully!");
        Ok(())
    }
}
