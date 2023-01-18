use crate::bot::machine::Message;
use crate::com::CliError;
use crate::sui::config::Ctx;
use crate::sui::object;
use crate::sui::object::ObjectType;
use log::*;
use std::time::Duration;
use sui_sdk::rpc_types::{SuiEvent, SuiEventEnvelope, SuiEventFilter};
use sui_sdk::types::base_types::ObjectID;
use sui_types::{event::EventID, query::EventQuery};
use tokio::sync::{mpsc::UnboundedSender, watch};
use tokio::task::JoinHandle;
use tokio_stream::StreamExt;
pub struct EventSubscriber {
    task: JoinHandle<anyhow::Result<()>>,
    close_tx: watch::Sender<bool>,
}

impl EventSubscriber {
    pub async fn new(ctx: Ctx, watch_tx: UnboundedSender<Message>) -> Self {
        let (close_tx, close_rx) = watch::channel(false);
        Self {
            task: tokio::spawn(Self::run(ctx, close_rx, watch_tx)),
            close_tx,
        }
    }
    async fn run(
        ctx: Ctx,
        close_rx: watch::Receiver<bool>,
        watch_tx: UnboundedSender<Message>,
    ) -> anyhow::Result<()> {
        let mut next_retrying_time = Duration::from_secs(1);
        while let Err(e) =
            Self::loop_event_message(ctx.clone(), close_rx.clone(), watch_tx.clone()).await
        {
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
        ctx: Ctx,
        mut close_rx: watch::Receiver<bool>,
        watch_tx: UnboundedSender<Message>,
    ) -> anyhow::Result<()> {
        // let x = ObjectID::from_str("0x2").unwrap();
        let filter = SuiEventFilter::All(vec![
            SuiEventFilter::Package(ctx.config.scale_package_id),
            // SuiEventFilter::Module("enter".to_string()),
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
                    // debug!("event sub got result: {:?}", rs);
                    match rs {
                        Some(Ok(event)) => {
                            // debug!("event sub got event: {:?}", event);
                            if let Some(event_rs) = get_change_object(event) {
                                // debug!("event sub got event_rs: {:?}", event_rs);
                                if event_rs.object_type != ObjectType::None {
                                    if let Err(e) = object::pull_object(ctx.clone(), event_rs.object_id,watch_tx.clone()).await{
                                        error!("event sub got error: {:?}", e);
                                    }
                                }
                            }
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
    pub async fn shutdown(self) -> anyhow::Result<()> {
        self.close_tx.send(true)?;
        self.task.await??;
        info!("EventSubscriber stopped successfully!");
        Ok(())
    }
}
#[derive(Debug, Clone)]
pub struct EventResult {
    pub object_type: ObjectType,
    pub object_id: ObjectID,
    pub is_new: bool,
}

fn get_change_object(event: SuiEventEnvelope) -> Option<EventResult> {
    match event.event {
        SuiEvent::NewObject {
            package_id: _,
            transaction_module: _,
            sender: _,
            recipient: _,
            object_type,
            object_id,
            version: _,
        } => Some(EventResult {
            object_type: object_type.as_str().into(),
            object_id,
            is_new: true,
        }),
        SuiEvent::MutateObject {
            package_id: _,
            transaction_module: _,
            sender: _,
            object_type,
            object_id,
            version: _,
        } => Some(EventResult {
            object_type: object_type.as_str().into(),
            object_id,
            is_new: false,
        }),
        _ => None,
    }
}

pub async fn sync_all_objects(ctx: Ctx, watch_tx: UnboundedSender<Message>) -> anyhow::Result<()> {
    debug!("sync all objects");
    tokio::spawn(async move {
        // get all events
        let mut cursor: Option<EventID> = None;
        while let Ok(page) = ctx
            .client
            .event_api()
            .get_events(
                EventQuery::MoveModule {
                    package: ctx.config.scale_package_id,
                    module: "enter".to_string(),
                },
                cursor.clone(),
                Some(20),
                true,
            )
            .await
        {
            cursor = page.next_cursor;
            for event in page.data {
                if let Some(event_rs) = get_change_object(event) {
                    if event_rs.object_type != ObjectType::None && event_rs.is_new {
                        if let Err(e) =
                            object::pull_object(ctx.clone(), event_rs.object_id, watch_tx.clone())
                                .await
                        {
                            error!("event sub got error: {:?}", e);
                        }
                    }
                }
            }
            if cursor.is_none() {
                break;
            }
        }
    });
    Ok(())
}
