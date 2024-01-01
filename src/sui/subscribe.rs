use std::str::FromStr;

use crate::bot::state::{Event, EventSyncRx, MessageSender};
use crate::com::{Task, TaskStopRx};
use crate::sui::config::Ctx;
use crate::sui::object;
use crate::sui::object::ObjectType;
use log::*;
use move_core_types::{identifier::Identifier, language_storage::TypeTag};
use sui_sdk::rpc_types::{EventFilter, SuiEvent};
use sui_sdk::types::base_types::ObjectID;
use sui_types::event::EventID;
// use tokio_stream::StreamExt;
use futures::StreamExt;
use serde_json::Value;

pub struct EventSubscriber {
    task: Task,
}

impl EventSubscriber {
    pub async fn new(ctx: Ctx, watch_tx: MessageSender, sync_rx: EventSyncRx) -> Self {
        let (close_tx, close_rx) = Task::new_shutdown_channel();
        Self {
            task: Task::new(
                "event sub",
                close_tx,
                tokio::spawn(Self::run(ctx, close_rx, watch_tx, sync_rx)),
            ),
        }
    }
    async fn run(
        ctx: Ctx,
        mut close_rx: TaskStopRx,
        watch_tx: MessageSender,
        mut sync_rx: EventSyncRx,
    ) -> anyhow::Result<()> {
        'connection: loop {
            info!("event sub connecting ...");
            // let filter = EventFilter::All(vec![
            //     EventFilter::Package(ctx.config.scale_package_id),
            //     // SuiEventFilter::Module("enter".to_string()),
            // ]);
            let filter = EventFilter::Package(ctx.config.scale_package_id);
            // todo: If the server is not running for the first time, it will continuously retry.
            let client = ctx.wallet.get_client().await?;
            let mut sub = client.event_api().subscribe_event(filter).await?;
            debug!("event sub created ...");
            // let mut timer = time::interval(Duration::from_secs(5));
            'sub: loop {
                tokio::select! {
                    r = &mut close_rx => {
                        debug!("event sub got close signal: {:?}", r);
                        break 'connection;
                    }
                    rs = sub.next() =>{
                        debug!("event sub got next: {:?}", rs);
                        match rs {
                            Some(Ok(event)) => {
                                debug!("event sub got event: {:?}", event);
                                if let Some(event_rs) = get_change_object(event) {
                                    // debug!("event sub got event_rs: {:?}", event_rs);
                                    if event_rs.object_type != ObjectType::None {
                                        match object::pull_object(ctx.clone(), event_rs.object_id).await{
                                            Ok(mut msg) => {
                                                debug!("pull object success: {:?}", msg);
                                                msg.event = event_rs.event;
                                                if let Err(e) = watch_tx.send(msg) {
                                                    error!("watch_tx send error: {:?}", e);
                                                }
                                            }
                                            Err(e) => {
                                                error!("pull object error: {:?}", e);
                                            }
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
                    id = sync_rx.recv() => {
                        debug!("event sync got sync object id: {:?}", id);
                        if let Some(id) = id {
                            match ObjectID::from_str(id.to_string().as_str()){
                                Ok(id) => {
                                    if let Err(e) = object::pull_object(ctx.clone(), id).await{
                                        error!("pull object error: {:?}", e);
                                    }
                                }
                                Err(e) => {
                                    error!("parse object id error: {:?}", e);
                                }
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
        self.task.shutdown().await;
    }
}
#[derive(Debug, Clone)]
pub struct EventResult {
    pub object_type: ObjectType,
    pub object_id: ObjectID,
    pub event: Event,
}

fn get_change_object(event: SuiEvent) -> Option<EventResult> {
    if event.type_.type_params.len() > 0 {
        if let TypeTag::Struct(v) = &event.type_.type_params[0] {
            if let Value::Object(obj) = &event.parsed_json {
                let ot: ObjectType = v.name.as_str().into();
                return Some(EventResult {
                    object_type: ot,
                    object_id: ObjectID::from_str(obj.get("id").unwrap().as_str().unwrap())
                        .unwrap(),
                    event: event.type_.name.as_str().into(),
                });
            }
        }
    }
    None
}

pub async fn sync_all_objects(ctx: Ctx, watch_tx: MessageSender) -> anyhow::Result<()> {
    info!("start sync all objects");
    tokio::spawn(async move {
        // get all events
        let mut cursor: Option<EventID> = None;
        let mut object_ids: Vec<ObjectID> = Vec::new();
        while let Ok(page) = ctx
            .client
            .event_api()
            .query_events(
                EventFilter::MoveModule {
                    package: ctx.config.scale_package_id,
                    module: Identifier::from_utf8("enter".as_bytes().to_vec()).unwrap(),
                },
                cursor.clone(),
                Some(100),
                true,
            )
            .await
        {
            cursor = page.next_cursor;
            for event in page.data {
                if let Some(event_rs) = get_change_object(event) {
                    if event_rs.object_type != ObjectType::None && event_rs.event == Event::Created
                    {
                        debug!("sync object: {:?}", event_rs);
                        object_ids.push(event_rs.object_id);
                    }
                }
            }
            if page.has_next_page == false || cursor.is_none() {
                break;
            }
        }
        if let Err(e) =
            object::pull_objects_and_send(ctx.clone(), object_ids, Event::Created, watch_tx).await
        {
            error!("pull objects error: {:?}", e);
        }
    });
    info!("end sync all objects");
    Ok(())
}
