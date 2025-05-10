use chrono::{DateTime, Utc};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct ChannelData {
    pub tx: broadcast::Sender<String>,
    pub message_count: usize,
    pub last_message: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
pub struct ChannelStats {
    pub channel_id: Arc<str>,
    pub connections: usize,
    pub messages: usize,
    pub last_message: Option<DateTime<Utc>>,
}
