use std::time::Duration;

#[derive(Debug, Clone)]
pub struct TtlRule {
    pub pattern: String,
    pub ttl: Duration,
}

impl TtlRule {
    pub fn new(pattern: impl Into<String>, ttl_seconds: u64) -> Self {
        Self {
            pattern: pattern.into(),
            ttl: Duration::from_secs(ttl_seconds),
        }
    }

    /// Check if path matches pattern (supporting trailing wildcard '*')
    pub fn matches(&self, path: &str) -> bool {
        if self.pattern.ends_with('*') {
            let prefix = &self.pattern[..self.pattern.len() - 1];
            path.starts_with(prefix)
        } else {
            self.pattern == path
        }
    }
}

#[derive(Debug, Clone)]
pub struct TtlManager {
    default_ttl: Duration,
    rules: Vec<TtlRule>,
}

impl TtlManager {
    pub fn new(default_ttl_seconds: u64, rules: Vec<TtlRule>) -> Self {
        Self {
            default_ttl: Duration::from_secs(default_ttl_seconds),
            rules,
        }
    }

    pub fn get_ttl(&self, path: &str) -> Duration {
        for rule in &self.rules {
            if rule.matches(path) {
                return rule.ttl;
            }
        }
        self.default_ttl
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ttl_matching() {
        let rules = vec![
            TtlRule::new("/api/products/*", 300),
            TtlRule::new("/api/users/*", 60),
            TtlRule::new("/api/config", 3600),
        ];
        let manager = TtlManager::new(120, rules);

        assert_eq!(manager.get_ttl("/api/products/100"), Duration::from_secs(300));
        assert_eq!(manager.get_ttl("/api/users/20"), Duration::from_secs(60));
        assert_eq!(manager.get_ttl("/api/config"), Duration::from_secs(3600));
        assert_eq!(manager.get_ttl("/other"), Duration::from_secs(120));
    }
}
