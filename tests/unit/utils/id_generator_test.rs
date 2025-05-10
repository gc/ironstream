#[cfg(test)]
mod tests {
    use crate::utils::id_generator::mini_id;
    use std::collections::HashSet;

    #[test]
    fn test_mini_id_length() {
        let id = mini_id(8);
        assert_eq!(id.len(), 8);

        let id = mini_id(16);
        assert_eq!(id.len(), 16);
    }

    #[test]
    fn test_mini_id_uniqueness() {
        let mut ids = HashSet::new();
        for _ in 0..1000 {
            let id = mini_id(8);
            assert!(!ids.contains(&id), "Generated duplicate ID: {}", id);
            ids.insert(id);
        }
    }

    #[test]
    fn test_mini_id_character_set() {
        const VALID_CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
        let valid_chars: HashSet<char> = VALID_CHARS.iter().map(|&b| b as char).collect();

        for _ in 0..100 {
            let id = mini_id(10);
            for c in id.chars() {
                assert!(
                    valid_chars.contains(&c),
                    "Generated ID contains invalid character: {}",
                    c
                );
            }
        }
    }
}
