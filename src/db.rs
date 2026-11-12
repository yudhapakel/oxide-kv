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
        self.set_with_ttl(key, value, None).await;
    }

    pub async fn set_with_ttl(&self, key: String, value: String, ttl_secs: Option<u64>) {
        let mut map = self.entries.write().await;
        let expires_at = ttl_secs.map(|sec| Instant::now() + Duration::from_secs(sec));
        map.insert(key, Entry { value, expires_at });
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
            num = num
                .checked_add(1)
                .ok_or("increment or decrement would overflow")?;
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
            num = num
                .checked_sub(1)
                .ok_or("increment or decrement would overflow")?;
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

    pub async fn persist(&self, key: &str) -> i64 {
        let mut map = self.entries.write().await;
        if let Some(entry) = map.get_mut(key) {
            if entry.is_expired() {
                map.remove(key);
                0
            } else if entry.expires_at.is_some() {
                entry.expires_at = None;
                1
            } else {
                0
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

    pub async fn keys(&self, pattern: &str) -> Vec<String> {
        let mut map = self.entries.write().await;
        map.retain(|_, entry| !entry.is_expired());
        let mut matched: Vec<String> = map
            .keys()
            .filter(|k| glob_match(pattern, k))
            .cloned()
            .collect();
        matched.sort();
        matched
    }

    pub async fn purge_expired(&self) -> usize {
        let mut map = self.entries.write().await;
        let before_count = map.len();
        map.retain(|_, entry| !entry.is_expired());
        before_count - map.len()
    }

    pub async fn dbsize(&self) -> i64 {
        let mut map = self.entries.write().await;
        map.retain(|_, entry| !entry.is_expired());
        map.len() as i64
    }

    pub async fn flushdb(&self) {
        let mut map = self.entries.write().await;
        map.clear();
    }

    pub async fn mset(&self, pairs: &[(String, String)]) {
        let mut map = self.entries.write().await;
        for (k, v) in pairs {
            map.insert(
                k.clone(),
                Entry {
                    value: v.clone(),
                    expires_at: None,
                },
            );
        }
    }

    pub async fn mget(&self, keys: &[String]) -> Vec<Option<String>> {
        let mut map = self.entries.write().await;
        let mut results = Vec::with_capacity(keys.len());

        for key in keys {
            if let Some(entry) = map.get(key) {
                if entry.is_expired() {
                    map.remove(key);
                    results.push(None);
                } else {
                    results.push(Some(entry.value.clone()));
                }
            } else {
                results.push(None);
            }
        }
        results
    }
}

fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0, 0);
    let mut star_pi: Option<usize> = None;
    let mut star_ti = 0;

    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star_pi = Some(pi);
            star_ti = ti;
            pi += 1;
        } else if let Some(sp) = star_pi {
            pi = sp + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }

    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }

    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn set_get_del_and_exists_work() {
        let db = Db::new();
        db.set("user:1".into(), "yudha".into()).await;
        assert_eq!(db.get("user:1").await.as_deref(), Some("yudha"));
        assert_eq!(db.exists(&["user:1".into(), "missing".into()]).await, 1);
        assert_eq!(db.del(&["user:1".into()]).await, 1);
        assert_eq!(db.get("user:1").await, None);
    }

    #[tokio::test]
    async fn incr_decr_and_persist_work() {
        let db = Db::new();
        assert_eq!(db.incr("hits".into()).await.unwrap(), 1);
        assert_eq!(db.incr("hits".into()).await.unwrap(), 2);
        assert_eq!(db.decr("hits".into()).await.unwrap(), 1);

        assert_eq!(db.expire("hits", 60).await, 1);
        assert!(db.ttl("hits").await >= 0);
        assert_eq!(db.persist("hits").await, 1);
        assert_eq!(db.ttl("hits").await, -1);
        assert_eq!(db.persist("hits").await, 0);
    }

    #[tokio::test]
    async fn keys_glob_matching_and_expiration_work() {
        let db = Db::new();
        db.mset(&[
            ("user:1".into(), "a".into()),
            ("user:2".into(), "b".into()),
            ("session:9".into(), "c".into()),
        ])
        .await;
        db.set_with_ttl("temp".into(), "x".into(), Some(0)).await;

        assert_eq!(db.keys("user:*").await, vec!["user:1", "user:2"]);
        assert_eq!(db.keys("user:?").await, vec!["user:1", "user:2"]);
        assert_eq!(db.keys("*").await, vec!["session:9", "user:1", "user:2"]);
        assert_eq!(db.dbsize().await, 3);
    }
}
