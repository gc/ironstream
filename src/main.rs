mod api;
mod models;
mod state;
mod utils;
mod websocket;

use axum::{http::StatusCode, routing::get, Json, Router};
use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};
use tokio::sync::RwLock;

use state::AppState;

#[tokio::main]
async fn main() {
    // Load configuration from environment variables
    let admin_token = std::env::var("ADMIN_TOKEN").expect("ADMIN_TOKEN must be set");
    let api_endpoint = std::env::var("API_ENDPOINT").expect("API_ENDPOINT must be set");
    let port = std::env::var("PORT").unwrap_or_else(|_| "3131".into());
    let rate_limit_count = std::env::var("RATE_LIMIT_COUNT")
        .unwrap_or_else(|_| "2".to_string())
        .parse::<u32>()
        .expect("RATE_LIMIT_COUNT must be a positive integer");
    let rate_limit_seconds = std::env::var("RATE_LIMIT_SECONDS")
        .unwrap_or_else(|_| "5".to_string())
        .parse::<u64>()
        .expect("RATE_LIMIT_SECONDS must be a positive integer");

    // Initialize application state
    let state = AppState::new(
        admin_token,
        api_endpoint,
        rate_limit_count,
        Duration::from_secs(rate_limit_seconds),
    );
    let state_arc = Arc::new(state);

    // Build the application router
    let app = Router::new()
        .route("/ws", get(websocket::handler::ws_handler))
        .merge(api::routes::configure_api_routes(state_arc.clone()))
        .fallback(|| async {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "NOT_FOUND" })),
            )
        })
        .with_state(state_arc);

    // Start the server
    let url = format!("0.0.0.0:{}", port);
    // tracing::info!("Listening on {}", &url);
    // tracing::info!("API Endpoint: {}", state_arc.api_endpoint);

    axum::serve(
        tokio::net::TcpListener::bind(&url).await.unwrap(),
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}
