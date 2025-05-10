use std::sync::Arc;
use tinyrand::RandRange;
use tinyrand_std::thread_rand;

const VALID_CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

pub fn mini_id(length: usize) -> Arc<str> {
    let mut rng = thread_rand();
    let mut id = String::with_capacity(length);
    let char_count = VALID_CHARS.len();

    for _ in 0..length {
        let idx = rng.next_range(0..char_count);
        id.push(VALID_CHARS[idx] as char);
    }

    Arc::from(id)
}
