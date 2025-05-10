use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::Utc;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::state::AppState;

pub async fn webhook_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(channel_id): Path<String>,
    payload: Result<Json<Value>, axum::extract::rejection::JsonRejection>,
) -> (StatusCode, Json<Value>) {
    // tracing::info!("Received webhook for channel {}", channel_id);

    // Check authorization
    if headers.get("authorization").and_then(|h| h.to_str().ok()) != Some(&state.admin_token) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "UNAUTHORIZED" })),
        );
    }

    // Check content type
    if headers.get("content-type").and_then(|h| h.to_str().ok()) != Some("application/json") {
        return (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Json(serde_json::json!({ "error": "UNSUPPORTED_MEDIA_TYPE" })),
        );
    }

    // Check payload
    if payload.is_err() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "BAD_REQUEST" })),
        );
    }

    let Json(payload) = payload.unwrap();
    let mut channel_write = state.channels.write().await;
    let message = serde_json::to_string(&payload).unwrap_or_default();
    let mut sent_to = Vec::new();

    // Collect client IDs that are in this channel
    {
        let client_read = state.clients.read().await;
        for (uuid, client) in client_read.iter() {
            if client.channels.contains(&channel_id) {
                sent_to.push(uuid.to_string());
            }
        }
    }

    // Get or create the channel
    let channel_key = Arc::from(channel_id.as_str());
    match channel_write.get_mut(&channel_key) {
        Some(data) => {
            data.message_count += 1;
            data.last_message = Some(Utc::now());
            let _ = data.tx.send(message);
        }
        None => {
            let (new_tx, _) = broadcast::channel(10);
            channel_write.insert(
                channel_key,
                crate::models::channel::ChannelData {
                    tx: new_tx.clone(),
                    message_count: 1,
                    last_message: Some(Utc::now()),
                },
            );
            let _ = new_tx.send(message);
        }
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({ "sent_to": sent_to })),
    )
}
