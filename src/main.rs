use axum::{
    extract::{ws::WebSocketUpgrade, ConnectInfo, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::sync::{broadcast, RwLock};
use toml;

const CHANNEL_CLEANUP_INTERVAL: Duration = Duration::from_secs(900);
const CHANNEL_INACTIVE_THRESHOLD: Duration = Duration::from_secs(600);
const CHANNEL_CAPACITY: usize = 10;
const MAX_CHANNELS_PER_CONNECTION: usize = 10;
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(80);
const HEARTBEAT_MESSAGE: &str = "ping";

#[derive(Clone, Deserialize, Serialize)]
struct Message {
    channel: String,
    data: String,
}

#[derive(Clone)]
struct ChannelData {
    tx: broadcast::Sender<String>,
    message_count: usize,
    last_message: Option<DateTime<Utc>>,
}

#[derive(Clone)]
struct ClientData {
    ip: SocketAddr,
    user_agent: Option<String>,
    channels: HashSet<String>,
}

#[derive(Serialize)]
struct ChannelStats {
    channel_id: String,
    connections: usize,
    messages: usize,
    last_message: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
struct ClientStats {
    ip: String,
    user_agent: Option<String>,
    channels: Vec<String>,
}

#[derive(Serialize)]
struct Stats {
    channels: Vec<ChannelStats>,
    clients: Vec<ClientStats>,
}

#[derive(Deserialize)]
struct WsQuery {
    channels: String,
}

#[derive(Clone, Deserialize)]
struct Config {
    admin_token: String,
    namespaces: HashMap<String, String>, // e.g., "dev.*" -> "token123"
    fallback_token: String,
}

type Channels = Arc<RwLock<Vec<(String, ChannelData)>>>;
type Clients = Arc<RwLock<Vec<ClientData>>>;

async fn ws_handler(
    Query(query): Query<WsQuery>,
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<(Channels, Clients, Config)>,
    headers: HeaderMap,
) -> Result<axum::response::Response, StatusCode> {
    let (channel_state, client_state, _) = state;
    let channels: HashSet<String> = query.channels.split(',').map(String::from).collect();
    if channels.len() > MAX_CHANNELS_PER_CONNECTION {
        return Err(StatusCode::BAD_REQUEST);
    }

    let user_agent = headers
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .map(String::from);

    Ok(ws.on_upgrade(move |socket| {
        handle_socket(
            socket,
            channels,
            channel_state,
            client_state,
            addr,
            user_agent,
        )
    }))
}

async fn stats_handler(
    State(state): State<(Channels, Clients, Config)>,
    headers: HeaderMap,
) -> Result<Json<Stats>, StatusCode> {
    let (channel_state, client_state, config) = state;
    if headers.get("authorization").and_then(|h| h.to_str().ok()) != Some(&config.admin_token) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let channel_read = channel_state.read().await;
    let client_read = client_state.read().await;

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
        .map(|client| ClientStats {
            ip: client.ip.to_string(),
            user_agent: client.user_agent.clone(),
            channels: client.channels.iter().cloned().collect(),
        })
        .collect();

    Ok(Json(Stats { channels, clients }))
}

async fn handle_socket(
    socket: axum::extract::ws::WebSocket,
    channel_ids: HashSet<String>,
    channel_state: Channels,
    client_state: Clients,
    ip: SocketAddr,
    user_agent: Option<String>,
) {
    let mut receivers = Vec::new();

    for channel_id in channel_ids.clone() {
        let (tx, rx) = {
            let channel_read = channel_state.read().await;
            match channel_read.iter().find(|(r, _)| r == &channel_id) {
                Some((_, data)) => (data.tx.clone(), data.tx.subscribe()),
                None => {
                    drop(channel_read);
                    let mut channel_write = channel_state.write().await;
                    let (new_tx, rx) = broadcast::channel(CHANNEL_CAPACITY);
                    channel_write.push((
                        channel_id.clone(),
                        ChannelData {
                            tx: new_tx.clone(),
                            message_count: 0,
                            last_message: None,
                        },
                    ));
                    (new_tx, rx)
                }
            }
        };
        receivers.push((channel_id, tx, rx));
    }

    let (mut ws_sender, mut ws_receiver) = socket.split();
    client_state.write().await.push(ClientData {
        ip,
        user_agent,
        channels: channel_ids.clone(),
    });

    let mut send_task = tokio::spawn(async move {
        let mut combined_rx =
            futures_util::stream::select_all(receivers.iter_mut().map(|(_, _, rx)| {
                Box::pin(futures_util::stream::unfold(rx, |rx| async move {
                    rx.recv().await.ok().map(|msg| (msg, rx))
                }))
            }));

        let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);
        loop {
            tokio::select! {
                Some(msg) = combined_rx.next() => {
                    if ws_sender.send(axum::extract::ws::Message::Text(msg)).await.is_err() {
                        break;
                    }
                }
                _ = heartbeat.tick() => {
                    if ws_sender.send(axum::extract::ws::Message::Text(HEARTBEAT_MESSAGE.to_string())).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    let client_state_clone = client_state.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            if matches!(msg, axum::extract::ws::Message::Close(_)) {
                break;
            }
        }
        client_state_clone
            .write()
            .await
            .retain(|client| client.ip != ip || !client.channels.is_subset(&channel_ids));
    });

    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }
}

async fn webhook(
    State(state): State<(Channels, Clients, Config)>,
    headers: HeaderMap,
    axum::extract::Path(channel_id): axum::extract::Path<String>,
    Json(payload): Json<Value>,
) -> StatusCode {
    let (channel_state, _, config) = state;
    let auth_token = headers.get("authorization").and_then(|h| h.to_str().ok());

    let valid_token = config
        .namespaces
        .iter()
        .find(|(ns, _)| ns.ends_with('*') && channel_id.starts_with(&ns[..ns.len() - 1]))
        .map(|(_, token)| token.as_str())
        .unwrap_or(&config.fallback_token);

    if auth_token != Some(valid_token) {
        return StatusCode::UNAUTHORIZED;
    }

    let message = serde_json::to_string(&payload).unwrap_or_default();
    let mut channel_write = channel_state.write().await;
    match channel_write.iter_mut().find(|(r, _)| r == &channel_id) {
        Some((_, data)) => {
            data.message_count += 1;
            data.last_message = Some(Utc::now());
            let _ = data.tx.send(message);
            StatusCode::OK
        }
        None => {
            let (new_tx, _) = broadcast::channel(CHANNEL_CAPACITY);
            channel_write.push((
                channel_id,
                ChannelData {
                    tx: new_tx.clone(),
                    message_count: 1,
                    last_message: Some(Utc::now()),
                },
            ));
            let _ = new_tx.send(message);
            StatusCode::CREATED
        }
    }
}

async fn cleanup_channels(channel_state: Channels) {
    let now = Utc::now();
    let mut channel_write = channel_state.write().await;
    channel_write.retain(|(_, data)| {
        let has_connections = data.tx.receiver_count() > 0;
        let is_recently_active = data
            .last_message
            .map(|last| {
                now.signed_duration_since(last).num_seconds()
                    < CHANNEL_INACTIVE_THRESHOLD.as_secs() as i64
            })
            .unwrap_or(false);
        has_connections || is_recently_active
    });
}

#[tokio::main]
async fn main() {
    let config_content =
        std::fs::read_to_string("config.toml").expect("Failed to read config.toml");
    let config: Config = toml::from_str(&config_content).expect("Failed to parse TOML config");

    let channel_state: Channels = Arc::new(RwLock::new(Vec::new()));
    let client_state: Clients = Arc::new(RwLock::new(Vec::new()));
    let cleanup_state = channel_state.clone();

    let port = std::env::var("PORT").unwrap_or_else(|_| "3131".into());

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(CHANNEL_CLEANUP_INTERVAL);
        loop {
            interval.tick().await;
            cleanup_channels(cleanup_state.clone()).await;
        }
    });

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/webhook/:channel_id", post(webhook))
        .route("/stats", get(stats_handler))
        .with_state((channel_state, client_state, config));

    let url = format!("0.0.0.0:{}", port);
    println!("Listening on {}", url);

    axum::serve(
        tokio::net::TcpListener::bind(url).await.unwrap(),
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}
