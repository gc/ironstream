use axum::{
    extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};
use tokio::sync::broadcast;

use crate::{
    models::{channel::ChannelData, client::ClientData},
    state::AppState,
    utils::id_generator::mini_id,
};

// Constants
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(80);
const HEARTBEAT_MESSAGE: &str = "ping";

pub fn proceed_with_socket(
    ws: WebSocketUpgrade,
    channel: String,
    addr: SocketAddr,
    headers: HeaderMap,
    state: Arc<AppState>,
    metadata: HashMap<String, String>,
) -> Result<axum::response::Response, (StatusCode, Json<Value>)> {
    let user_agent = headers
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .map(String::from);
    let id = mini_id(8);

    Ok(ws.on_upgrade(move |socket| {
        handle_socket(socket, channel, state, addr, user_agent, id, metadata)
    }))
}

pub async fn handle_socket(
    socket: WebSocket,
    channel: String,
    state: Arc<AppState>,
    ip: SocketAddr,
    user_agent: Option<String>,
    id: Arc<str>,
    metadata: HashMap<String, String>,
) {
    // Get or create channel
    let (_tx, rx) = {
        let mut channel_write = state.channels.write().await;
        let channel_key: Arc<str> = Arc::from(channel.as_str());
        let entry = channel_write.entry(channel_key.clone()).or_insert_with(|| {
            let (tx, _) = broadcast::channel(10);
            ChannelData {
                tx,
                message_count: 0,
                last_message: None,
            }
        });
        (entry.tx.clone(), entry.tx.subscribe())
    };

    // Create client data
    let mut channels_set = HashSet::new();
    channels_set.insert(channel.clone());
    let client_data = ClientData {
        id: id.clone(),
        ip,
        user_agent,
        channels: channels_set,
        connected_at: Utc::now(),
        metadata,
    };

    // Add client to clients map
    state.clients.write().await.insert(id.clone(), client_data);

    // Split socket into sender and receiver
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Spawn send task
    let mut send_task = tokio::spawn(async move {
        let mut rx = rx;
        let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);

        loop {
            tokio::select! {
                msg = rx.recv() => {
                    if let Ok(msg) = msg {
                        if ws_sender.send(WsMessage::Text(msg.into())).await.is_err() {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                _ = heartbeat.tick() => {
                    if ws_sender.send(WsMessage::Text(HEARTBEAT_MESSAGE.into())).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    // Spawn receive task
    let state_clone = state.clone();
    let id_clone = id.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(WsMessage::Close(_)) => break,
                Ok(_) => continue,
                Err(_) => break,
            }
        }

        // Remove client on disconnect
        state_clone.clients.write().await.remove(&id_clone);
    });

    // Wait for either task to complete
    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }
}
