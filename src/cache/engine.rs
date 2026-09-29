use std::{
    hash::{BuildHasher, Hasher},
    sync::{
        atomic::Ordering,
        Arc,
    },
    time::Duration,
};

use ahash::RandomState;
use bytes::Bytes;
use http::HeaderMap;

use super::{
    entry::CachedResponse,
    eviction::EvictionPolicy,
    key::CacheKey,
    shard::CacheShard,
    singleflight::SingleFlight,
    ttl::TtlManager,
};

#[derive(Clone)]
pub struct CacheEngine {
    shards: Arc<Vec<CacheShard>>,
    hasher_builder: RandomState,
    pub ttl_manager: Arc<TtlManager>,
    pub single_flight: SingleFlight<CachedResponse>,
    pub negative_ttl: Option<Duration>,
    pub enabled: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CacheStats {
    pub keys: usize,
    pub hits: u64,
    pub misses: u64,
    pub sets: u64,
    pub deletes: u64,
    pub evictions: u64,
    pub expired: u64,
    pub memory_bytes: usize,
    pub hit_rate: f64,
    pub shard_count: usize,
}

impl CacheEngine {
    pub fn new(
        shard_count: usize,
        max_memory_bytes: usize,
        eviction_policy: EvictionPolicy,
        ttl_manager: TtlManager,
        negative_ttl_seconds: Option<u64>,
        enabled: bool,
    ) -> Self {
        let num_shards = if shard_count == 0 { 16 } else { shard_count };
        let max_bytes_per_shard = if max_memory_bytes > 0 {
            max_memory_bytes / num_shards
        } else {
            0
        };

        let mut shards = Vec::with_capacity(num_shards);
        for _ in 0..num_shards {
            shards.push(CacheShard::new(max_bytes_per_shard, eviction_policy));
        }

        Self {
            shards: Arc::new(shards),
            hasher_builder: RandomState::new(),
            ttl_manager: Arc::new(ttl_manager),
            single_flight: SingleFlight::new(),
            negative_ttl: negative_ttl_seconds.map(Duration::from_secs),
            enabled,
        }
    }

    /// Fast hashing to select shard index
    fn get_shard_index(&self, key: &str) -> usize {
        let mut hasher = self.hasher_builder.build_hasher();
        hasher.write(key.as_bytes());
        (hasher.finish() as usize) % self.shards.len()
    }

    fn shard(&self, key: &str) -> &CacheShard {
        let idx = self.get_shard_index(key);
        &self.shards[idx]
    }

    pub fn get(&self, key: &CacheKey) -> Option<CachedResponse> {
        if !self.enabled {
            return None;
        }
        self.shard(key.as_str()).get(key.as_str())
    }

    pub fn set(
        &self,
        key: &CacheKey,
        path: &str,
        status: u16,
        headers: &HeaderMap,
        body: Bytes,
        custom_ttl: Option<Duration>,
    ) {
        if !self.enabled {
            return;
        }

        let ttl = match custom_ttl {
            Some(t) => t,
            None => {
                if status == 404 {
                    if let Some(neg_ttl) = self.negative_ttl {
                        neg_ttl
                    } else {
                        self.ttl_manager.get_ttl(path)
                    }
                } else {
                    self.ttl_manager.get_ttl(path)
                }
            }
        };

        let expires_at = std::time::Instant::now() + ttl;
        let response = CachedResponse::new(status, headers, body, expires_at);
        self.shard(key.as_str()).set(key.as_str().to_string(), response);
    }

    pub fn delete(&self, key: &CacheKey) -> bool {
        self.shard(key.as_str()).delete(key.as_str())
    }

    /// Invalidate cache entries associated with a URI path (e.g. GET:/api/users/100)
    pub fn invalidate_path(&self, path: &str) -> usize {
        let mut total = 0;
        let target_prefix = format!("GET:{}", path);
        for shard in self.shards.iter() {
            total += shard.invalidate_prefix(&target_prefix);
        }
        total
    }

    pub fn clear_all(&self) {
        for shard in self.shards.iter() {
            shard.clear();
        }
    }

    pub fn stats(&self) -> CacheStats {
        let mut total_keys = 0;
        let mut total_hits = 0;
        let mut total_misses = 0;
        let mut total_sets = 0;
        let mut total_deletes = 0;
        let mut total_evictions = 0;
        let mut total_expired = 0;
        let mut total_memory = 0;

        for shard in self.shards.iter() {
            total_keys += shard.len();
            total_hits += shard.hits.load(Ordering::Relaxed);
            total_misses += shard.misses.load(Ordering::Relaxed);
            total_sets += shard.sets.load(Ordering::Relaxed);
            total_deletes += shard.deletes.load(Ordering::Relaxed);
            total_evictions += shard.evictions.load(Ordering::Relaxed);
            total_expired += shard.expired.load(Ordering::Relaxed);
            total_memory += shard.current_bytes.load(Ordering::Relaxed);
        }

        let total_requests = total_hits + total_misses;
        let hit_rate = if total_requests == 0 {
            0.0
        } else {
            (total_hits as f64 / total_requests as f64) * 100.0
        };

        CacheStats {
            keys: total_keys,
            hits: total_hits,
            misses: total_misses,
            sets: total_sets,
            deletes: total_deletes,
            evictions: total_evictions,
            expired: total_expired,
            memory_bytes: total_memory,
            hit_rate,
            shard_count: self.shards.len(),
        }
    }
}
