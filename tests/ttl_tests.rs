use rust_cache_server::cache::{TtlManager, TtlRule};
use std::time::Duration;

#[test]
fn test_ttl_matching_rules() {
    let rules = vec![
        TtlRule::new("/api/products/*", 300),
        TtlRule::new("/api/users/*", 60),
        TtlRule::new("/api/config", 3600),
    ];
    let manager = TtlManager::new(45, rules);

    assert_eq!(manager.get_ttl("/api/products/123"), Duration::from_secs(300));
    assert_eq!(manager.get_ttl("/api/products/list?page=1"), Duration::from_secs(300));
    assert_eq!(manager.get_ttl("/api/users/99"), Duration::from_secs(60));
    assert_eq!(manager.get_ttl("/api/config"), Duration::from_secs(3600));
    assert_eq!(manager.get_ttl("/api/unknown"), Duration::from_secs(45));
}
