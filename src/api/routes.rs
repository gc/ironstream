use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use std::sync::Arc;

use super::{handlers, middleware::admin_auth, webhook};
use crate::state::AppState;

pub fn configure_api_routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route(
            "/stats",
            get(handlers::stats_handler)
                .route_layer(middleware::from_fn_with_state(state.clone(), admin_auth)),
        )
        .route(
            "/disconnect",
            post(handlers::disconnect_user)
                .route_layer(middleware::from_fn_with_state(state.clone(), admin_auth)),
        )
        .route(
            "/broadcast/{channel_id}",
            post(webhook::webhook_handler)
                .route_layer(middleware::from_fn_with_state(state.clone(), admin_auth)),
        )
}
