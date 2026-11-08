mod aof;
mod db;
mod resp;
use aof::Aof;
use db::Db;
use resp::{Value, parse_command};
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
                eprintln!(" Connection error: {:?}", e);
            }
        });
    }
}

async fn handle_client(
    mut socket: TcpStream,
    db: Db,
    aof: Aof,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut buffer = [0; 512];

    loop {
        let bytes_read = socket.read(&mut buffer).await?;
        if bytes_read == 0 {
            break;
        }

        let response = match parse_command(&buffer[..bytes_read]) {
            Ok(args) if !args.is_empty() => {
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
                        if args.len() < 3 {
                            Value::Error(
                                ("wrong number of arguments for 'set' command".to_string()),
                            )
                        } else {
                            let key = args[1].clone();
                            let value = args[2].clone();
                            db.set(key, value).await;
                            let _ = aof.append_command(&args).await;
                            Value::SimpleString("OK".to_string())
                        }
                    }
                    "GET" => {
                        if args.len() < 2 {
                            Value::Error(
                                ("wrong number of arguments for 'get' command".to_string()),
                            )
                        } else {
                            let key = &args[1];
                            match db.get(key).await {
                                Some(val) => Value::BulkString((val)),
                                None => Value::Null,
                            }
                        }
                    }
                    "DEL" => {
                        if args.len() < 2 {
                            Value::Error(
                                ("wrong number of arguments for 'del' command".to_string()),
                            )
                        } else {
                            let keys = &args[1..];
                            let deleted = db.del(keys).await;
                            if deleted > 0 {
                                let _ = aof.append_command(&args).await;
                            }
                            Value::Integer(deleted)
                        }
                    }
                    "EXISTS" => {
                        if args.len() < 2 {
                            Value::Error(
                                ("wrong number of arguments for 'exists' command".to_string()),
                            )
                        } else {
                            let keys = &args[1..];
                            let count = db.exists(keys).await;
                            Value::Integer((count))
                        }
                    }
                    "INCR" => {
                        if args.len() != 2 {
                            Value::Error(
                                ("wrong number of arguments for 'incr' command".to_string()),
                            )
                        } else {
                            let key = args[1].clone();
                            match db.incr(key).await {
                                Ok(val) => {
                                    let _ = aof.append_command(&args).await;
                                    Value::Integer(val)
                                }
                                Err(err) => Value::Error(err.to_string()),
                            }
                        }
                    }
                    "DECR" => {
                        if args.len() != 2 {
                            Value::Error(
                                ("wrong number of arguments for 'decr' command".to_string()),
                            )
                        } else {
                            let key = args[1].clone();
                            match db.decr(key).await {
                                Ok(val) => {
                                    let _ = aof.append_command(&args).await;
                                    Value::Integer(val)
                                }
                                Err(err) => Value::Error(err.to_string()),
                            }
                        }
                    }
                    "EXPIRE" => {
                        if args.len() != 3 {
                            Value::Error(("wrong arguments of for  'expire' command".to_string()))
                        } else {
                            let key = &args[1];
                            match args[2].parse::<u64>() {
                                Ok(sec) => {
                                    let res = db.expire(key, sec).await;
                                    if res == 1 {
                                        let _ = aof.append_command(&args).await;
                                    }
                                    Value::Integer(res)
                                }
                                Err(_) => Value::Error(
                                    "value is not an integer or out of range".to_string(),
                                ),
                            }
                        }
                    }
                    "TTL" => {
                        if args.len() != 2 {
                            Value::Error(
                                ("wrong number of arguments for 'ttl' command".to_string()),
                            )
                        } else {
                            let key = &args[1];
                            Value::Integer((db.ttl(key).await))
                        }
                    }
                    "DBSIZE" => Value::Integer(db.dbsize().await),
                    "FLUSHDB" => {
                        db.flushdb().await;
                        let _ = aof.append_command(&args).await;
                        Value::SimpleString(("OK".to_string()))
                    }
                    "MSET" => {
                        if args.len() < 3 || (args.len() - 1) % 2 != 0 {
                            Value::Error("wrong number of arguments for 'mset' command".to_string())
                        } else {
                            let mut pairs = Vec::new();
                            let mut i = 1;
                            while i < args.len() {
                                pairs.push((args[i].clone(), args[i + 1].clone()));
                                i += 2;
                            }
                            db.mset(&pairs).await;
                            let _ = aof.append_command(&args).await;
                            Value::SimpleString("OK".to_string())
                        }
                    }
                    "MGET" => {
                        if args.len() < 2 {
                            Value::Error(
                                ("wrong number of arguments for 'mget' command".to_string()),
                            )
                        } else {
                            let keys = &args[1..];
                            let values = db.mget(keys).await;
                            let resp_items = values
                                .into_iter()
                                .map(|v| match v {
                                    Some(val) => Value::BulkString((val)),
                                    None => Value::Null,
                                })
                                .collect();
                            Value::Array((resp_items))
                        }
                    }
                    _ => Value::Error(format!("unknown command '{}'", cmd)),
                }
            }
            _ => Value::Error("unable to parse command".to_string()),
        };

        socket.write_all(&response.serialize()).await?;
    }
    Ok(())
}
