# Rust Cache Gateway

A production-grade, high-performance HTTP reverse proxy and standalone caching gateway written in Rust with Tokio and Axum.

---

## Architecture Overview

```text
                         Internet / Clients
                                 |
                                 v
                     +-----------------------+
                     |  Rust Cache Gateway   |
                     |  (Axum + Tokio)       |
                     |                       |
                     |  • Sharded Cache      |
                     |  • Single-Flight      |
                     |  • LRU Eviction       |
                     |  • Route TTL          |
                     |  • Prometheus Metrics |
                     +-----------+-----------+
                                 |
                     +-----------+-----------+
                     |                       |
                 CACHE HIT               CACHE MISS
                     |                       |
                     v                       v
               Cached Response         Backend API
                                       (FastAPI / Django /
                                        Laravel / Node / Go)
                                             |
                                             v
                                         Database
```

---

## Key Features

- **Transparent HTTP Reverse Proxy**: Preserves methods, paths, normalized query parameters, headers, and request bodies.
- **High-Performance Sharded In-Memory Cache**: Fine-grained locking across shards using fast `ahash` hashing to minimize contention.
- **Cache Stampede / Dogpiling Protection (Single-Flight)**: Requests for the same cold cache key are coalesced into a single backend fetch.
- **Smart Cache Invalidation**: Automatic invalidation of path entries upon successful mutating requests (`PUT`, `PATCH`, `DELETE`).
- **LRU Eviction with Strict Memory Accounting**: Configurable memory caps per shard with least-recently-used eviction.
- **Route-Based Pattern TTL**: Pattern matching support (e.g. `/api/products/* -> 300s`, `/api/users/* -> 60s`).
- **Safety First**:
  - Authenticated requests (`Authorization` header) bypass public caching by default.
  - Safe handling of hop-by-hop headers (`Connection`, `Keep-Alive`, `Upgrade`, etc.).
  - `Cache-Control: no-store` and `private` responses are not stored.
- **Observability**: Prometheus metrics (`/metrics`), structured JSON logging (`tracing`), health endpoint (`/health`), and admin statistics (`/admin/stats`).

---

## Project Structure

```text
rust-cache-server/
│
├── Cargo.toml
├── Dockerfile
├── docker-compose.yml
├── README.md
│
├── config/
│   └── cache.toml
│
├── src/
│   ├── main.rs                  # Server entrypoint and lifecycle
│   ├── lib.rs                   # Library exports for integration testing
│   │
│   ├── config/                  # Configuration loaders (TOML)
│   │   ├── mod.rs
│   │   └── settings.rs
│   │
│   ├── cache/                   # High-performance cache engine
│   │   ├── mod.rs
│   │   ├── engine.rs            # Core sharded cache coordinator
│   │   ├── entry.rs             # CachedResponse model and metadata
│   │   ├── eviction.rs          # LRU eviction tracker
│   │   ├── key.rs               # Normalized cache key generator
│   │   ├── shard.rs             # Shard implementation with memory bounds
│   │   ├── singleflight.rs      # Stampede request coalescing
│   │   └── ttl.rs               # Route-based TTL rule manager
│   │
│   ├── proxy/                   # Reverse proxy pipeline
│   │   ├── mod.rs
│   │   ├── forward.rs           # Connection-pooled Reqwest backend forwarder
│   │   ├── request.rs           # Hop-by-hop header filtering and auth detection
│   │   └── response.rs          # Response builder and cacheability policy
│   │
│   ├── middleware/              # Routing and policy middleware
│   │   ├── mod.rs
│   │   ├── cache.rs             # Master proxy request handler
│   │   ├── invalidation.rs      # Cache invalidation on mutation
│   │   └── policy.rs            # HTTP Method caching policy
│   │
│   ├── metrics/                 # Observability
│   │   ├── mod.rs
│   │   └── counters.rs          # Prometheus counters, gauges, histograms
│   │
│   ├── api/                     # Management endpoints
│   │   ├── mod.rs
│   │   ├── admin.rs             # Admin stats and cache purge endpoints
│   │   └── health.rs            # Health check endpoint
│   │
│   └── errors/
│       ├── mod.rs
│       └── error.rs             # Gateway error types and HTTP conversions
│
├── tests/
│   ├── cache_tests.rs           # Unit tests for cache engine & LRU
│   ├── ttl_tests.rs             # Route TTL matching tests
│   ├── invalidation_tests.rs    # Invalidation tests
│   ├── proxy_tests.rs           # Proxy headers & policy tests
│   └── integration_tests.rs     # End-to-end integration test with mock backend
│
└── benches/
    └── cache_benchmark.rs       # Criterion benchmark suite
```

---

## Configuration (`config/cache.toml`)

```toml
[server]
host = "0.0.0.0"
port = 8080

[backend]
url = "http://localhost:5000"
timeout_seconds = 10

[cache]
enabled = true
default_ttl_seconds = 60
max_memory_mb = 1024
shards = 16
eviction_policy = "lru"
negative_ttl_seconds = 10

[[cache.rules]]
pattern = "/api/products/*"
ttl_seconds = 300

[[cache.rules]]
pattern = "/api/users/*"
ttl_seconds = 60

[[cache.rules]]
pattern = "/api/config/*"
ttl_seconds = 3600
```

---

## Running the Application

### 1. Run with Cargo
```bash
cargo run --release
```

### 2. Run Tests
```bash
cargo test
```

### 3. Run Benchmarks
```bash
cargo bench
```

### 4. Run with Docker Compose
```bash
docker-compose up --build
```

---

## Endpoints

| Endpoint | Method | Description |
|---|---|---|
| `/*` | ANY | Transparent reverse proxy to configured backend |
| `/health` | GET | Health status and summary stats |
| `/metrics` | GET | Prometheus metrics export |
| `/admin/stats` | GET | Detailed JSON stats (hits, misses, memory, rate) |
| `/admin/cache` | DELETE | Purge entire cache |
| `/admin/cache/:key` | DELETE | Purge specific cache key |
