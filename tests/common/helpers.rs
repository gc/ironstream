use chrono::Utc;
use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};
use tokio::sync::broadcast;

use websocket_server::{
    models::{channel::ChannelData, client::ClientData},
    state::AppState,
};

/// Create a test AppState with predefined values for testing
pub fn create_test_state() -> AppState {
    AppState::new(
        "test_admin_token".to_string(),
        "https://example.com".to_string(),
        10,
        Duration::from_secs(1),
    )
}

/// Create a populated test state with channels and clients
pub async fn create_populated_test_state() -> Arc<AppState> {
    let state = create_test_state();
    let state_arc = Arc::new(state);

    // Add test channel
    let (tx, _) = broadcast::channel(10);
    let channel_data = ChannelData {
        tx,
        message_count: 5,
        last_message: Some(Utc::now()),
    };

    let channel_id = Arc::from("test-channel");
    state_arc
        .channels
        .write()
        .await
        .insert(channel_id, channel_data);

    // Add test client
    let mut channels = HashSet::new();
    channels.insert("test-channel".to_string());
    let client_data = ClientData {
        id: Arc::from("TEST123"),
        ip: "127.0.0.1:8080".parse::<SocketAddr>().unwrap(),
        user_agent: Some("Test Agent".to_string()),
        channels,
        connected_at: Utc::now(),
        metadata: HashMap::new(),
    };

    state_arc
        .clients
        .write()
        .await
        .insert(Arc::from("TEST123"), client_data);

    state_arc
}

/// Helper to create a mock websocket for testing
pub fn create_test_headers() -> http::HeaderMap {
    let mut headers = http::HeaderMap::new();
    headers.insert("user-agent", "Test Client/1.0".parse().unwrap());
    headers.insert("x-test-header", "test-value".parse().unwrap());
    headers
}
