use axum::{
    Json,
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use serde_json::Value;
use std::sync::Arc;

use crate::state::AppState;

pub async fn admin_auth(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, (StatusCode, Json<Value>)> {
    if req
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        == Some(state.admin_token.as_str())
    {
        Ok(next.run(req).await)
    } else {
        Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "UNAUTHORIZED" })),
        ))
    }
}
