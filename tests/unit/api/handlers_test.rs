#[cfg(test)]
mod tests {
    use axum::{extract::State, http::StatusCode};
    use chrono::Utc;
    use std::{
        collections::{HashMap, HashSet},
        net::SocketAddr,
        sync::Arc,
        time::Duration,
    };
    use tokio::sync::broadcast;

    use crate::{
        api::handlers::{disconnect_user, stats_handler},
        models::{
            channel::ChannelData,
            client::{ClientData, DisconnectPayload},
        },
        state::AppState,
    };

    #[tokio::test]
    async fn test_stats_handler() {
        // Create test state
        let state = AppState::new(
            "test_token".to_string(),
            "https://example.com".to_string(),
            10,
            Duration::from_secs(1),
        );
        let state_arc = Arc::new(state);

        // Add a test channel
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

        // Add a test client
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

        // Call the handler
        let (status, json) = stats_handler(State(state_arc)).await;

        // Verify the response
        assert_eq!(status, StatusCode::OK);

        // Parse and verify JSON response
        let json_obj = json.0.as_object().unwrap();
        assert!(json_obj.contains_key("channels"));
        assert!(json_obj.contains_key("clients"));

        let channels = json_obj.get("channels").unwrap().as_array().unwrap();
        assert_eq!(channels.len(), 1);

        let clients = json_obj.get("clients").unwrap().as_array().unwrap();
        assert_eq!(clients.len(), 1);
    }

    #[tokio::test]
    async fn test_disconnect_user_success() {
        // Create test state
        let state = AppState::new(
            "test_token".to_string(),
            "https://example.com".to_string(),
            10,
            Duration::from_secs(1),
        );
        let state_arc = Arc::new(state);

        // Add a test client
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

        // Create disconnect payload
        let payload = DisconnectPayload {
            id: Arc::from("TEST123"),
            channel: "test-channel".to_string(),
        };

        // Call the handler
        let result = disconnect_user(State(state_arc.clone()), axum::Json(payload)).await;

        // Verify the response
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), StatusCode::OK);

        // Verify client was removed
        assert_eq!(state_arc.clients.read().await.len(), 0);
    }

    #[tokio::test]
    async fn test_disconnect_user_not_found() {
        // Create test state
        let state = AppState::new(
            "test_token".to_string(),
            "https://example.com".to_string(),
            10,
            Duration::from_secs(1),
        );
        let state_arc = Arc::new(state);

        // Create disconnect payload for non-existent client
        let payload = DisconnectPayload {
            id: Arc::from("NONEXIST"),
            channel: "test-channel".to_string(),
        };

        // Call the handler
        let result = disconnect_user(State(state_arc), axum::Json(payload)).await;

        // Verify the response
        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}
