use axum::{
    Json,
    extract::{ConnectInfo, Query, State, ws::WebSocketUpgrade},
    http::{HeaderMap, StatusCode},
};
use serde_json::Value;
use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use crate::{
    models::auth::WsQuery, state::AppState, utils::rate_limit::check_rate_limit,
    websocket::connection::proceed_with_socket,
};

pub async fn ws_handler(
    query: Result<Query<WsQuery>, axum::extract::rejection::QueryRejection>,
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<axum::response::Response, (StatusCode, Json<Value>)> {
    let query = match query {
        Ok(q) => q.0,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "BAD_REQUEST" })),
            ));
        }
    };

    // Rate limiting
    check_rate_limit(&state, addr).await?;

    // Prepare headers for authentication request
    let headers_json: HashMap<String, String> = headers
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|v| (k.to_string(), v.to_string())))
        .collect();

    // Make authentication request
    let auth_response = state
        .http_client
        .post(&state.api_endpoint)
        .json(&serde_json::json!({
            "channel": &query.channel,
            "token": &query.token,
            "ip": addr.to_string(),
            "headers": headers_json,
        }))
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await;

    // Process authentication response
    let auth_result = match auth_response {
        Ok(resp) => match resp.json::<crate::models::auth::AuthResponse>().await {
            Ok(auth_data) => {
                if auth_data.ok {
                    Ok(auth_data.metadata)
                } else {
                    Err((
                        StatusCode::UNAUTHORIZED,
                        Json(serde_json::json!({ "error": "Authentication failed" })),
                    ))
                }
            }
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(
                    serde_json::json!({ "error": format!("Failed to parse auth response: {}", e) }),
                ),
            )),
        },
        Err(e) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": format!("API error: {}", e) })),
        )),
    };

    // Return the result
    match auth_result {
        Ok(metadata) => {
            proceed_with_socket(ws, query.channel, addr, headers, state.clone(), metadata)
        }
        Err(err) => Err(err),
    }
}
