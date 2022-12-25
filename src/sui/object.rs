use crate::sui::config::Config;
use log::*;
use std::sync::Arc;
use std::time::Duration;
use sui_sdk::rpc_types::{SuiEvent, SuiEventFilter};
// use sui_sdk::types::event;
// use sui_sdk::types::{
//     base_types::{ObjectID, SuiAddress, TransactionDigest},
//     event::Event,
// };
// use sui_sdk::SuiClient;
use tokio_stream::StreamExt;

pub async fn sub_sui_events(config: Arc<Config>) -> anyhow::Result<()> {
    let sui_client_config = config.get_sui_config()?;
    // let client = SuiClient::new(http_url, ws_url, request_timeout)
    let client = sui_client_config
        .get_active_env()?
        .create_rpc_client(Some(Duration::from_secs(1000)))
        .await?;
    let filter = SuiEventFilter::All(vec![
        SuiEventFilter::Package(config.scale_package_id),
        SuiEventFilter::Module("in".to_string()),
        // SuiEventFilter::EventType(Event::NewObject {_}),
    ]);
    let mut sub = client.event_api().subscribe_event(filter).await?;
    loop {
        if let Some(rs) = sub.next().await {
            match rs {
                Ok(event) => match event.event {
                    SuiEvent::NewObject {
                        package_id,
                        transaction_module,
                        sender,
                        recipient,
                        object_type,
                        object_id,
                        version,
                    } => {
                        println!("New object: {:?}", object_id);
                    }
                    _ => {}
                },
                Err(e) => {
                    error!("Error: {:?}", e);
                    break;
                }
            }
        } else {
            debug!("Event stream closed");
            break;
        }
    }
    Ok(())
}
