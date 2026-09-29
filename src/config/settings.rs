use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Settings {
    pub server: ServerConfig,
    pub backend: BackendConfig,
    pub cache: CacheConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_admin_api_key")]
    pub admin_api_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BackendConfig {
    pub url: String,
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CacheConfig {
    #[serde(default = "default_cache_enabled")]
    pub enabled: bool,
    #[serde(default = "default_ttl_seconds")]
    pub default_ttl_seconds: u64,
    #[serde(default = "default_max_memory_mb")]
    pub max_memory_mb: usize,
    #[serde(default = "default_shards")]
    pub shards: usize,
    #[serde(default = "default_eviction_policy")]
    pub eviction_policy: String,
    pub negative_ttl_seconds: Option<u64>,
    #[serde(default)]
    pub rules: Vec<CacheRuleConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CacheRuleConfig {
    pub pattern: String,
    pub ttl_seconds: u64,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_admin_api_key() -> Option<String> {
    None
}

fn default_timeout_seconds() -> u64 {
    10
}

fn default_cache_enabled() -> bool {
    true
}

fn default_ttl_seconds() -> u64 {
    60
}

fn default_max_memory_mb() -> usize {
    1024
}

fn default_shards() -> usize {
    16
}

fn default_eviction_policy() -> String {
    "lru".to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: default_host(),
                port: default_port(),
                admin_api_key: None,
            },
            backend: BackendConfig {
                url: "http://localhost:5000".to_string(),
                timeout_seconds: default_timeout_seconds(),
            },
            cache: CacheConfig {
                enabled: true,
                default_ttl_seconds: default_ttl_seconds(),
                max_memory_mb: default_max_memory_mb(),
                shards: default_shards(),
                eviction_policy: default_eviction_policy(),
                negative_ttl_seconds: Some(10),
                rules: vec![
                    CacheRuleConfig {
                        pattern: "/api/products/*".to_string(),
                        ttl_seconds: 300,
                    },
                    CacheRuleConfig {
                        pattern: "/api/users/*".to_string(),
                        ttl_seconds: 60,
                    },
                    CacheRuleConfig {
                        pattern: "/api/config/*".to_string(),
                        ttl_seconds: 3600,
                    },
                ],
            },
        }
    }
}

impl Settings {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let content = fs::read_to_string(path)?;
        let settings: Settings = toml::from_str(&content)?;
        Ok(settings)
    }

    pub fn load() -> Self {
        // Try config/cache.toml, then cache.toml, then default
        if let Ok(settings) = Self::load_from_file("config/cache.toml") {
            settings
        } else if let Ok(settings) = Self::load_from_file("cache.toml") {
            settings
        } else {
            Self::default()
        }
    }
}
