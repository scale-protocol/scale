use crate::bot::machine::Message;
use crate::sui::config::Ctx;
use crate::sui::object;
use crate::sui::object::ObjectType;
use log::*;
// use std::time::Duration;
use sui_sdk::rpc_types::{SuiEvent, SuiEventEnvelope, SuiEventFilter};
use sui_sdk::types::base_types::ObjectID;
use sui_types::{event::EventID, query::EventQuery};
use tokio::sync::{mpsc::UnboundedSender, watch};
use tokio::{
    task::JoinHandle,
    time::{self, Duration},
};
// use tokio_stream::StreamExt;
use futures::StreamExt;

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
        mut close_rx: watch::Receiver<bool>,
        watch_tx: UnboundedSender<Message>,
    ) -> anyhow::Result<()> {
        'connection: loop {
            debug!("event sub connecting ...");
            let filter = SuiEventFilter::All(vec![
                SuiEventFilter::Package(ctx.config.scale_package_id),
                // SuiEventFilter::Module("enter".to_string()),
            ]);
            let client = ctx.config
            .get_sui_config()?
            .get_active_env()?
            .create_rpc_client(Some(Duration::from_secs(10)))
            .await?;
            let mut sub = client.event_api().subscribe_event(filter).await?;
            debug!("event sub created ...");
            // let mut timer = time::interval(Duration::from_secs(5));
            'sub: loop {
                tokio::select! {
                    _ = close_rx.changed() => {
                        debug!("event sub got close signal");
                        break 'connection;
                    }
                    rs = sub.next() =>{
                        match rs {
                            Some(Ok(event)) => {
                                debug!("event sub got event: {:?}", event);
                                if let Some(event_rs) = get_change_object(event) {
                                    // debug!("event sub got event_rs: {:?}", event_rs);
                                    if event_rs.object_type != ObjectType::None {
                                        if let Err(e) = object::pull_object(ctx.clone(), event_rs.object_id,watch_tx.clone()).await {
                                            error!("pull object error: {:?}", e);
                                        }
                                    }
                                }
                            }
                            Some(Err(e)) => {
                                error!("event sub got error: {:?}", e);
                            }
                            None => {
                                debug!("event sub got None");
                                break 'sub;
                            }
                        }
                    }
                }
            }
            drop(sub);
            info!("sui event sub reconnecting ...");
        }
        Ok(())
    }

    pub async fn shutdown(self) {
        debug!("EventSubscriber shutdown");
        if let Err(e) = self.close_tx.send(true) {
            error!("EventSubscriber close_tx send error: {:?}", e);
        }
        if let Err(e) = time::timeout(Duration::from_secs(2), async {
            if let Err(e) = self.task.await {
                error!("task shutdown error: {:?}", e);
            }
        })
        .await
        {
            error!("task shutdown await timeout: {:?}", e);
        }
        info!("EventSubscriber stopped successfully!");
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
        SuiEvent::TransferObject {
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
                Some(2),
                true,
            )
            .await
        {
            cursor = page.next_cursor;
            for event in page.data {
                if let Some(event_rs) = get_change_object(event) {
                    if event_rs.object_type != ObjectType::None && event_rs.is_new {
                        debug!("sync object: {:?}", event_rs);
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
