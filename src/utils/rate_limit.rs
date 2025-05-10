use crate::state::AppState;
use axum::{http::StatusCode, Json};
use serde_json::Value;
use std::{net::SocketAddr, time::Instant};

pub async fn check_rate_limit(
    state: &AppState,
    addr: SocketAddr,
) -> Result<(), (StatusCode, Json<Value>)> {
    let mut rate_limits = state.rate_limits.write().await;
    let now = Instant::now();
    let entry = rate_limits
        .entry(addr)
        .or_insert(crate::state::RateLimitEntry {
            count: 0,
            last_reset: now,
        });

    if now.duration_since(entry.last_reset) > state.rate_limit_duration {
        entry.count = 0;
        entry.last_reset = now;
    }

    if entry.count >= state.rate_limit_count {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({ "error": "TOO_MANY_REQUESTS" })),
        ));
    }

    entry.count += 1;
    Ok(())
}
