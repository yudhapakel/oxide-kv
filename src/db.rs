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
}
