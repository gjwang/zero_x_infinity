//! Timestamp nonce store for replay attack prevention.
//!
//! Uses atomic compare-and-swap to ensure ts_nonce is monotonically increasing.
//! Each API Key has its own nonce counter, stored in a thread-safe DashMap.

use dashmap::DashMap;
use std::sync::atomic::{AtomicI64, Ordering};

/// Thread-safe timestamp nonce store.
///
/// Stores the last seen ts_nonce for each API Key.
/// Uses atomic operations for lock-free concurrent access.
pub struct TsStore {
    /// Map from API Key to last seen ts_nonce
    store: DashMap<String, AtomicI64>,
}

impl TsStore {
    /// Create a new empty TsStore.
    pub fn new() -> Self {
        Self {
            store: DashMap::new(),
        }
    }

    /// Compare and swap if new value is greater than current.
    ///
    /// Returns `true` if the update was successful (new_ts > old_ts).
    /// Returns `false` if new_ts <= old_ts (replay attack detected).
    ///
    /// # Thread Safety
    /// Uses atomic CAS loop to ensure correctness under concurrent access.
    pub fn compare_and_swap_if_greater(&self, api_key: &str, new_ts: i64) -> bool {
        // Get or create entry for this API Key
        let entry = self
            .store
            .entry(api_key.to_string())
            .or_insert_with(|| AtomicI64::new(0));

        loop {
            let current = entry.load(Ordering::Acquire);

            // Check if new_ts is greater than current
            if new_ts <= current {
                return false; // Replay attack or stale nonce
            }

            // Try to update atomically
            match entry.compare_exchange(current, new_ts, Ordering::Release, Ordering::Acquire) {
                Ok(_) => return true, // Successfully updated
                Err(_) => continue,   // Another thread updated, retry
            }
        }
    }

    /// Get the last seen ts_nonce for an API Key.
    ///
    /// Returns None if the API Key has never been seen.
    pub fn get(&self, api_key: &str) -> Option<i64> {
        self.store
            .get(api_key)
            .map(|entry| entry.load(Ordering::Acquire))
    }

    /// Remove an API Key from the store.
    ///
    /// Used when an API Key is deleted or disabled.
    pub fn remove(&self, api_key: &str) {
        self.store.remove(api_key);
    }

    /// Get the number of tracked API Keys.
    pub fn len(&self) -> usize {
        self.store.len()
    }

    /// Check if the store is empty.
    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }
}

impl Default for TsStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_new_api_key() {
        let store = TsStore::new();
        assert!(store.compare_and_swap_if_greater("AK_TEST", 1000));
        assert_eq!(store.get("AK_TEST"), Some(1000));
    }

    #[test]
    fn test_increasing_nonce() {
        let store = TsStore::new();
        assert!(store.compare_and_swap_if_greater("AK_TEST", 1000));
        assert!(store.compare_and_swap_if_greater("AK_TEST", 2000));
        assert!(store.compare_and_swap_if_greater("AK_TEST", 3000));
        assert_eq!(store.get("AK_TEST"), Some(3000));
    }

    #[test]
    fn test_reject_stale_nonce() {
        let store = TsStore::new();
        assert!(store.compare_and_swap_if_greater("AK_TEST", 2000));
        assert!(!store.compare_and_swap_if_greater("AK_TEST", 1000)); // stale
        assert!(!store.compare_and_swap_if_greater("AK_TEST", 2000)); // same
        assert_eq!(store.get("AK_TEST"), Some(2000));
    }

    #[test]
    fn test_multiple_api_keys() {
        let store = TsStore::new();
        assert!(store.compare_and_swap_if_greater("AK_1", 1000));
        assert!(store.compare_and_swap_if_greater("AK_2", 500));
        assert_eq!(store.get("AK_1"), Some(1000));
        assert_eq!(store.get("AK_2"), Some(500));
    }

    #[test]
    fn test_concurrent_access() {
        let store = Arc::new(TsStore::new());
        let api_key = "AK_CONCURRENT";

        // Spawn multiple threads trying to update the same key
        let mut handles = vec![];
        for i in 0..10 {
            let store = Arc::clone(&store);
            let key = api_key.to_string();
            handles.push(thread::spawn(move || {
                let ts = (i + 1) * 1000;
                store.compare_and_swap_if_greater(&key, ts);
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Final value should be the highest attempted: 10000
        assert_eq!(store.get(api_key), Some(10000));
    }

    #[test]
    fn test_remove() {
        let store = TsStore::new();
        assert!(store.compare_and_swap_if_greater("AK_TEST", 1000));
        assert_eq!(store.len(), 1);

        store.remove("AK_TEST");
        assert_eq!(store.len(), 0);
        assert_eq!(store.get("AK_TEST"), None);
    }
}
