use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct WsQuery {
    pub channel: String,
    pub token: String,
}

#[derive(Clone, Deserialize)]
pub struct AuthResponse {
    pub ok: bool,
    pub metadata: HashMap<String, String>,
}
