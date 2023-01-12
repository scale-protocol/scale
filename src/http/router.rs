use std::net::SocketAddr;

use crate::bot::influxdb::Influxdb;
use crate::bot::machine::SharedStateMap;
use crate::http::query::empty_string_as_none;
use crate::http::response::JsonResponse;
use axum::{
    self,
    error_handling::HandleErrorLayer,
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Extension, Path, Query, TypedHeader,
    },
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

use super::service;
// use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
pub struct HttpServer {
    shutdown_tx: oneshot::Sender<()>,
}

impl HttpServer {
    pub async fn new(addr: &SocketAddr, mp: SharedStateMap, db: Arc<Influxdb>) -> Self {
        let router = router(mp, db);
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let server = axum::Server::bind(&addr)
            .serve(router.into_make_service())
            .with_graceful_shutdown(async {
                shutdown_rx.await.ok();
            });
        info!("start web server ...");
        tokio::spawn(async move {
            if let Err(e) = server.await {
                println!("server error: {}", e);
            }
        });
        Self { shutdown_tx }
    }

    pub async fn shutdown(self) {
        info!("send http server shutdown signal");
        let _ = self.shutdown_tx.send(());
    }
}

pub fn router(mp: SharedStateMap, db: Arc<Influxdb>) -> Router {
    let app: Router = Router::new()
        .route("/user/info/:address", get(get_user_info))
        .route(
            "/user/positions/:prefix/:address",
            get(get_user_position_list),
        )
        .route("/price/history", get(get_price_history))
        .route("/price/history_c/", get(get_price_history_column))
        .route("/ws", get(ws_handler))
        .layer(
            ServiceBuilder::new()
                // Handle errors from middleware
                .layer(HandleErrorLayer::new(handle_error))
                .load_shed()
                // .concurrency_limit(1024)
                .timeout(Duration::from_secs(3))
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(DefaultMakeSpan::default().include_headers(true)),
                ), // .into_inner(),
        )
        .layer(Extension(mp))
        .layer(Extension(db));
    app
}

async fn get_user_info(
    Path(address): Path<String>,
    Extension(state): Extension<SharedStateMap>,
) -> impl IntoResponse {
    JsonResponse::from(service::get_account_info(address, state)).to_json()
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
    user_agent: Option<TypedHeader<headers::UserAgent>>,
) -> impl IntoResponse {
    if let Some(TypedHeader(user_agent)) = user_agent {
        debug!("`{}` connected", user_agent.as_str());
    }
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    if let Some(msg) = socket.recv().await {
        if let Ok(msg) = msg {
            match msg {
                Message::Text(t) => {
                    println!("client sent str: {:?}", t);
                }
                Message::Binary(_) => {
                    println!("client sent binary data");
                }
                Message::Ping(_) => {
                    println!("socket ping");
                }
                Message::Pong(_) => {
                    println!("socket pong");
                }
                Message::Close(_) => {
                    println!("client disconnected");
                    return;
                }
            }
        } else {
            debug!("client disconnected");
            return;
        }
    }

    loop {
        if socket
            .send(Message::Text(String::from("Hi!")))
            .await
            .is_err()
        {
            println!("client disconnected");
            return;
        }
    }
}
