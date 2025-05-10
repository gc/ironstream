use axum::{Router, http::StatusCode, routing::get};
use std::{net::SocketAddr, sync::Arc, time::Duration};

use crate::api::routes;
use crate::api::types::{ApiResponse, ApiResult};
use crate::state::AppState;
use crate::websocket::handler;

pub struct Server {
    state: Arc<AppState>,
    port: u16,
}

impl Server {
    pub fn new(
        admin_token: String,
        api_endpoint: String,
        rate_limit_count: u32,
        rate_limit_seconds: u64,
        port: Option<u16>,
    ) -> Self {
        let state = AppState::new(
            admin_token,
            api_endpoint,
            rate_limit_count,
            Duration::from_secs(rate_limit_seconds),
        );
        let state_arc = Arc::new(state);

        Self {
            state: state_arc,
            port: port.unwrap_or(3113),
        }
    }

    pub async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let app = Router::new()
            .route("/ws", get(handler::ws_handler))
            .merge(routes::configure_api_routes(self.state.clone()))
            .fallback(|| async {
                (
                    StatusCode::NOT_FOUND,
                    ApiResult(ApiResponse::<()>::error("Not found")),
                )
            })
            .with_state(self.state.clone());

        let url = format!("0.0.0.0:{}", self.port);

        axum::serve(
            tokio::net::TcpListener::bind(&url).await?,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await?;

        Ok(())
    }
}
