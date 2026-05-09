use std::collections::HashMap;
use std::time::{Duration, Instant};

struct CacheEntry<V> {
    value: V,
    inserted_at: Instant,
    last_accessed: Instant,
    ttl: Option<Duration>,
}

impl<V> CacheEntry<V> {
    fn is_expired(&self) -> bool {
        match self.ttl {
            Some(ttl) => self.inserted_at.elapsed() > ttl,
            None => false,
        }
    }
}

pub struct LruCache<V> {
    entries: HashMap<String, CacheEntry<V>>,
    capacity: usize,
    default_ttl: Duration,
    access_order: Vec<String>,
}

impl<V> LruCache<V> {
    pub fn new(capacity: usize, default_ttl: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            capacity,
            default_ttl,
            access_order: Vec::new(),
        }
    }

    pub fn insert(&mut self, key: String, value: V) -> Option<V> {
        self.insert_with_ttl(key, value, self.default_ttl)
    }

    pub fn insert_with_ttl(&mut self, key: String, value: V, ttl: Duration) -> Option<V> {
        let old_value = self.remove(&key);

        if self.capacity == 0 {
            return old_value;
        }

        if self.entries.len() >= self.capacity {
            if let Some(lru_key) = self.access_order.first().cloned() {
                self.remove(&lru_key);
            }
        }

        let now = Instant::now();
        self.entries.insert(
            key.clone(),
            CacheEntry {
                value,
                inserted_at: now,
                last_accessed: now,
                ttl: Some(ttl),
            },
        );
        self.access_order.push(key);

        old_value
    }

    pub fn get(&mut self, key: &str) -> Option<&V> {
        if let Some(entry) = self.entries.get(key) {
            if entry.is_expired() {
                self.remove(key);
                return None;
            }
        }

        if let Some(entry) = self.entries.get_mut(key) {
            entry.last_accessed = Instant::now();
        }

        self.touch_access_order(key);

        self.entries.get(key).map(|e| &e.value)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut V> {
        if let Some(entry) = self.entries.get(key) {
            if entry.is_expired() {
                self.remove(key);
                return None;
            }
        }

        if let Some(entry) = self.entries.get_mut(key) {
            entry.last_accessed = Instant::now();
        }

        self.touch_access_order(key);

        self.entries.get_mut(key).map(|e| &mut e.value)
    }

    pub fn remove(&mut self, key: &str) -> Option<V> {
        self.entries.remove(key).map(|entry| {
            self.access_order.retain(|k| k != key);
            entry.value
        })
    }

    pub fn contains(&self, key: &str) -> bool {
        match self.entries.get(key) {
            Some(entry) => !entry.is_expired(),
            None => false,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.access_order.clear();
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn evict_expired(&mut self) -> usize {
        let expired_keys: Vec<String> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.is_expired())
            .map(|(key, _)| key.clone())
            .collect();

        let count = expired_keys.len();
        for key in expired_keys {
            self.remove(&key);
        }
        count
    }

    fn touch_access_order(&mut self, key: &str) {
        self.access_order.retain(|k| k != key);
        self.access_order.push(key.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cache(capacity: usize) -> LruCache<String> {
        LruCache::new(capacity, Duration::from_secs(300))
    }

    #[test]
    fn test_insert_and_get() {
        let mut cache = make_cache(10);

        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);

        cache.insert("a".to_string(), "alpha".to_string());
        cache.insert("b".to_string(), "bravo".to_string());

        assert_eq!(cache.len(), 2);
        assert!(!cache.is_empty());

        assert_eq!(cache.get("a"), Some(&"alpha".to_string()));
        assert_eq!(cache.get("b"), Some(&"bravo".to_string()));
        assert_eq!(cache.get("c"), None);
    }

    #[test]
    fn test_insert_overwrite_returns_old() {
        let mut cache = make_cache(10);

        cache.insert("key".to_string(), "v1".to_string());
        let old = cache.insert("key".to_string(), "v2".to_string());

        assert_eq!(old, Some("v1".to_string()));
        assert_eq!(cache.get("key"), Some(&"v2".to_string()));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_get_mut() {
        let mut cache = make_cache(10);

        cache.insert("key".to_string(), "original".to_string());

        if let Some(val) = cache.get_mut("key") {
            val.push_str("_modified");
        }

        assert_eq!(cache.get("key"), Some(&"original_modified".to_string()));
    }

    #[test]
    fn test_remove() {
        let mut cache = make_cache(10);

        cache.insert("a".to_string(), "alpha".to_string());
        cache.insert("b".to_string(), "bravo".to_string());

        let removed = cache.remove("a");
        assert_eq!(removed, Some("alpha".to_string()));
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get("a"), None);
        assert_eq!(cache.get("b"), Some(&"bravo".to_string()));

        let removed_again = cache.remove("a");
        assert_eq!(removed_again, None);
    }

    #[test]
    fn test_contains() {
        let mut cache = make_cache(10);

        cache.insert("a".to_string(), "alpha".to_string());

        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));

        cache.remove("a");
        assert!(!cache.contains("a"));
    }

    #[test]
    fn test_clear() {
        let mut cache = make_cache(10);

        cache.insert("a".to_string(), "alpha".to_string());
        cache.insert("b".to_string(), "bravo".to_string());
        cache.clear();

        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.get("a"), None);
    }

    #[test]
    fn test_capacity() {
        let cache: LruCache<String> = LruCache::new(42, Duration::from_secs(60));
        assert_eq!(cache.capacity(), 42);
    }

    #[test]
    fn test_lru_eviction_at_capacity() {
        let mut cache = make_cache(3);

        cache.insert("a".to_string(), "1".to_string());
        cache.insert("b".to_string(), "2".to_string());
        cache.insert("c".to_string(), "3".to_string());
        assert_eq!(cache.len(), 3);

        cache.insert("d".to_string(), "4".to_string());

        assert_eq!(cache.len(), 3);
        assert_eq!(cache.get("a"), None, "a should have been evicted as LRU");
        assert_eq!(cache.get("b"), Some(&"2".to_string()));
        assert_eq!(cache.get("c"), Some(&"3".to_string()));
        assert_eq!(cache.get("d"), Some(&"4".to_string()));
    }

    #[test]
    fn test_lru_eviction_respects_access_order() {
        let mut cache = make_cache(3);

        cache.insert("a".to_string(), "1".to_string());
        cache.insert("b".to_string(), "2".to_string());
        cache.insert("c".to_string(), "3".to_string());

        cache.get("a");

        cache.insert("d".to_string(), "4".to_string());

        assert_eq!(
            cache.get("a"),
            Some(&"1".to_string()),
            "a was recently accessed, should not be evicted"
        );
        assert_eq!(cache.get("b"), None, "b is now LRU and should be evicted");
        assert_eq!(cache.get("c"), Some(&"3".to_string()));
        assert_eq!(cache.get("d"), Some(&"4".to_string()));
    }

    #[test]
    fn test_lru_eviction_on_overwrite_no_double_evict() {
        let mut cache = make_cache(3);

        cache.insert("a".to_string(), "1".to_string());
        cache.insert("b".to_string(), "2".to_string());
        cache.insert("c".to_string(), "3".to_string());

        let old = cache.insert("a".to_string(), "1_new".to_string());
        assert_eq!(old, Some("1".to_string()));
        assert_eq!(
            cache.len(),
            3,
            "overwriting existing key should not increase size or evict"
        );
    }

    #[test]
    fn test_ttl_expiry_on_get() {
        let mut cache = LruCache::new(10, Duration::from_millis(500));

        cache.insert("key".to_string(), "value".to_string());
        assert_eq!(cache.get("key"), Some(&"value".to_string()));

        std::thread::sleep(Duration::from_millis(600));

        assert_eq!(
            cache.get("key"),
            None,
            "expired entry should return None on get"
        );
        assert_eq!(cache.len(), 0, "expired entry should be removed from cache");
    }

    #[test]
    fn test_ttl_expiry_on_contains() {
        let mut cache = LruCache::new(10, Duration::from_millis(500));

        cache.insert("key".to_string(), "value".to_string());
        assert!(cache.contains("key"));

        std::thread::sleep(Duration::from_millis(600));

        assert!(
            !cache.contains("key"),
            "expired entry should not be reported as contained"
        );
    }

    #[test]
    fn test_insert_with_ttl_custom_expiry() {
        let mut cache = LruCache::new(10, Duration::from_secs(300));

        cache.insert_with_ttl(
            "short".to_string(),
            "v1".to_string(),
            Duration::from_millis(100),
        );
        cache.insert_with_ttl(
            "long".to_string(),
            "v2".to_string(),
            Duration::from_secs(300),
        );

        std::thread::sleep(Duration::from_millis(150));

        assert_eq!(
            cache.get("short"),
            None,
            "short TTL entry should be expired"
        );
        assert_eq!(
            cache.get("long"),
            Some(&"v2".to_string()),
            "long TTL entry should still be valid"
        );
    }

    #[test]
    fn test_ttl_expiry_on_get_mut() {
        let mut cache = LruCache::new(10, Duration::from_millis(500));

        cache.insert("key".to_string(), "value".to_string());

        std::thread::sleep(Duration::from_millis(600));

        assert_eq!(
            cache.get_mut("key"),
            None,
            "expired entry should return None on get_mut"
        );
    }

    #[test]
    fn test_evict_expired() {
        let mut cache = LruCache::new(10, Duration::from_millis(200));

        cache.insert("a".to_string(), "1".to_string());
        cache.insert("b".to_string(), "2".to_string());
        cache.insert_with_ttl("c".to_string(), "3".to_string(), Duration::from_secs(300));

        std::thread::sleep(Duration::from_millis(300));

        let evicted = cache.evict_expired();
        assert_eq!(evicted, 2, "two entries with short TTL should be evicted");
        assert_eq!(cache.len(), 1);
        assert_eq!(
            cache.get("c"),
            Some(&"3".to_string()),
            "long TTL entry should remain"
        );
    }

    #[test]
    fn test_evict_expired_none_expired() {
        let mut cache = LruCache::new(10, Duration::from_secs(300));

        cache.insert("a".to_string(), "1".to_string());
        cache.insert("b".to_string(), "2".to_string());

        let evicted = cache.evict_expired();
        assert_eq!(evicted, 0);
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn test_access_order_updated_on_get() {
        let mut cache = make_cache(3);

        cache.insert("a".to_string(), "1".to_string());
        cache.insert("b".to_string(), "2".to_string());
        cache.insert("c".to_string(), "3".to_string());

        cache.get("a");
        cache.get("b");

        cache.insert("d".to_string(), "4".to_string());

        assert_eq!(
            cache.get("c"),
            None,
            "c should be evicted (oldest unaccessed)"
        );
        assert_eq!(cache.get("a"), Some(&"1".to_string()));
        assert_eq!(cache.get("b"), Some(&"2".to_string()));
        assert_eq!(cache.get("d"), Some(&"4".to_string()));
    }

    #[test]
    fn test_access_order_updated_on_get_mut() {
        let mut cache = make_cache(3);

        cache.insert("a".to_string(), "1".to_string());
        cache.insert("b".to_string(), "2".to_string());
        cache.insert("c".to_string(), "3".to_string());

        let _ = cache.get_mut("a");

        cache.insert("d".to_string(), "4".to_string());

        assert_eq!(
            cache.get("b"),
            None,
            "b should be evicted (LRU after a was accessed)"
        );
        assert_eq!(cache.get("a"), Some(&"1".to_string()));
        assert_eq!(cache.get("c"), Some(&"3".to_string()));
        assert_eq!(cache.get("d"), Some(&"4".to_string()));
    }

    #[test]
    fn test_remove_updates_access_order() {
        let mut cache = make_cache(3);

        cache.insert("a".to_string(), "1".to_string());
        cache.insert("b".to_string(), "2".to_string());
        cache.insert("c".to_string(), "3".to_string());

        cache.remove("a");

        cache.insert("d".to_string(), "4".to_string());
        cache.insert("e".to_string(), "5".to_string());

        assert_eq!(cache.len(), 3);
        assert_eq!(
            cache.get("b"),
            None,
            "b should be evicted after a was removed and d,e inserted"
        );
        assert_eq!(cache.get("c"), Some(&"3".to_string()));
        assert_eq!(cache.get("d"), Some(&"4".to_string()));
        assert_eq!(cache.get("e"), Some(&"5".to_string()));
    }

    #[test]
    fn test_clear_resets_access_order() {
        let mut cache = make_cache(3);

        cache.insert("a".to_string(), "1".to_string());
        cache.insert("b".to_string(), "2".to_string());
        cache.clear();

        cache.insert("c".to_string(), "3".to_string());
        cache.insert("d".to_string(), "4".to_string());
        cache.insert("e".to_string(), "5".to_string());
        cache.insert("f".to_string(), "6".to_string());

        assert_eq!(cache.len(), 3);
        assert_eq!(
            cache.get("c"),
            None,
            "c should be evicted after clear reset access order"
        );
    }

    #[test]
    fn test_zero_capacity() {
        let mut cache: LruCache<String> = LruCache::new(0, Duration::from_secs(60));

        cache.insert("a".to_string(), "1".to_string());
        assert_eq!(
            cache.len(),
            0,
            "zero-capacity cache should not hold any entries"
        );
        assert_eq!(cache.get("a"), None);
    }
}
