use crate::state::AppState;
use std::time::{Duration, Instant};

// Define constants
pub const CACHE_TTL: Duration = Duration::from_secs(30);

pub async fn read_cache(state: &AppState, cache_key: &str) -> Option<Result<(), String>> {
    let cache_read = state.cache.read().await;
    cache_read.get(cache_key).and_then(|entry| {
        if Instant::now().duration_since(entry.timestamp) < CACHE_TTL {
            Some(entry.result.clone())
        } else {
            None
        }
    })
}

pub async fn write_cache(state: &AppState, cache_key: String, result: Result<(), String>) {
    let mut cache_write = state.cache.write().await;
    cache_write.insert(
        cache_key,
        crate::state::CacheEntry {
            result,
            timestamp: Instant::now(),
        },
    );
}
