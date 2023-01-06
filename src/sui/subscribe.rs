use crate::sui::config::Context;
use crate::sui::object::{self, ObjectType};
use crate::{app::Task, com::CliError};
use async_trait::async_trait;
use log::*;
use std::sync::Arc;
use std::time::Duration;
use sui_sdk::rpc_types::{SuiEvent, SuiEventEnvelope, SuiEventFilter};
use sui_sdk::types::base_types::ObjectID;
use sui_types::{event::EventID, query::EventQuery};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio_stream::StreamExt;
pub struct EventSubscriber {
    task: JoinHandle<anyhow::Result<()>>,
    close_tx: watch::Sender<bool>,
}

impl EventSubscriber {
    pub async fn new(ctx: Arc<Context>) -> Self {
        let (close_tx, close_rx) = watch::channel(false);
        Self {
            task: tokio::spawn(Self::run(ctx, close_rx)),
            close_tx,
        }
    }
    async fn run(ctx: Arc<Context>, close_rx: watch::Receiver<bool>) -> anyhow::Result<()> {
        let mut next_retrying_time = Duration::from_secs(1);
        while let Err(e) = Self::loop_event_message(ctx.clone(), close_rx.clone()).await {
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
                    if next_retrying_time > Duration::from_secs(90) {
                        next_retrying_time = Duration::from_secs(1);
                    }
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
        ctx: Arc<Context>,
        mut close_rx: watch::Receiver<bool>,
    ) -> anyhow::Result<()> {
        // let x = ObjectID::from_str("0x2").unwrap();
        let filter = SuiEventFilter::All(vec![
            SuiEventFilter::Package(ctx.config.scale_package_id),
            // SuiEventFilter::Module("in".to_string()),
        ]);
        let mut sub = ctx.client.event_api().subscribe_event(filter).await?;
        debug!("event sub created");
        loop {
            tokio::select! {
                _ = close_rx.changed() => {
                    debug!("event sub got close signal");
                    return Ok(());
                }
                rs = sub.next() => {
                    debug!("event sub got result: {:?}", rs);
                    match rs {
                        Some(Ok(event)) => {
                            debug!("event sub got event: {:?}", event);
                            // Self::handle_event(event);
                        }
                        Some(Err(e)) => {
                            error!("event sub got error: {:?}", e);
                            return Err(CliError::WebSocketConnectionClosed.into());
                        }
                        None => {
                            debug!("event sub got None");
                            return Err(CliError::WebSocketConnectionClosed.into());
                        }
                    }
                }
            }
        }
    }

    fn get_change_object(event: SuiEventEnvelope) -> (ObjectType, Option<ObjectID>) {
        match event.event {
            SuiEvent::NewObject {
                package_id: _,
                transaction_module: _,
                sender: _,
                recipient: _,
                object_type,
                object_id,
                version: _,
            } => (object_type.as_str().into(), Some(object_id)),
            SuiEvent::MutateObject {
                package_id: _,
                transaction_module: _,
                sender: _,
                object_type,
                object_id,
                version: _,
            } => (object_type.as_str().into(), Some(object_id)),
            _ => (ObjectType::None, None),
        }
    }
}

#[async_trait]
impl Task for EventSubscriber {
    async fn stop(self) -> anyhow::Result<()> {
        self.close_tx.send(true)?;
        self.task.await??;
        info!("EventSubscriber stopped successfully!");
        Ok(())
    }
}

pub async fn sync_all_objects(ctx: Arc<Context>) -> anyhow::Result<()> {
    debug!("sync_all_objects");
    tokio::spawn(async move {
        // get all events
        let mut cursor: Option<EventID> = None;
        while let Ok(page) = ctx
            .client
            .event_api()
            .get_events(
                EventQuery::MoveModule {
                    package: ctx.config.scale_package_id,
                    module: "in".to_string(),
                },
                cursor.clone(),
                Some(20),
                Some(true),
            )
            .await
        {
            cursor = page.next_cursor;
            debug!("got data: {:?}", page.data);
            for event in page.data {}
            if cursor.is_none() {
                break;
            }
        }
    });

    Ok(())
}
