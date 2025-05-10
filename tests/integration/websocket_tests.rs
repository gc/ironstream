use axum::{
    body::Body,
    extract::ws::{Message, WebSocket},
    http::{Request, StatusCode},
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use mockito::{mock, server_url};
use std::{sync::Arc, time::Duration};
use tower::ServiceExt;

use websocket_server::{state::AppState, websocket::handler::ws_handler};

// This is a more comprehensive test that would require setting up
// an actual WebSocket connection. In a real project, you might use
// a library like tokio-tungstenite to create a WebSocket client.
#[tokio::test]
async fn test_websocket_route() {
    // Setup mock server for authentication
    let mock_auth = mock("POST", "/")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"ok":true,"metadata":{}}"#)
        .expect(1)
        .create();

    // Create test state with mock server URL
    let state = AppState::new(
        "test_token".to_string(),
        server_url(),
        10,
        Duration::from_secs(1),
    );
    let state_arc = Arc::new(state);

    // Create router with WebSocket handler
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state_arc);

    // In a real test, you would make a WebSocket connection here
    // For this example, we'll just check that the route responds correctly to a GET request
    let response = app
        .oneshot(
            Request::builder()
                .uri("/ws?channel=test-channel&token=valid-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Verify the response is a switching protocols response
    assert_eq!(response.status(), StatusCode::SWITCHING_PROTOCOLS);

    // Verify the mock was called
    mock_auth.assert();
}
