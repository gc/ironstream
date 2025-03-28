use axum::{
    extract::{
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
        ConnectInfo, Path, Query, State,
    },
    http::{HeaderMap, Request, StatusCode},
    middleware::{self, Next},
    response::Response,
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
use tinyrand::RandRange;
use tinyrand_std::thread_rand;
use tokio::sync::{broadcast, RwLock};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(80);
const HEARTBEAT_MESSAGE: &str = "ping";
const API_TIMEOUT: Duration = Duration::from_secs(2);
const CACHE_TTL: Duration = Duration::from_secs(30);

const VALID_CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
pub fn mini_id(length: usize) -> Arc<str> {
    let mut rng = thread_rand();
    let mut id = String::with_capacity(length);
    let char_count = VALID_CHARS.len();

    for _ in 0..length {
        let idx = rng.next_range(0..char_count);
        id.push(VALID_CHARS[idx] as char);
    }

    Arc::from(id)
}

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
    id: Arc<str>,
    ip: SocketAddr,
    user_agent: Option<String>,
    channels: HashSet<String>,
    connected_at: DateTime<Utc>,
    metadata: HashMap<String, String>,
}

#[derive(Deserialize)]
struct AuthResponse {
    ok: bool,
    metadata: HashMap<String, String>,
}

#[derive(Serialize)]
struct ChannelStats {
    channel_id: Arc<str>,
    connections: usize,
    messages: usize,
    last_message: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
struct ClientStats {
    id: Arc<str>,
    ip: String,
    user_agent: Option<String>,
    channels: Vec<String>,
    connected_at: DateTime<Utc>,
    metadata: HashMap<String, String>,
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
    id: Arc<str>,
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

type Channels = Arc<RwLock<HashMap<Arc<str>, ChannelData>>>;
type Clients = Arc<RwLock<HashMap<Arc<str>, ClientData>>>;
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
    query: Result<Query<WsQuery>, axum::extract::rejection::QueryRejection>,
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<axum::response::Response, (StatusCode, Json<Value>)> {
    let query = match query {
        Ok(q) => q.0,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "BAD_REQUEST" })),
            ))
        }
    };
    let state_clone = state.clone();

    // Rate limiting logic
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
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                Json(serde_json::json!({ "error": "TOO_MANY_REQUESTS" })),
            ));
        }
        entry.count += 1;
    }

    let cache_key = format!("{}:{}", query.channel, query.token);
    if let Some(cached_result) = read_cache(&state_clone, &cache_key).await {
        return match cached_result {
            Ok(()) => proceed_with_socket(
                ws,
                query.channel,
                addr,
                headers,
                state_clone.clone(),
                HashMap::new(),
            ),
            Err(err) => {
                let err_json = serde_json::json!({ "error": err }).to_string();
                let ws_response = ws.on_upgrade(move |socket| async move {
                    let mut socket = socket;
                    let _ = socket.send(WsMessage::Text(err_json.into())).await;
                    socket.close().await.ok();
                });
                Ok(ws_response)
            }
        };
    }

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
        Ok(resp) => match resp.json::<AuthResponse>().await {
            Ok(auth_data) => {
                if auth_data.ok {
                    Ok(auth_data.metadata)
                } else {
                    Err((
                        StatusCode::UNAUTHORIZED,
                        Json(serde_json::json!({ "error": "Authentication failed" })),
                    ))
                }
            }
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(
                    serde_json::json!({ "error": format!("Failed to parse auth response: {}", e) }),
                ),
            )),
        },
        Err(e) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": format!("API error: {}", e) })),
        )),
    };

    let cache_result = auth_result
        .clone()
        .map(|_| ())
        .map_err(|e| e.1.get("error").unwrap().as_str().unwrap().to_string());
    write_cache(&state_clone, cache_key.clone(), cache_result).await;

    match auth_result {
        Ok(metadata) => proceed_with_socket(
            ws,
            query.channel,
            addr,
            headers,
            state_clone.clone(),
            metadata,
        ),
        Err(err) => match err.0 {
            StatusCode::UNAUTHORIZED
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::INTERNAL_SERVER_ERROR => {
                let err_json = err.1.to_string();
                let ws_response = ws.on_upgrade(move |socket| async move {
                    let mut socket = socket;
                    let _ = socket.send(WsMessage::Text(err_json.into())).await;
                    socket.close().await.ok();
                });
                Ok(ws_response)
            }
            _ => Err(err),
        },
    }
}
fn proceed_with_socket(
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

async fn handle_socket(
    socket: WebSocket,
    channel: String,
    state: Arc<AppState>,
    ip: SocketAddr,
    user_agent: Option<String>,
    id: Arc<str>,
    metadata: HashMap<String, String>,
) {
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
    state.clients.write().await.insert(id.clone(), client_data);

    let (mut ws_sender, mut ws_receiver) = socket.split();
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
        state_clone.clients.write().await.remove(&id_clone);
    });

    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }
}

async fn stats_handler(State(state): State<Arc<AppState>>) -> (StatusCode, Json<Value>) {
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

async fn webhook(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(channel_id): Path<String>,
    payload: Result<Json<Value>, axum::extract::rejection::JsonRejection>,
) -> (StatusCode, Json<Value>) {
    println!("Received webhook for channel {}", channel_id);
    if headers.get("authorization").and_then(|h| h.to_str().ok()) != Some(&state.admin_token) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "UNAUTHORIZED" })),
        );
    }
    if headers.get("content-type").and_then(|h| h.to_str().ok()) != Some("application/json") {
        return (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Json(serde_json::json!({ "error": "UNSUPPORTED_MEDIA_TYPE" })),
        );
    }
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
    {
        let client_read = state.clients.read().await;
        for (uuid, client) in client_read.iter() {
            if client.channels.contains(&channel_id) {
                sent_to.push(uuid.to_string());
            }
        }
    }
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
                ChannelData {
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

async fn disconnect_user(
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

async fn admin_auth<B>(
    State(state): State<Arc<AppState>>,
    req: Request<B>,
    next: Next,
) -> Result<Response, (StatusCode, Json<Value>)>
where
    B: Send + 'static,
    axum::body::Body: From<B>,
{
    if req
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        == Some(state.admin_token.as_str())
    {
        let req = req.map(|b| b.into());
        return Ok(next.run(req).await);
    }
    Err((
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({ "error": "UNAUTHORIZED" })),
    ))
}

#[tokio::main]
async fn main() {
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

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route(
            "/broadcast/{channel_id}",
            post(webhook).route_layer(middleware::from_fn_with_state(state.clone(), admin_auth)),
        )
        .route(
            "/stats",
            get(stats_handler)
                .route_layer(middleware::from_fn_with_state(state.clone(), admin_auth)),
        )
        .route(
            "/disconnect",
            post(disconnect_user)
                .route_layer(middleware::from_fn_with_state(state.clone(), admin_auth)),
        )
        .fallback(|| async {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "NOT_FOUND" })),
            )
        })
        .with_state(state);

    let url = format!("0.0.0.0:{}", port);

    println!("Listening on {}", &url);
    println!("API Endpoint: {}", api_endpoint);
    println!("Port: {}", port);

    axum::serve(
        tokio::net::TcpListener::bind(&url).await.unwrap(),
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}
