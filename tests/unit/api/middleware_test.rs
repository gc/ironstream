#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        extract::State,
        http::{Request, StatusCode},
        middleware::Next,
        response::Response,
    };
    use http_body::Empty;
    use hyper::body::Bytes;
    use std::{sync::Arc, time::Duration};

    use crate::{api::middleware::admin_auth, state::AppState};

    #[tokio::test]
    async fn test_admin_auth_success() {
        // Create test state with known token
        let admin_token = "test_admin_token";
        let state = AppState::new(
            admin_token.to_string(),
            "https://example.com".to_string(),
            10,
            Duration::from_secs(1),
        );
        let state_arc = Arc::new(state);

        // Create request with correct token
        let mut request = Request::new(Body::empty());
        request
            .headers_mut()
            .insert("authorization", admin_token.parse().unwrap());

        // Mock the next middleware
        let next = |req: Request<Body>| async {
            let body = Empty::new().map_err(|_| panic!());
            Ok::<_, ()>(Response::new(body))
        };

        // Call the middleware
        let result = admin_auth(State(state_arc), request, Next::new(next)).await;

        // Verify the response
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_admin_auth_failure() {
        // Create test state with known token
        let admin_token = "test_admin_token";
        let state = AppState::new(
            admin_token.to_string(),
            "https://example.com".to_string(),
            10,
            Duration::from_secs(1),
        );
        let state_arc = Arc::new(state);

        // Create request with incorrect token
        let mut request = Request::new(Body::empty());
        request
            .headers_mut()
            .insert("authorization", "wrong_token".parse().unwrap());

        // Mock the next middleware (should not be called)
        let next = |req: Request<Body>| async {
            let body = Empty::new().map_err(|_| panic!());
            Ok::<_, ()>(Response::new(body))
        };

        // Call the middleware
        let result = admin_auth(State(state_arc), request, Next::new(next)).await;

        // Verify the response is an error with UNAUTHORIZED status
        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_admin_auth_missing_header() {
        // Create test state
        let state = AppState::new(
            "test_admin_token".to_string(),
            "https://example.com".to_string(),
            10,
            Duration::from_secs(1),
        );
        let state_arc = Arc::new(state);

        // Create request with no authorization header
        let request = Request::new(Body::empty());

        // Mock the next middleware (should not be called)
        let next = |req: Request<Body>| async {
            let body = Empty::new().map_err(|_| panic!());
            Ok::<_, ()>(Response::new(body))
        };

        // Call the middleware
        let result = admin_auth(State(state_arc), request, Next::new(next)).await;

        // Verify the response is an error with UNAUTHORIZED status
        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
}
