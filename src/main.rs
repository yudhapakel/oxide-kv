mod aof;
mod db;
mod resp;

use aof::Aof;
use bytes::BytesMut;
use db::Db;
use resp::{Value, try_parse_command};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{Duration, interval};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let aof_path = "appendonly.aof";
    let db = Db::new();

    match Aof::load(aof_path, &db).await {
        Ok(count) if count > 0 => {
            println!("[aof] Replayed {} mutation(s) from {}", count, aof_path);
        }
        Ok(_) => {
            println!("[aof] No previous state found or file empty");
        }
        Err(e) => {
            eprintln!("[aof] Error loading {}: {:?}", aof_path, e);
        }
    }

    let aof = Aof::new(aof_path).await?;

    let reaper_db = db.clone();
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_millis(500));
        loop {
            ticker.tick().await;
            let purged = reaper_db.purge_expired().await;
            if purged > 0 {
                println!("[reaper] Purged {} expired key(s) from memory", purged);
            }
        }
    });

    let addr = "127.0.0.1:6379";
    let listener = TcpListener::bind(addr).await?;
    println!("oxide-kv server listening on {}", addr);

    loop {
        let (socket, client_addr) = listener.accept().await?;
        println!("Client connected: {}", client_addr);

        let db_clone = db.clone();
        let aof_clone = aof.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_client(socket, db_clone, aof_clone).await {
                eprintln!("Connection error: {:?}", e);
            }
        });
    }
}

async fn handle_client(
    mut socket: TcpStream,
    db: Db,
    aof: Aof,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut buffer = BytesMut::with_capacity(4096);

    loop {
        while let Some((args, consumed)) = match try_parse_command(&buffer) {
            Ok(parsed) => parsed,
            Err(err) => {
                let err_frame = Value::Error(err).serialize();
                socket.write_all(&err_frame).await?;
                buffer.clear();
                None
            }
        } {
            let _ = buffer.split_to(consumed);
            if args.is_empty() {
                continue;
            }
            let response = execute_command(&args, &db, &aof).await;
            socket.write_all(&response.serialize()).await?;
        }

        let bytes_read = socket.read_buf(&mut buffer).await?;
        if bytes_read == 0 {
            break;
        }
    }
    Ok(())
}

async fn execute_command(args: &[String], db: &Db, aof: &Aof) -> Value {
    let cmd = args[0].to_uppercase();
    match cmd.as_str() {
        "PING" => {
            if args.len() > 1 {
                Value::BulkString(args[1].clone())
            } else {
                Value::SimpleString("PONG".to_string())
            }
        }
        "SET" => {
            if args.len() == 3 {
                db.set(args[1].clone(), args[2].clone()).await;
                let _ = aof.append_command(args).await;
                Value::SimpleString("OK".to_string())
            } else if args.len() == 5 && args[3].eq_ignore_ascii_case("EX") {
                match args[4].parse::<u64>() {
                    Ok(sec) => {
                        db.set_with_ttl(args[1].clone(), args[2].clone(), Some(sec))
                            .await;
                        let _ = aof.append_command(args).await;
                        Value::SimpleString("OK".to_string())
                    }
                    Err(_) => Value::Error("value is not an integer or out of range".to_string()),
                }
            } else {
                Value::Error("wrong number of arguments for 'set' command".to_string())
            }
        }
        "GET" => {
            if args.len() != 2 {
                Value::Error("wrong number of arguments for 'get' command".to_string())
            } else {
                match db.get(&args[1]).await {
                    Some(val) => Value::BulkString(val),
                    None => Value::Null,
                }
            }
        }
        "DEL" => {
            if args.len() < 2 {
                Value::Error("wrong number of arguments for 'del' command".to_string())
            } else {
                let deleted = db.del(&args[1..]).await;
                if deleted > 0 {
                    let _ = aof.append_command(args).await;
                }
                Value::Integer(deleted)
            }
        }
        "EXISTS" => {
            if args.len() < 2 {
                Value::Error("wrong number of arguments for 'exists' command".to_string())
            } else {
                Value::Integer(db.exists(&args[1..]).await)
            }
        }
        "INCR" => {
            if args.len() != 2 {
                Value::Error("wrong number of arguments for 'incr' command".to_string())
            } else {
                match db.incr(args[1].clone()).await {
                    Ok(val) => {
                        let _ = aof.append_command(args).await;
                        Value::Integer(val)
                    }
                    Err(err) => Value::Error(err.to_string()),
                }
            }
        }
        "DECR" => {
            if args.len() != 2 {
                Value::Error("wrong number of arguments for 'decr' command".to_string())
            } else {
                match db.decr(args[1].clone()).await {
                    Ok(val) => {
                        let _ = aof.append_command(args).await;
                        Value::Integer(val)
                    }
                    Err(err) => Value::Error(err.to_string()),
                }
            }
        }
        "EXPIRE" => {
            if args.len() != 3 {
                Value::Error("wrong number of arguments for 'expire' command".to_string())
            } else {
                match args[2].parse::<u64>() {
                    Ok(sec) => {
                        let res = db.expire(&args[1], sec).await;
                        if res == 1 {
                            let _ = aof.append_command(args).await;
                        }
                        Value::Integer(res)
                    }
                    Err(_) => Value::Error("value is not an integer or out of range".to_string()),
                }
            }
        }
        "PERSIST" => {
            if args.len() != 2 {
                Value::Error("wrong number of arguments for 'persist' command".to_string())
            } else {
                let res = db.persist(&args[1]).await;
                if res == 1 {
                    let _ = aof.append_command(args).await;
                }
                Value::Integer(res)
            }
        }
        "TTL" => {
            if args.len() != 2 {
                Value::Error("wrong number of arguments for 'ttl' command".to_string())
            } else {
                Value::Integer(db.ttl(&args[1]).await)
            }
        }
        "KEYS" => {
            if args.len() != 2 {
                Value::Error("wrong number of arguments for 'keys' command".to_string())
            } else {
                let matched = db.keys(&args[1]).await;
                Value::Array(matched.into_iter().map(Value::BulkString).collect())
            }
        }
        "DBSIZE" => Value::Integer(db.dbsize().await),
        "FLUSHDB" => {
            db.flushdb().await;
            let _ = aof.append_command(args).await;
            Value::SimpleString("OK".to_string())
        }
        "MSET" => {
            if args.len() < 3 || (args.len() - 1) % 2 != 0 {
                Value::Error("wrong number of arguments for 'mset' command".to_string())
            } else {
                let pairs: Vec<(String, String)> = args[1..]
                    .chunks_exact(2)
                    .map(|c| (c[0].clone(), c[1].clone()))
                    .collect();
                db.mset(&pairs).await;
                let _ = aof.append_command(args).await;
                Value::SimpleString("OK".to_string())
            }
        }
        "MGET" => {
            if args.len() < 2 {
                Value::Error("wrong number of arguments for 'mget' command".to_string())
            } else {
                let values = db.mget(&args[1..]).await;
                let resp_items = values
                    .into_iter()
                    .map(|v| match v {
                        Some(val) => Value::BulkString(val),
                        None => Value::Null,
                    })
                    .collect();
                Value::Array(resp_items)
            }
        }
        _ => Value::Error(format!("unknown command '{}'", cmd)),
    }
}
