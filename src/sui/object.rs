use crate::sui::config::Config;
use crate::{app::Task, com::CliError};
use async_trait::async_trait;
use log::*;
use std::sync::Arc;
use std::time::Duration;
use sui_sdk::rpc_types::{SuiEvent, SuiEventFilter};
use tokio_stream::StreamExt;
pub struct EventSubscriber {
    is_closed: bool,
    config: Arc<Config>,
}

impl EventSubscriber {
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            is_closed: false,
            config,
        }
    }
    async fn run(&self) -> anyhow::Result<()> {
        while let Err(e) = self.loop_message().await {
            debug!("Error: {:?}", e);
            if let Some(ce) = e.root_cause().downcast_ref::<CliError>() {
                debug!("CliError: {:?}", ce);
                if *ce == CliError::WebSocketConnectionClosed {
                    error!("WebSocket connection closed, retrying in 5 seconds");
                    tokio::time::sleep(Duration::from_secs(2)).await;
                } else {
                    return Err(e);
                }
            } else {
                return Err(e);
            }
        }
        Ok(())
    }
    async fn loop_message(&self) -> anyhow::Result<()> {
        let sui_client_config = self.config.get_sui_config()?;
        let client = sui_client_config
            .get_active_env()?
            .create_rpc_client(Some(Duration::from_secs(1000)))
            .await?;
        let filter = SuiEventFilter::All(vec![
            SuiEventFilter::Package(self.config.scale_package_id),
            SuiEventFilter::Module("in".to_string()),
        ]);
        let mut sub = client.event_api().subscribe_event(filter).await?;
        loop {
            if let Some(rs) = sub.next().await {
                match rs {
                    Ok(event) => {
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
                    Err(e) => {
                        error!("Error: {:?}", e);
                        return Err(CliError::WebSocketConnectionClosed.into());
                    }
                }
            } else {
                debug!("Event stream closed");
                break;
            }
        }
        Ok(())
    }
}

#[async_trait]
impl Task for EventSubscriber {
    async fn start(&self) -> anyhow::Result<()> {
        self.run().await
    }
    async fn stop(&self) -> anyhow::Result<()> {
        Ok(())
    }
    fn is_stopped(&self) -> bool {
        self.is_closed
    }
}
