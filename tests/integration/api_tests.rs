use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use std::{sync::Arc, time::Duration};
use tower::ServiceExt;

use websocket_server::{api::routes::configure_api_routes, state::AppState};

#[tokio::test]
async fn test_api_routes_unauthorized() {
    // Create test state
    let state = AppState::new(
        "test_admin_token".to_string(),
        "https://example.com".to_string(),
        10,
        Duration::from_secs(1),
    );
    let state_arc = Arc::new(state);

    // Create router with API routes
    let app = configure_api_routes(state_arc);

    // Test /stats endpoint without authorization
    let response = app
        .oneshot(
            Request::builder()
                .uri("/stats")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // Test /disconnect endpoint without authorization
    let response = app
        .oneshot(
            Request::builder()
                .uri("/disconnect")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"id":"TEST123","channel":"test-channel"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_api_routes_authorized() {
    // Create test state
    let state = AppState::new(
        "test_admin_token".to_string(),
        "https://example.com".to_string(),
        10,
        Duration::from_secs(1),
    );
    let state_arc = Arc::new(state);

    // Create router with API routes
    let app = configure_api_routes(state_arc);

    // Test /stats endpoint with authorization
    let response = app
        .oneshot(
            Request::builder()
                .uri("/stats")
                .header("authorization", "test_admin_token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
