use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, Mutex},
};
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct SingleFlight<T: Clone + Send + 'static> {
    in_flight: Arc<Mutex<HashMap<String, broadcast::Sender<T>>>>,
}

impl<T: Clone + Send + 'static> Default for SingleFlight<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + Send + 'static> SingleFlight<T> {
    pub fn new() -> Self {
        Self {
            in_flight: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Execute a function or wait for an in-flight execution for the given key
    pub async fn execute<F, Fut>(&self, key: &str, f: F) -> Result<T, String>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, String>>,
    {
        // 1. Check if there is already an in-flight request while holding lock briefly
        let (rx_opt, is_leader) = {
            let mut map = self.in_flight.lock().unwrap();
            if let Some(sender) = map.get(key) {
                (Some(sender.subscribe()), false)
            } else {
                let (tx, _rx) = broadcast::channel(1);
                map.insert(key.to_string(), tx);
                (None, true)
            }
        };

        // 2. If leader, execute the task (no lock held across .await!)
        if is_leader {
            let result = f().await;

            // Broadcast result and remove key
            let mut map = self.in_flight.lock().unwrap();
            if let Some(sender) = map.remove(key) {
                if let Ok(ref val) = result {
                    let _ = sender.send(val.clone());
                }
            }
            return result;
        }

        // 3. If follower, wait for the leader's broadcast
        if let Some(mut rx) = rx_opt {
            match rx.recv().await {
                Ok(val) => Ok(val),
                Err(_) => Err("Single-flight leader dropped without producing result".to_string()),
            }
        } else {
            Err("Failed to subscribe to in-flight single flight channel".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[tokio::test]
    async fn test_single_flight_coalescing() {
        let group = SingleFlight::<String>::new();
        let calls = Arc::new(AtomicUsize::new(0));

        let mut handles = vec![];
        for _ in 0..10 {
            let g = group.clone();
            let c = Arc::clone(&calls);
            handles.push(tokio::spawn(async move {
                g.execute("test-key", || async {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    c.fetch_add(1, Ordering::SeqCst);
                    Ok("computed_value".to_string())
                })
                .await
            }));
        }

        for h in handles {
            let res = h.await.unwrap();
            assert_eq!(res.unwrap(), "computed_value");
        }

        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
