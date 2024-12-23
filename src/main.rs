use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashSet, sync::Arc, time::Duration};
use tokio::sync::{broadcast, RwLock};

const CHANNEL_CLEANUP_INTERVAL: Duration = Duration::from_secs(900);
const CHANNEL_INACTIVE_THRESHOLD: Duration = Duration::from_secs(600);
const CHANNEL_CAPACITY: usize = 10;
const MAX_CHANNELS_PER_CONNECTION: usize = 10;

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

#[derive(Serialize)]
struct ChannelStats {
    channel_id: String,
    connections: usize,
    messages: usize,
    last_message: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
struct Stats {
    channels: Vec<ChannelStats>,
}

#[derive(Deserialize)]
struct WsQuery {
    channels: String,
}

type Channels = Arc<RwLock<Vec<(String, ChannelData)>>>;

async fn ws_handler(
    Query(query): Query<WsQuery>,
    ws: axum::extract::ws::WebSocketUpgrade,
    State((channel_state, _)): State<(Channels, String)>,
) -> Result<axum::response::Response, StatusCode> {
    let channels: HashSet<String> = query.channels.split(',').map(String::from).collect();
    if channels.len() > MAX_CHANNELS_PER_CONNECTION {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(ws.on_upgrade(move |socket| handle_socket(socket, channels, channel_state)))
}

async fn stats_handler(State((channel_state, _)): State<(Channels, String)>) -> Json<Stats> {
    let channel_read = channel_state.read().await;
    let channels = channel_read
        .iter()
        .map(|(id, data)| ChannelStats {
            channel_id: id.clone(),
            connections: data.tx.receiver_count(),
            messages: data.message_count,
            last_message: data.last_message,
        })
        .collect();

    Json(Stats { channels })
}

async fn handle_socket(
    socket: axum::extract::ws::WebSocket,
    channel_ids: HashSet<String>,
    channel_state: Channels,
) {
    let mut receivers = Vec::new();

    for channel_id in channel_ids {
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

    let mut send_task = tokio::spawn(async move {
        let mut combined_rx =
            futures_util::stream::select_all(receivers.iter_mut().map(|(_, _, rx)| {
                Box::pin(futures_util::stream::unfold(rx, |rx| async move {
                    rx.recv().await.ok().map(|msg| (msg, rx))
                }))
            }));

        while let Some(msg) = combined_rx.next().await {
            if ws_sender
                .send(axum::extract::ws::Message::Text(msg))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            if matches!(msg, axum::extract::ws::Message::Close(_)) {
                break;
            }
        }
    });

    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }
}

async fn webhook(
    State((channel_state, auth_token)): State<(Channels, String)>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(channel_id): axum::extract::Path<String>,
    Json(payload): Json<Value>,
) -> StatusCode {
    if headers.get("authorization").and_then(|h| h.to_str().ok()) != Some(&auth_token) {
        return StatusCode::UNAUTHORIZED;
    }

    let message = serde_json::to_string(&payload).unwrap_or_default();

    let mut channel_read = channel_state.write().await;
    match channel_read.iter_mut().find(|(r, _)| r == &channel_id) {
        Some((_, data)) => {
            data.message_count += 1;
            data.last_message = Some(Utc::now());
            let _ = data.tx.send(message);
            StatusCode::OK
        }
        None => {
            let (new_tx, _) = broadcast::channel(CHANNEL_CAPACITY);
            channel_read.push((
                channel_id,
                ChannelData {
                    tx: new_tx,
                    message_count: 1,
                    last_message: Some(Utc::now()),
                },
            ));
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
    let channel_state: Channels = Arc::new(RwLock::new(Vec::new()));
    let cleanup_state = channel_state.clone();
    let auth_token = std::env::var("AUTH_TOKEN").expect("AUTH_TOKEN env variable must be set");
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
        .with_state((channel_state, auth_token.clone()));

    let url = format!("0.0.0.0:{}", port);
    println!("Listening on {}", url);

    axum::serve(
        tokio::net::TcpListener::bind(url).await.unwrap(),
        app.into_make_service(),
    )
    .await
    .unwrap();
}
