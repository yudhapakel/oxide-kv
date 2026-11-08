use std::path::{self, Path};
use std::sync::Arc;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

use crate::db;

#[derive(Clone)]
pub struct Aof {
    file: Arc<Mutex<File>>,
}

impl Aof {
    pub async fn new<P: AsRef<Path>>(path: P) -> Result<Self, std::io::Error> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await?;

        Ok((Aof {
            file: Arc::new(Mutex::new(file)),
        }))
    }

    pub async fn append_command(&self, args: &[String]) -> Result<(), std::io::Error> {
        let mut file = self.file.lock().await;
        let line = args.join(" ") + "\n";
        file.write_all(line.as_bytes()).await?;
        file.flush().await?;
        Ok(())
    }

    pub async fn load<P: AsRef<Path>>(
        path: P,
        db: &crate::db::Db,
    ) -> Result<usize, std::io::Error> {
        if !path.as_ref().exists() {
            return Ok((0));
        }

        let file = File::open(path).await?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        let mut count = 0;

        while let Some(line) = lines.next_line().await? {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let parts: Vec<String> = trimmed.split_whitespace().map(|s| s.to_string()).collect();
            if parts.is_empty() {
                continue;
            }

            let cmd = parts[0].to_uppercase();
            match cmd.as_str() {
                "SET" if parts.len() >= 3 => {
                    db.set(parts[1].clone(), parts[2].clone()).await;
                    count += 1;
                }
                "DEL" if parts.len() >= 2 => {
                    db.del(&parts[1..]).await;
                    count += 1;
                }
                "INCR" if parts.len() == 2 => {
                    let _ = db.incr(parts[1].clone()).await;
                    count += 1;
                }
                "DECR" if parts.len() == 2 => {
                    let _ = db.decr(parts[1].clone()).await;
                    count += 1;
                }
                "EXPIRE" if parts.len() == 3 => {
                    if let Ok(sec) = parts[2].parse::<u64>() {
                        db.expire(&parts[1], sec).await;
                        count += 1;
                    }
                }
                "FLUSHDB" => {
                    db.flushdb().await;
                    count += 1;
                }
                _ => {}
            }
        }
        Ok((count))
    }
}
