use prometheus::{
    Encoder, HistogramOpts, HistogramVec, IntCounter, IntCounterVec, IntGauge, Opts, Registry,
    TextEncoder,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct MetricsRegistry {
    pub registry: Arc<Registry>,
    pub cache_hits: IntCounter,
    pub cache_misses: IntCounter,
    pub cache_sets: IntCounter,
    pub cache_deletes: IntCounter,
    pub cache_evictions: IntCounter,
    pub cache_expirations: IntCounter,
    pub cache_entries: IntGauge,
    pub cache_memory_bytes: IntGauge,
    pub backend_requests: IntCounterVec,
    pub backend_errors: IntCounterVec,
    pub backend_duration_seconds: HistogramVec,
    pub http_requests_total: IntCounterVec,
    pub http_request_duration_seconds: HistogramVec,
}

impl MetricsRegistry {
    pub fn new() -> Self {
        let registry = Registry::new();

        let cache_hits = IntCounter::with_opts(
            Opts::new("cache_hits_total", "Total number of cache hits"),
        ).unwrap();
        let cache_misses = IntCounter::with_opts(
            Opts::new("cache_misses_total", "Total number of cache misses"),
        ).unwrap();
        let cache_sets = IntCounter::with_opts(
            Opts::new("cache_sets_total", "Total number of cache sets"),
        ).unwrap();
        let cache_deletes = IntCounter::with_opts(
            Opts::new("cache_deletes_total", "Total number of cache deletes"),
        ).unwrap();
        let cache_evictions = IntCounter::with_opts(
            Opts::new("cache_evictions_total", "Total number of LRU cache evictions"),
        ).unwrap();
        let cache_expirations = IntCounter::with_opts(
            Opts::new("cache_expirations_total", "Total number of expired cache entries"),
        ).unwrap();
        let cache_entries = IntGauge::with_opts(
            Opts::new("cache_entries", "Current number of cache entries"),
        ).unwrap();
        let cache_memory_bytes = IntGauge::with_opts(
            Opts::new("cache_memory_bytes", "Current memory used by cache entries in bytes"),
        ).unwrap();

        let backend_requests = IntCounterVec::new(
            Opts::new("backend_requests_total", "Total backend HTTP requests"),
            &["method", "status"],
        ).unwrap();
        let backend_errors = IntCounterVec::new(
            Opts::new("backend_errors_total", "Total backend errors"),
            &["type"],
        ).unwrap();
        let backend_duration_seconds = HistogramVec::new(
            HistogramOpts::new("backend_duration_seconds", "Backend request latency in seconds")
                .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]),
            &["method"],
        ).unwrap();

        let http_requests_total = IntCounterVec::new(
            Opts::new("http_requests_total", "Total HTTP requests handled by the gateway"),
            &["method", "cache_status"],
        ).unwrap();
        let http_request_duration_seconds = HistogramVec::new(
            HistogramOpts::new("http_request_duration_seconds", "Total HTTP request duration in seconds")
                .buckets(vec![0.0005, 0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]),
            &["cache_status"],
        ).unwrap();

        registry.register(Box::new(cache_hits.clone())).unwrap();
        registry.register(Box::new(cache_misses.clone())).unwrap();
        registry.register(Box::new(cache_sets.clone())).unwrap();
        registry.register(Box::new(cache_deletes.clone())).unwrap();
        registry.register(Box::new(cache_evictions.clone())).unwrap();
        registry.register(Box::new(cache_expirations.clone())).unwrap();
        registry.register(Box::new(cache_entries.clone())).unwrap();
        registry.register(Box::new(cache_memory_bytes.clone())).unwrap();
        registry.register(Box::new(backend_requests.clone())).unwrap();
        registry.register(Box::new(backend_errors.clone())).unwrap();
        registry.register(Box::new(backend_duration_seconds.clone())).unwrap();
        registry.register(Box::new(http_requests_total.clone())).unwrap();
        registry.register(Box::new(http_request_duration_seconds.clone())).unwrap();

        Self {
            registry: Arc::new(registry),
            cache_hits,
            cache_misses,
            cache_sets,
            cache_deletes,
            cache_evictions,
            cache_expirations,
            cache_entries,
            cache_memory_bytes,
            backend_requests,
            backend_errors,
            backend_duration_seconds,
            http_requests_total,
            http_request_duration_seconds,
        }
    }

    pub fn render_prometheus(&self) -> String {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = vec![];
        encoder.encode(&metric_families, &mut buffer).unwrap_or_default();
        String::from_utf8(buffer).unwrap_or_default()
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}
