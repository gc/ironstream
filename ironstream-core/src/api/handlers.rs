use axum::{Json, extract::State, http::StatusCode};
use serde_json::Value;
use std::sync::Arc;

use crate::{
    models::{
        channel::ChannelStats,
        client::{ClientStats, DisconnectPayload},
    },
    state::AppState,
};

#[derive(serde::Serialize)]
struct Stats {
    channels: Vec<ChannelStats>,
    clients: Vec<ClientStats>,
}

pub async fn stats_handler(State(state): State<Arc<AppState>>) -> (StatusCode, Json<Value>) {
    let channel_read = state.channels.read().await;
    let client_read = state.clients.read().await;

    let channels = channel_read
        .iter()
        .map(|(id, data)| ChannelStats {
            channel_id: id.clone(),
            connections: data.tx.receiver_count(),
            messages: data.message_count,
            last_message: data.last_message,
        })
        .collect();

    let clients = client_read
        .iter()
        .map(|(_, client)| ClientStats {
            id: client.id.clone(),
            ip: client.ip.to_string(),
            user_agent: client.user_agent.clone(),
            channels: client.channels.iter().cloned().collect(),
            connected_at: client.connected_at,
            metadata: client.metadata.clone(),
        })
        .collect();

    (
        StatusCode::OK,
        Json(serde_json::json!(Stats { channels, clients })),
    )
}

pub async fn disconnect_user(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<DisconnectPayload>,
) -> Result<StatusCode, (StatusCode, Json<Value>)> {
    let mut clients = state.clients.write().await;
    match clients.get_mut(&payload.id) {
        Some(client) => {
            if client.channels.contains(&payload.channel) {
                client.channels.remove(&payload.channel);
                if client.channels.is_empty() {
                    clients.remove(&payload.id);
                }
                Ok(StatusCode::OK)
            } else {
                Err((
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({ "error": "NOT_FOUND" })),
                ))
            }
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "NOT_FOUND" })),
        )),
    }
}
