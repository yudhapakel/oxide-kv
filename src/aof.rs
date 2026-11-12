use std::path::Path;
use std::sync::Arc;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

use crate::db::Db;
use crate::resp::parse_inline_args;

#[derive(Clone)]
pub struct Aof {
    file: Arc<Mutex<File>>,
}

fn format_aof_arg(arg: &str) -> String {
    let needs_quotes = arg.is_empty()
        || arg
            .chars()
            .any(|c| c.is_whitespace() || c == '"' || c == '\'' || c == '\\');
    if !needs_quotes {
        return arg.to_string();
    }

    let mut out = String::with_capacity(arg.len() + 2);
    out.push('"');
    for ch in arg.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

impl Aof {
    pub async fn new<P: AsRef<Path>>(path: P) -> Result<Self, std::io::Error> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await?;

        Ok(Aof {
            file: Arc::new(Mutex::new(file)),
        })
    }

    pub async fn append_command(&self, args: &[String]) -> Result<(), std::io::Error> {
        let mut file = self.file.lock().await;
        let encoded: Vec<String> = args.iter().map(|a| format_aof_arg(a)).collect();
        let line = encoded.join(" ") + "\n";
        file.write_all(line.as_bytes()).await?;
        file.flush().await?;
        Ok(())
    }

    pub async fn load<P: AsRef<Path>>(path: P, db: &Db) -> Result<usize, std::io::Error> {
        if !path.as_ref().exists() {
            return Ok(0);
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

            let parts = parse_inline_args(trimmed);
            if parts.is_empty() {
                continue;
            }

            let cmd = parts[0].to_uppercase();
            match cmd.as_str() {
                "SET" if parts.len() == 3 => {
                    db.set(parts[1].clone(), parts[2].clone()).await;
                    count += 1;
                }
                "SET" if parts.len() == 5 && parts[3].eq_ignore_ascii_case("EX") => {
                    if let Ok(sec) = parts[4].parse::<u64>() {
                        db.set_with_ttl(parts[1].clone(), parts[2].clone(), Some(sec))
                            .await;
                        count += 1;
                    }
                }
                "MSET" if parts.len() >= 3 && (parts.len() - 1) % 2 == 0 => {
                    let pairs: Vec<(String, String)> = parts[1..]
                        .chunks_exact(2)
                        .map(|c| (c[0].clone(), c[1].clone()))
                        .collect();
                    db.mset(&pairs).await;
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
                "PERSIST" if parts.len() == 2 => {
                    db.persist(&parts[1]).await;
                    count += 1;
                }
                "FLUSHDB" => {
                    db.flushdb().await;
                    count += 1;
                }
                _ => {}
            }
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn replays_mset_persist_and_quoted_values() {
        let path = std::env::temp_dir().join(format!(
            "oxide_kv_test_{}.aof",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        {
            let aof = Aof::new(&path).await.unwrap();
            aof.append_command(&["SET".into(), "greeting".into(), "hello world".into()])
                .await
                .unwrap();
            aof.append_command(&[
                "MSET".into(),
                "a".into(),
                "1".into(),
                "b".into(),
                "two words".into(),
            ])
            .await
            .unwrap();
            aof.append_command(&["EXPIRE".into(), "a".into(), "120".into()])
                .await
                .unwrap();
            aof.append_command(&["PERSIST".into(), "a".into()])
                .await
                .unwrap();
        }

        let db = Db::new();
        let replayed = Aof::load(&path, &db).await.unwrap();
        assert_eq!(replayed, 4);
        assert_eq!(db.get("greeting").await.as_deref(), Some("hello world"));
        assert_eq!(db.get("a").await.as_deref(), Some("1"));
        assert_eq!(db.get("b").await.as_deref(), Some("two words"));
        assert_eq!(db.ttl("a").await, -1);

        let _ = std::fs::remove_file(&path);
    }
}
