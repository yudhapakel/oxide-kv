# oxide-kv

[![Rust](https://img.shields.io/badge/Rust-1.75+-orange.svg?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square)](LICENSE)
[![Tokio](https://img.shields.io/badge/Runtime-Tokio-brightgreen.svg?style=flat-square)](https://tokio.rs/)
[![Protocol](https://img.shields.io/badge/Protocol-RESP-red.svg?style=flat-square)](https://redis.io/docs/reference/protocol-spec/)

A lightweight, concurrent, in-memory key-value storage engine implementing the Redis Serialization Protocol (RESP), built from scratch in Rust using the Tokio asynchronous runtime.

---

## Overview

`oxide-kv` is an educational systems programming project designed to explore low-level asynchronous networking, protocol parsing, thread-safe data structures, and persistent storage mechanisms. It aims to provide a functional subset of Redis commands while serving as a testbed for safe concurrency in Rust.

---

## Features

- **Asynchronous I/O:** Powered by `tokio` for non-blocking TCP network handling and efficient task scheduling.
- **RESP Protocol Implementation:** Custom serializer and parser supporting standard Redis serialization types (Simple Strings, Errors, Bulk Strings, Arrays, and Null values).
- **Client Compatibility:** Interoperable with standard clients such as `redis-cli`, network debuggers (`nc`/`telnet`), and Redis language SDKs.
- **Zero Heavy Abstractions:** Low-overhead protocol parsing operating directly on raw byte buffers.

---

## Architecture

```text
+-----------------------+
|  Client (redis-cli)   |
+-----------+-----------+
            | (TCP Port 6379)
            v
+-----------------------+
|   tokio::net::Tcp     |  <-- Concurrent Connection Acceptance
+-----------+-----------+
            | (Raw Bytes)
            v
+-----------------------+
|     RESP Parser       |  <-- Deserializes RESP Array / Inline Commands
+-----------+-----------+
            | (Value Enum)
            v
+-----------------------+
|    Command Handler    |  <-- Executes In-Memory Operations
+-----------+-----------+
            | (Response)
            v
+-----------------------+
|    RESP Serializer    |  <-- Writes bytes (+PONG\r\n, $len, etc.)
+-----------------------+
```

---

## Supported Commands

- [x] `PING` (inline and RESP bulk format, optional custom message)
- [ ] `SET <key> <value>`
- [ ] `GET <key>`
- [ ] `DEL <key>`
- [ ] `EXISTS <key>`
- [ ] `EXPIRE <key> <seconds>`

---

## Getting Started

### Prerequisites

- Rust toolchain (Cargo & rustc 1.75 or newer)

### Building and Running

1. Clone the repository:
   ```bash
   git clone https://github.com/yudhapakel/oxide-kv.git
   cd oxide-kv
   ```

2. Compile and run the server:
   ```bash
   cargo run --release
   ```
   The server will start listening on `127.0.0.1:6379`.

3. Test the connection:
   Using `redis-cli`:
   ```bash
   redis-cli -p 6379 PING
   # Output: PONG

   redis-cli -p 6379 PING "hello world"
   # Output: "hello world"
   ```

   Using netcat:
   ```bash
   nc 127.0.0.1 6379
   PING
   # Output: +PONG
   ```

---

## Roadmap

- [ ] In-memory thread-safe state storage (`Arc<RwLock<HashMap<String, Bytes>>>`)
- [ ] Key expiration and TTL background cleaner
- [ ] Append-Only File (AOF) persistence logging
- [ ] Performance benchmarking using standard redis-benchmark suites

---

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
