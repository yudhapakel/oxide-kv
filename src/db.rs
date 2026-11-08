use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Clone)]
struct Entry {
    value: String,
    expires_at: Option<Instant>,
}

impl Entry {
    fn is_expired(&self) -> bool {
        if let Some(exp) = self.expires_at {
            Instant::now() >= exp
        } else {
            false
        }
    }
}

#[derive(Clone)]
pub struct Db {
    entries: Arc<RwLock<HashMap<String, Entry>>>,
}

impl Db {
    pub fn new() -> Self {
        Db {
            entries: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn set(&self, key: String, value: String) {
        let mut map = self.entries.write().await;
        map.insert(
            key,
            Entry {
                value,
                expires_at: None,
            },
        );
    }

    pub async fn get(&self, key: &str) -> Option<String> {
        let mut map = self.entries.write().await;
        if let Some(entry) = map.get(key) {
            if entry.is_expired() {
                map.remove(key);
                None
            } else {
                Some(entry.value.clone())
            }
        } else {
            None
        }
    }

    pub async fn del(&self, keys: &[String]) -> i64 {
        let mut map = self.entries.write().await;
        let mut count = 0;
        for key in keys {
            if map.remove(key).is_some() {
                count += 1;
            }
        }
        count
    }

    pub async fn exists(&self, keys: &[String]) -> i64 {
        let mut map = self.entries.write().await;
        let mut count = 0;
        for key in keys {
            if let Some(entry) = map.get(key) {
                if entry.is_expired() {
                    map.remove(key);
                } else {
                    count += 1;
                }
            }
        }
        count
    }

    pub async fn incr(&self, key: String) -> Result<i64, &'static str> {
        let mut map = self.entries.write().await;
        if let Some(entry) = map.get_mut(&key) {
            if entry.is_expired() {
                entry.value = "1".to_string();
                entry.expires_at = None;
                return Ok(1);
            }
            let mut num: i64 = entry
                .value
                .parse()
                .map_err(|_| "value is not an integer or out of range")?;
            num += 1;
            entry.value = num.to_string();
            Ok(num)
        } else {
            map.insert(
                key,
                Entry {
                    value: "1".to_string(),
                    expires_at: None,
                },
            );
            Ok(1)
        }
    }

    pub async fn decr(&self, key: String) -> Result<i64, &'static str> {
        let mut map = self.entries.write().await;
        if let Some(entry) = map.get_mut(&key) {
            if entry.is_expired() {
                entry.value = "-1".to_string();
                entry.expires_at = None;
                return Ok(-1);
            }
            let mut num: i64 = entry
                .value
                .parse()
                .map_err(|_| "value is not an integer or out of range")?;
            num -= 1;
            entry.value = num.to_string();
            Ok(num)
        } else {
            map.insert(
                key,
                Entry {
                    value: "-1".to_string(),
                    expires_at: None,
                },
            );
            Ok(-1)
        }
    }

    pub async fn expire(&self, key: &str, seconds: u64) -> i64 {
        let mut map = self.entries.write().await;
        if let Some(entry) = map.get_mut(key) {
            if entry.is_expired() {
                map.remove(key);
                0
            } else {
                entry.expires_at = Some(Instant::now() + Duration::from_secs(seconds));
                1
            }
        } else {
            0
        }
    }

    pub async fn ttl(&self, key: &str) -> i64 {
        let mut map = self.entries.write().await;
        if let Some(entry) = map.get(key) {
            if entry.is_expired() {
                map.remove(key);
                -2
            } else if let Some(exp) = entry.expires_at {
                let remaining = exp.saturating_duration_since(Instant::now());
                remaining.as_secs() as i64
            } else {
                -1
            }
        } else {
            -2
        }
    }

    pub async fn purge_expired(&self) -> usize {
        let mut map = self.entries.write().await;
        let before_count = map.len();

        map.retain(|_, entry| !entry.is_expired());

        let removed = before_count - map.len();
        removed
    }
}
