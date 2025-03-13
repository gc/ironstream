use axum::{
    extract::{
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
        ConnectInfo, Path, Query, State,
    },
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(80);
const HEARTBEAT_MESSAGE: &str = "ping";
const API_TIMEOUT: Duration = Duration::from_secs(2);
const CACHE_TTL: Duration = Duration::from_secs(30);

#[derive(Clone, Serialize, Deserialize)]
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
    uuid: Uuid,
    ip: SocketAddr,
    user_agent: Option<String>,
    channels: HashSet<String>,
    connected_at: DateTime<Utc>,
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
    uuid: String,
    ip: String,
    user_agent: Option<String>,
    channels: Vec<String>,
    connected_at: DateTime<Utc>,
}

#[derive(Serialize)]
struct Stats {
    channels: Vec<ChannelStats>,
    clients: Vec<ClientStats>,
}

#[derive(Deserialize)]
struct WsQuery {
    channel: String,
    token: String,
}

#[derive(Deserialize)]
struct DisconnectPayload {
    uuid: String,
    channel: String,
}

#[derive(Clone)]
struct CacheEntry {
    result: Result<(), String>,
    timestamp: Instant,
}

#[derive(Clone)]
struct RateLimitEntry {
    count: u32,
    last_reset: Instant,
}

type Channels = Arc<RwLock<HashMap<String, ChannelData>>>;
type Clients = Arc<RwLock<HashMap<Uuid, ClientData>>>;
type Cache = Arc<RwLock<HashMap<String, CacheEntry>>>;
type RateLimits = Arc<RwLock<HashMap<SocketAddr, RateLimitEntry>>>;

struct AppState {
    channels: Channels,
    clients: Clients,
    cache: Cache,
    rate_limits: RateLimits,
    http_client: Client,
    admin_token: String,
    api_endpoint: String,
    rate_limit_count: u32,
    rate_limit_duration: Duration,
}

// New function to read from cache
async fn read_cache(state: &AppState, cache_key: &str) -> Option<Result<(), String>> {
    let cache_read = state.cache.read().await;
    cache_read.get(cache_key).and_then(|entry| {
        if Instant::now().duration_since(entry.timestamp) < CACHE_TTL {
            Some(entry.result.clone())
        } else {
            None
        }
    })
}

// New function to write to cache
async fn write_cache(state: &AppState, cache_key: String, result: Result<(), String>) {
    let mut cache_write = state.cache.write().await;
    cache_write.insert(
        cache_key,
        CacheEntry {
            result,
            timestamp: Instant::now(),
        },
    );
}

async fn ws_handler(
    Query(query): Query<WsQuery>,
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<axum::response::Response, StatusCode> {
    let state_clone = state.clone(); // Clone state for rate limiting and API calls

    // Simple rate limiting
    {
        let mut rate_limits = state_clone.rate_limits.write().await;
        let now = Instant::now();
        let entry = rate_limits.entry(addr).or_insert(RateLimitEntry {
            count: 0,
            last_reset: now,
        });
        if now.duration_since(entry.last_reset) > state_clone.rate_limit_duration {
            entry.count = 0;
            entry.last_reset = now;
        }
        if entry.count >= state_clone.rate_limit_count {
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
        entry.count += 1;
    }

    // Check cache
    let cache_key = format!("{}:{}", query.channel, query.token);
    if let Some(cached_result) = read_cache(&state_clone, &cache_key).await {
        return match cached_result {
            Ok(()) => proceed_with_socket(ws, query.channel, addr, headers, state_clone.clone()), // Clone again for socket
            Err(err) => {
                let err_json = serde_json::json!({ "error": err }).to_string();
                let ws_response = ws.on_upgrade(move |socket| async move {
                    let mut socket = socket;
                    let _ = socket.send(WsMessage::Text(err_json)).await;
                    socket.close().await.ok();
                });
                Ok(ws_response)
            }
        };
    }

    // API authentication
    let headers_json: HashMap<String, String> = headers
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|v| (k.to_string(), v.to_string())))
        .collect();
    let auth_response = state_clone
        .http_client
        .post(&state_clone.api_endpoint)
        .json(&serde_json::json!({
            "channel": &query.channel,
            "token": &query.token,
            "ip": addr.to_string(),
            "headers": headers_json,
        }))
        .timeout(API_TIMEOUT)
        .send()
        .await;

    let auth_result = match auth_response {
        Ok(resp) if resp.status().is_success() => Ok(()),
        Ok(resp) => Err(resp
            .text()
            .await
            .unwrap_or_else(|_| "Authentication failed".to_string())),
        // Err(_e) => Err("API error".to_string()),
        Err(e) => Err(format!("API error: {}", e)),
    };

    // Write to cache
    write_cache(&state_clone, cache_key.clone(), auth_result.clone()).await;

    match auth_result {
        Ok(()) => proceed_with_socket(ws, query.channel, addr, headers, state_clone.clone()), // Clone again for socket
        Err(err) => {
            let err_json = serde_json::json!({ "error": err }).to_string();
            let ws_response = ws.on_upgrade(move |socket| async move {
                let mut socket = socket;
                let _ = socket.send(WsMessage::Text(err_json)).await;
                socket.close().await.ok();
            });
            Ok(ws_response)
        }
    }
}

fn proceed_with_socket(
    ws: WebSocketUpgrade,
    channel: String,
    addr: SocketAddr,
    headers: HeaderMap,
    state: Arc<AppState>,
) -> Result<axum::response::Response, StatusCode> {
    let user_agent = headers
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .map(String::from);
    let uuid = Uuid::new_v4();
    Ok(ws.on_upgrade(move |socket| handle_socket(socket, channel, state, addr, user_agent, uuid)))
}

async fn handle_socket(
    socket: WebSocket,
    channel: String,
    state: Arc<AppState>,
    ip: SocketAddr,
    user_agent: Option<String>,
    uuid: Uuid,
) {
    let (_tx, rx) = {
        let mut channel_write = state.channels.write().await;
        let entry = channel_write.entry(channel.clone()).or_insert_with(|| {
            let (tx, _) = broadcast::channel(10);
            ChannelData {
                tx,
                message_count: 0,
                last_message: None,
            }
        });
        (entry.tx.clone(), entry.tx.subscribe())
    };

    let mut channels_set = HashSet::new();
    channels_set.insert(channel.clone());
    let client_data = ClientData {
        uuid,
        ip,
        user_agent,
        channels: channels_set.clone(),
        connected_at: Utc::now(),
    };
    state.clients.write().await.insert(uuid, client_data);

    let (mut ws_sender, mut ws_receiver) = socket.split();
    let mut send_task = tokio::spawn(async move {
        let mut rx = rx;
        let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);
        loop {
            tokio::select! {
                msg = rx.recv() => {
                    match msg {
                        Ok(msg) => {
                            if ws_sender.send(WsMessage::Text(msg)).await.is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                _ = heartbeat.tick() => {
                    if ws_sender.send(WsMessage::Text(HEARTBEAT_MESSAGE.to_string())).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    let state_clone = state.clone();
    let uuid_clone = uuid;
    let mut recv_task = tokio::spawn(async move {
        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(WsMessage::Close(_)) => break,
                Ok(_) => continue,
                Err(_) => break,
            }
        }
        state_clone.clients.write().await.remove(&uuid_clone);
    });

    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    };
}

async fn stats_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Stats>, StatusCode> {
    if headers.get("authorization").and_then(|h| h.to_str().ok()) != Some(&state.admin_token) {
        return Err(StatusCode::UNAUTHORIZED);
    }

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
            uuid: client.uuid.to_string(),
            ip: client.ip.to_string(),
            user_agent: client.user_agent.clone(),
            channels: client.channels.iter().cloned().collect(),
            connected_at: client.connected_at,
        })
        .collect();

    Ok(Json(Stats { channels, clients }))
}

async fn webhook(
    State(state): State<Arc<AppState>>,
    Path(channel_id): Path<String>,
    Json(payload): Json<Value>,
) -> Json<Value> {
    let mut channel_write = state.channels.write().await;
    let message = serde_json::to_string(&payload).unwrap_or_default();
    let mut sent_to = Vec::new();

    {
        let client_read = state.clients.read().await;
        for (uuid, client) in client_read.iter() {
            if client.channels.contains(&channel_id) {
                sent_to.push(uuid.to_string());
            }
        }
    }

    match channel_write.get_mut(&channel_id) {
        Some(data) => {
            data.message_count += 1;
            data.last_message = Some(Utc::now());
            let _ = data.tx.send(message);
        }
        None => {
            let (new_tx, _) = broadcast::channel(10);
            channel_write.insert(
                channel_id.clone(),
                ChannelData {
                    tx: new_tx.clone(),
                    message_count: 1,
                    last_message: Some(Utc::now()),
                },
            );
            let _ = new_tx.send(message);
        }
    }

    Json(serde_json::json!({ "sent_to": sent_to }))
}

async fn disconnect_user(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<DisconnectPayload>,
) -> Result<StatusCode, StatusCode> {
    if headers.get("authorization").and_then(|h| h.to_str().ok()) != Some(&state.admin_token) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let mut clients = state.clients.write().await;
    let uuid = Uuid::parse_str(&payload.uuid).map_err(|_| StatusCode::BAD_REQUEST)?;
    match clients.get_mut(&uuid) {
        Some(client) => {
            if client.channels.contains(&payload.channel) {
                client.channels.remove(&payload.channel);
                if client.channels.is_empty() {
                    clients.remove(&uuid);
                }
                Ok(StatusCode::OK)
            } else {
                Err(StatusCode::NOT_FOUND)
            }
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

#[tokio::main]
async fn main() {
    println!("Starting ironstream");
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

    println!("Finished reading env vars");
    let channels: Channels = Arc::new(RwLock::new(HashMap::new()));
    let clients: Clients = Arc::new(RwLock::new(HashMap::new()));
    let cache: Cache = Arc::new(RwLock::new(HashMap::new()));
    let rate_limits: RateLimits = Arc::new(RwLock::new(HashMap::new()));

    let state = Arc::new(AppState {
        channels,
        clients,
        cache,
        rate_limits,
        http_client: Client::new(),
        admin_token: admin_token.clone(),
        api_endpoint: api_endpoint.clone(),
        rate_limit_count,
        rate_limit_duration: Duration::from_secs(rate_limit_seconds),
    });

    println!("Starting router");
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/broadcast/:channel_id", post(webhook))
        .route("/admin/stats", get(stats_handler))
        .route("/admin/disconnect", post(disconnect_user))
        .with_state(state);

    let url = format!("0.0.0.0:{}", port);
    let admin_token_tail = admin_token
        .chars()
        .rev()
        .take(3)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    println!("Listening on {}", url);
    println!("API Endpoint: {}", api_endpoint);
    println!("Port: {}", port);
    println!("Admin Token (last 3 chars): {}", admin_token_tail);

    println!("Axum serve");
    axum::serve(
        tokio::net::TcpListener::bind(url).await.unwrap(),
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();

    println!("Finished");
}
