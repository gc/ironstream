#[cfg(test)]
mod tests {
    use axum::{
        extract::{ws::WebSocketUpgrade, ConnectInfo, Query},
        http::{HeaderMap, StatusCode},
    };
    use mockito::{mock, server_url};
    use std::{net::SocketAddr, sync::Arc, time::Duration};

    use crate::{models::auth::WsQuery, state::AppState, websocket::handler::ws_handler};

    #[tokio::test]
    async fn test_ws_handler_bad_query() {
        // Create test state
        let state = AppState::new(
            "test_token".to_string(),
            "https://example.com".to_string(),
            10,
            Duration::from_secs(1),
        );
        let state_arc = Arc::new(state);

        // Call handler with a bad query
        let bad_query: Result<Query<WsQuery>, _> =
            Err(axum::extract::rejection::QueryRejection::MissingQueryString);

        let ws = WebSocketUpgrade::default();
        let addr = "127.0.0.1:8080".parse::<SocketAddr>().unwrap();
        let headers = HeaderMap::new();

        let result = ws_handler(
            bad_query,
            ws,
            ConnectInfo(addr),
            axum::extract::State(state_arc),
            headers,
        )
        .await;

        // Verify the response is an error with BAD_REQUEST status
        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_ws_handler_auth_failure() {
        // Setup mock server for authentication
        let mock_auth = mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"ok":false,"metadata":{}}"#)
            .create();

        // Create test state with mock server URL
        let state = AppState::new(
            "test_token".to_string(),
            server_url(),
            10,
            Duration::from_secs(1),
        );
        let state_arc = Arc::new(state);

        // Create a valid query
        let query_data = WsQuery {
            channel: "test-channel".to_string(),
            token: "invalid-token".to_string(),
        };
        let query = Ok(Query(query_data));

        let ws = WebSocketUpgrade::default();
        let addr = "127.0.0.1:8080".parse::<SocketAddr>().unwrap();
        let headers = HeaderMap::new();

        let result = ws_handler(
            query,
            ws,
            ConnectInfo(addr),
            axum::extract::State(state_arc),
            headers,
        )
        .await;

        // Verify the mock was called
        mock_auth.assert();

        // Verify the response is an error with UNAUTHORIZED status
        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
}
