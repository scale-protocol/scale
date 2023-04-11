use std::net::SocketAddr;

use crate::bot::influxdb::Influxdb;
use crate::bot::state::Address;
use crate::bot::{
    machine::SharedStateMap,
    ws::{PriceStatusWatchRx, PriceWatchRx, WsWatchRx},
};
use crate::com::CliError;
use crate::http::query::empty_string_as_none;
use crate::http::response::JsonResponse;
use crate::http::service;
use axum::{
    self,
    error_handling::HandleErrorLayer,
    extract::{ws::WebSocketUpgrade, Extension, Path, Query},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Router,
};
use log::*;
use serde::Deserialize;

use std::sync::Arc;
use std::{borrow::Cow, time::Duration};
use tokio::sync::oneshot;
use tower::{BoxError, ServiceBuilder};
use tower_http::trace::{DefaultMakeSpan, TraceLayer};

// use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
pub struct HttpServer {
    shutdown_tx: oneshot::Sender<()>,
    price_broadcast: service::PriceBroadcast,
}

impl HttpServer {
    pub async fn new(
        addr: &SocketAddr,
        mp: SharedStateMap,
        db: Arc<Influxdb>,
        event_ws_rx: WsWatchRx,
        price_ws_rx: PriceWatchRx,
    ) -> Self {
        let dps = service::new_price_status();
        let (price_broadcast, price_status_rx) =
            service::PriceBroadcast::new(mp.clone(), dps.clone(), price_ws_rx, db.clone()).await;
        let router = router(mp.clone(), db.clone(), price_status_rx, event_ws_rx);
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let server = axum::Server::bind(&addr)
            .serve(router.into_make_service())
            .with_graceful_shutdown(async {
                shutdown_rx.await.ok();
            });
        info!("start web server ...");
        tokio::spawn(async move {
            if let Err(e) = server.await {
                error!("server error: {}", e);
            }
        });
        tokio::spawn(async move {
            service::init_price_history_cache(mp, db).await;
        });
        Self {
            shutdown_tx,
            price_broadcast,
        }
    }

    pub async fn shutdown(self) {
        info!("send http server shutdown signal");
        let _ = self.shutdown_tx.send(());
        let _ = self.price_broadcast.shutdown().await;
    }
}

pub fn router(
    mp: SharedStateMap,
    db: Arc<Influxdb>,
    price_status_rx: PriceStatusWatchRx,
    event_ws_rx: WsWatchRx,
) -> Router {
    let app: Router = Router::new()
        .route("/account/info/:address", get(get_user_info))
        .route(
            "/account/positions/:prefix/:address",
            get(get_user_position_list),
        )
        .route(
            "/account/position/:address/:position_address",
            get(get_position_info),
        )
        .route("/markets/:prefix", get(get_market_list))
        .route("/symbols", get(get_symbol_list))
        .route("/price/history", get(get_price_history))
        .route("/price/history_full", get(get_price_history_column))
        .route("/ws", get(ws_handler))
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(handle_error))
                .load_shed()
                // .concurrency_limit(1024)
                .timeout(Duration::from_secs(60))
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(DefaultMakeSpan::default().include_headers(true)),
                ), // .into_inner(),
        )
        .layer(Extension(mp))
        .layer(Extension(price_status_rx))
        .layer(Extension(event_ws_rx))
        .layer(Extension(db));
    app.fallback(handler_404)
}

async fn handler_404() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        "Welcome to scale robot service. No resources found.",
    )
}

async fn get_user_info(
    Path(address): Path<String>,
    Extension(state): Extension<SharedStateMap>,
) -> impl IntoResponse {
    JsonResponse::from(service::get_account_info(state, address)).to_json()
}
async fn get_position_info(
    Path((address, position_address)): Path<(String, String)>,
    Extension(state): Extension<SharedStateMap>,
) -> impl IntoResponse {
    JsonResponse::from(service::get_position_info(state, address, position_address)).to_json()
}

async fn get_user_position_list(
    Path((prefix, address)): Path<(String, String)>,
    Extension(state): Extension<SharedStateMap>,
) -> impl IntoResponse {
    JsonResponse::from(service::get_position_list(state, prefix, address)).to_json()
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct HistoryParams {
    #[serde(default, deserialize_with = "empty_string_as_none")]
    range: Option<String>,
    symbol: Option<String>,
}
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct WsParams {
    #[serde(default, deserialize_with = "empty_string_as_none")]
    account: Option<String>,
}

async fn get_price_history(
    Query(q_m): Query<HistoryParams>,
    Extension(db): Extension<Arc<Influxdb>>,
) -> impl IntoResponse {
    let r = service::get_price_history(q_m.symbol, q_m.range, db).await;
    JsonResponse::from(r).to_json()
}

async fn get_price_history_column(
    Query(q_m): Query<HistoryParams>,
    Extension(db): Extension<Arc<Influxdb>>,
) -> impl IntoResponse {
    let r = service::get_price_history_column(q_m.symbol, q_m.range, db).await;
    JsonResponse::from(r).to_json()
}
async fn get_market_list(
    Path(prefix): Path<String>,
    Extension(state): Extension<SharedStateMap>,
) -> impl IntoResponse {
    let r = service::get_market_list(state, prefix).await;
    JsonResponse::from(r).to_json()
}

async fn get_symbol_list(Extension(state): Extension<SharedStateMap>) -> impl IntoResponse {
    let r = service::get_symbol_list(state).await;
    JsonResponse::from(r).to_json()
}
async fn handle_error(error: BoxError) -> impl IntoResponse {
    if error.is::<tower::timeout::error::Elapsed>() {
        return (StatusCode::REQUEST_TIMEOUT, Cow::from("request timed out"));
    }
    if error.is::<tower::load_shed::error::Overloaded>() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Cow::from("service is overloaded, try again later"),
        );
    }
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Cow::from(format!("Unhandled internal error: {}", error)),
    )
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(q): Query<WsParams>,
    Extension(state): Extension<SharedStateMap>,
    Extension(price_status_ws_rx): Extension<PriceStatusWatchRx>,
    Extension(event_ws_rx): Extension<WsWatchRx>,
) -> impl IntoResponse {
    let jr = JsonResponse::<()>::default();
    let mut address = None;
    if let Some(account) = q.account {
        if let Ok(add) = <Address as std::str::FromStr>::from_str(account.as_str()) {
            address = Some(add);
        } else {
            return jr
                .err(CliError::InvalidWsAddressSigner.into())
                .to_json()
                .into_response();
        }
    }
    return ws.on_upgrade(|socket| {
        service::handle_ws(state, socket, address, price_status_ws_rx, event_ws_rx)
    });
}
