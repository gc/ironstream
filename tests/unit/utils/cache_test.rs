#[cfg(test)]
mod tests {
    use crate::{
        state::AppState,
        utils::cache::{read_cache, write_cache, CACHE_TTL},
    };
    use std::time::Duration;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_cache_write_read() {
        let state = AppState::new(
            "test_token".to_string(),
            "https://example.com".to_string(),
            10,
            Duration::from_secs(1),
        );

        let cache_key = "test:key".to_string();
        let result = Ok(());

        // Write to cache
        write_cache(&state, cache_key.clone(), result.clone()).await;

        // Read from cache
        let cached = read_cache(&state, &cache_key).await;
        assert!(cached.is_some());
        assert_eq!(cached.unwrap(), result);
    }

    #[tokio::test]
    async fn test_cache_expiration() {
        // Use a short TTL for testing
        let short_ttl = Duration::from_millis(100);
        let state = AppState::new(
            "test_token".to_string(),
            "https://example.com".to_string(),
            10,
            Duration::from_secs(1),
        );

        let cache_key = "expiring:key".to_string();
        let result = Ok(());

        // Write to cache
        write_cache(&state, cache_key.clone(), result).await;

        // Verify we can read it immediately
        let cached = read_cache(&state, &cache_key).await;
        assert!(cached.is_some());

        // Wait for the cache to expire
        sleep(short_ttl + Duration::from_millis(50)).await;

        // Cache entry should be gone now or considered expired
        let cached = read_cache(&state, &cache_key).await;
        assert!(cached.is_none());
    }
}
