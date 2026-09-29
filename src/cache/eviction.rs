use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvictionPolicy {
    Lru,
    TtlOnly,
}

#[derive(Debug)]
pub struct LruTracker {
    access_order: VecDeque<String>,
}

impl LruTracker {
    pub fn new() -> Self {
        Self {
            access_order: VecDeque::new(),
        }
    }

    pub fn record_access(&mut self, key: &str) {
        if let Some(pos) = self.access_order.iter().position(|k| k == key) {
            self.access_order.remove(pos);
        }
        self.access_order.push_back(key.to_string());
    }

    pub fn record_insert(&mut self, key: &str) {
        self.record_access(key);
    }

    pub fn record_delete(&mut self, key: &str) {
        if let Some(pos) = self.access_order.iter().position(|k| k == key) {
            self.access_order.remove(pos);
        }
    }

    pub fn pop_lru(&mut self) -> Option<String> {
        self.access_order.pop_front()
    }
}
