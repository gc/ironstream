use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    sync::Arc,
};

#[derive(Clone)]
pub struct ClientData {
    pub id: Arc<str>,
    pub ip: SocketAddr,
    pub user_agent: Option<String>,
    pub channels: HashSet<String>,
    pub connected_at: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

#[derive(Serialize)]
pub struct ClientStats {
    pub id: Arc<str>,
    pub ip: String,
    pub user_agent: Option<String>,
    pub channels: Vec<String>,
    pub connected_at: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

#[derive(Deserialize)]
pub struct DisconnectPayload {
    pub id: Arc<str>,
    pub channel: String,
}
