use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        RwLock,
    },
};

use super::{
    entry::CachedResponse,
    eviction::{EvictionPolicy, LruTracker},
};

pub struct CacheShard {
    store: RwLock<ShardStore>,
    pub hits: AtomicU64,
    pub misses: AtomicU64,
    pub sets: AtomicU64,
    pub deletes: AtomicU64,
    pub evictions: AtomicU64,
    pub expired: AtomicU64,
    pub current_bytes: AtomicUsize,
    pub max_bytes: usize,
    pub eviction_policy: EvictionPolicy,
}

struct ShardStore {
    entries: HashMap<String, CachedResponse>,
    lru: LruTracker,
}

impl CacheShard {
    pub fn new(max_bytes: usize, eviction_policy: EvictionPolicy) -> Self {
        Self {
            store: RwLock::new(ShardStore {
                entries: HashMap::new(),
                lru: LruTracker::new(),
            }),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            sets: AtomicU64::new(0),
            deletes: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
            expired: AtomicU64::new(0),
            current_bytes: AtomicUsize::new(0),
            max_bytes,
            eviction_policy,
        }
    }

    pub fn get(&self, key: &str) -> Option<CachedResponse> {
        let mut store = self.store.write().unwrap();
        if let Some(entry) = store.entries.get(key) {
            if entry.is_expired() {
                let size = entry.size_bytes;
                store.entries.remove(key);
                store.lru.record_delete(key);
                self.current_bytes.fetch_sub(size, Ordering::Relaxed);
                self.expired.fetch_add(1, Ordering::Relaxed);
                self.misses.fetch_add(1, Ordering::Relaxed);
                return None;
            }

            let response = entry.clone();
            store.lru.record_access(key);
            self.hits.fetch_add(1, Ordering::Relaxed);
            Some(response)
        } else {
            self.misses.fetch_add(1, Ordering::Relaxed);
            None
        }
    }

    pub fn set(&self, key: String, response: CachedResponse) {
        let mut store = self.store.write().unwrap();
        let new_size = response.size_bytes;

        // If replacing existing key, subtract old size
        if let Some(old) = store.entries.remove(&key) {
            self.current_bytes.fetch_sub(old.size_bytes, Ordering::Relaxed);
            store.lru.record_delete(&key);
        }

        // Evict until within max_bytes limit if eviction is enabled
        if self.max_bytes > 0 && self.eviction_policy == EvictionPolicy::Lru {
            while self.current_bytes.load(Ordering::Relaxed) + new_size > self.max_bytes {
                if let Some(lru_key) = store.lru.pop_lru() {
                    if let Some(evicted) = store.entries.remove(&lru_key) {
                        self.current_bytes.fetch_sub(evicted.size_bytes, Ordering::Relaxed);
                        self.evictions.fetch_add(1, Ordering::Relaxed);
                    }
                } else {
                    break;
                }
            }
        }

        store.lru.record_insert(&key);
        store.entries.insert(key, response);
        self.current_bytes.fetch_add(new_size, Ordering::Relaxed);
        self.sets.fetch_add(1, Ordering::Relaxed);
    }

    pub fn delete(&self, key: &str) -> bool {
        let mut store = self.store.write().unwrap();
        if let Some(entry) = store.entries.remove(key) {
            store.lru.record_delete(key);
            self.current_bytes.fetch_sub(entry.size_bytes, Ordering::Relaxed);
            self.deletes.fetch_add(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    pub fn invalidate_prefix(&self, prefix: &str) -> usize {
        let mut store = self.store.write().unwrap();
        let matching_keys: Vec<String> = store
            .entries
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();

        let count = matching_keys.len();
        for key in matching_keys {
            if let Some(entry) = store.entries.remove(&key) {
                store.lru.record_delete(&key);
                self.current_bytes.fetch_sub(entry.size_bytes, Ordering::Relaxed);
                self.deletes.fetch_add(1, Ordering::Relaxed);
            }
        }
        count
    }

    pub fn clear(&self) {
        let mut store = self.store.write().unwrap();
        store.entries.clear();
        store.lru = LruTracker::new();
        self.current_bytes.store(0, Ordering::Relaxed);
    }

    pub fn len(&self) -> usize {
        let store = self.store.read().unwrap();
        store.entries.len()
    }
}
