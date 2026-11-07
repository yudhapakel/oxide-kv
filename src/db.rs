use std::collections::HashMap;
use std::collections::hash_map::Keys;
use std::io::SeekFrom;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct Db {
    entries: Arc<RwLock<HashMap<String, String>>>,
}

impl Db {
    pub fn new() -> Self {
        Db {
            entries: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn set(&self, key: String, value: String) {
        let mut map = self.entries.write().await;
        map.insert(key, value);
    }

    pub async fn get(&self, key: &str) -> Option<String> {
        let map = self.entries.read().await;
        map.get(key).cloned()
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
        let map = self.entries.read().await;
        let mut count = 0;
        for key in keys {
            if map.contains_key(key) {
                count += 1
            }
        }
        count
    }

    pub async fn incr(&self, key: String) -> Result<i64, &'static str> {
        let mut map = self.entries.write().await;
        let val_str = map.entry(key).or_insert_with(|| "0".to_string());
        let mut num: i64 = val_str
            .parse()
            .map_err(|_| "value is not an integer or out of range")?;
        num += 1;
        *val_str = num.to_string();
        Ok((num))
    }

    pub async fn decr(&self, key: String) -> Result<i64, &'static str> {
        let mut map = self.entries.write().await;
        let val_str = map.entry(key).or_insert_with(|| "0".to_string());
        let mut num: i64 = val_str
            .parse()
            .map_err(|_| "value is not an integer or out of range")?;
        num -= 1;
        *val_str = num.to_string();
        Ok(num)
    }
}
