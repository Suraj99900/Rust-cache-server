use bytes::Bytes;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use http::HeaderMap;
use rust_cache_server::cache::{CacheEngine, CacheKey, EvictionPolicy, TtlManager};

fn bench_cache_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("Cache Operations");
    let ttl_manager = TtlManager::new(60, vec![]);
    let engine = CacheEngine::new(16, 1024 * 1024 * 100, EvictionPolicy::Lru, ttl_manager, None, true);

    let key = CacheKey::new("GET", "/api/products/100", None);
    let body = Bytes::from("{\"id\": 100, \"name\": \"Benchmark Item\"}");
    let headers = HeaderMap::new();

    // Populate key for GET HIT benchmark
    engine.set(&key, "/api/products/100", 200, &headers, body.clone(), None);

    group.bench_function("GET Cache HIT", |b| {
        b.iter(|| {
            let res = engine.get(black_box(&key));
            black_box(res)
        });
    });

    group.bench_function("SET Cache Entry", |b| {
        let mut i = 0;
        b.iter(|| {
            i += 1;
            let k = CacheKey::new("GET", &format!("/api/products/{}", i), None);
            engine.set(&k, "/api/products", 200, &headers, body.clone(), None);
        });
    });

    group.finish();
}

fn bench_sharding_scalability(c: &mut Criterion) {
    let mut group = c.benchmark_group("Sharding Scaling");
    let shard_counts = [1, 4, 8, 16, 32];
    let body = Bytes::from("benchmark payload data");
    let headers = HeaderMap::new();

    for &shards in &shard_counts {
        let ttl_manager = TtlManager::new(60, vec![]);
        let engine = CacheEngine::new(
            shards,
            1024 * 1024 * 100,
            EvictionPolicy::Lru,
            ttl_manager,
            None,
            true,
        );

        // Pre-populate 1000 keys
        for i in 0..1000 {
            let k = CacheKey::new("GET", &format!("/api/item/{}", i), None);
            engine.set(&k, "/api/item", 200, &headers, body.clone(), None);
        }

        group.bench_with_input(
            BenchmarkId::new("Concurrent-style GET Hits across shards", shards),
            &shards,
            |b, _| {
                let mut i = 0usize;
                b.iter(|| {
                    i = (i + 1) % 1000;
                    let k = CacheKey::new("GET", &format!("/api/item/{}", i), None);
                    let res = engine.get(black_box(&k));
                    black_box(res)
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_cache_operations, bench_sharding_scalability);
criterion_main!(benches);
