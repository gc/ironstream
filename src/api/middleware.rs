use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
    Json,
};
use serde_json::Value;
use std::sync::Arc;

use crate::state::AppState;

pub async fn admin_auth<B>(
    State(state): State<Arc<AppState>>,
    req: Request<B>,
    next: Next<B>,
) -> Result<Response, (StatusCode, Json<Value>)>
where
    B: Send + 'static,
{
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
