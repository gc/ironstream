use reqwest::Client;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;

use crate::models::{channel::ChannelData, client::ClientData};

// #[derive(Clone)]
// pub struct CacheEntry {
//     pub result: Result<(), String>,
//     pub timestamp: Instant,
// }

#[derive(Clone)]
pub struct RateLimitEntry {
    pub count: u32,
    pub last_reset: Instant,
}

pub type Channels = Arc<RwLock<HashMap<Arc<str>, ChannelData>>>;
pub type Clients = Arc<RwLock<HashMap<Arc<str>, ClientData>>>;
// pub type Cache = Arc<RwLock<HashMap<String, CacheEntry>>>;
pub type RateLimits = Arc<RwLock<HashMap<SocketAddr, RateLimitEntry>>>;

pub struct AppState {
    pub channels: Channels,
    pub clients: Clients,
    // pub cache: Cache,
    pub rate_limits: RateLimits,
    pub http_client: Client,
    pub admin_token: String,
    pub api_endpoint: String,
    pub rate_limit_count: u32,
    pub rate_limit_duration: Duration,
}

impl AppState {
    pub fn new(
        admin_token: String,
        api_endpoint: String,
        rate_limit_count: u32,
        rate_limit_duration: Duration,
    ) -> Self {
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            clients: Arc::new(RwLock::new(HashMap::new())),
            // cache: Arc::new(RwLock::new(HashMap::new())),
            rate_limits: Arc::new(RwLock::new(HashMap::new())),
            http_client: Client::new(),
            admin_token,
            api_endpoint,
            rate_limit_count,
            rate_limit_duration,
        }
    }
}
