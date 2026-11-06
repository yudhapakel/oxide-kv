mod resp;
use resp::{Value, parse_command};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "127.0.0.1:6379";
    let listener = TcpListener::bind(addr).await?;
    println!("oxide-kv server listening on {}", addr);

    loop {
        let (socket, client_addr) = listener.accept().await?;
        println!("Client connected: {}", client_addr);

        tokio::spawn(async move {
            if let Err(e) = handle_client(socket).await {
                eprintln!(" Connection error: {:?}", e);
            }
        });
    }
}

async fn handle_client(mut socket: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
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
                    _ => Value::Error(format!("unknown command '{}'", cmd)),
                }
            }
            _ => Value::Error("unable to parse command".to_string()),
        };

        socket.write_all(&response.serialize()).await?;
    }
    Ok(())
}
