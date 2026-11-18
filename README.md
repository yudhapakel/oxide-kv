# oxide-kv

[![Rust](https://img.shields.io/badge/Rust-1.75+-orange.svg?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square)](LICENSE)
[![Tokio](https://img.shields.io/badge/Runtime-Tokio-brightgreen.svg?style=flat-square)](https://tokio.rs/)
[![Protocol](https://img.shields.io/badge/Protocol-RESP-red.svg?style=flat-square)](https://redis.io/docs/reference/protocol-spec/)

In-memory key-value data store implementing the Redis Serialization Protocol (RESP), built from scratch in Rust using Tokio.

---

## Overview

`oxide-kv` is an asynchronous key-value storage engine built to study networking, protocol parsing, and storage persistence in Rust. It implements a core subset of Redis commands with non-blocking I/O, concurrent read access via `tokio::sync::RwLock`, atomic arithmetic, active/passive key expiration, and Append-Only File (AOF) persistence.

---

## Features

- **Asynchronous TCP Server:** Non-blocking connection handling with Tokio tasks.
- **Streaming RESP Parser:** Byte-buffer framing using `BytesMut` supporting pipelined requests, inline strings, bulk strings, integers, arrays, and error frames.
- **Concurrent Storage Engine:** Thread-safe state backed by `tokio::sync::RwLock<HashMap<String, Entry>>` for parallel reads and serialized writes.
- **Atomic Operations:** 64-bit integer arithmetic with `INCR` and `DECR`.
- **Key Expiration & Active Reaper:** Lazy expiration on read access combined with a periodic background task (500ms interval) to sweep expired keys.
- **AOF Persistence:** Mutation logging to `appendonly.aof` with automatic state replay during startup.
- **Multi-Key Operations:** Batch operations (`MGET`, `MSET`), pattern matching (`KEYS *`), and administrative commands (`DBSIZE`, `FLUSHDB`).
- **Redis Client Interoperability:** Compatible with `redis-cli`, standard Redis client libraries, and plain TCP sockets (`nc`, `telnet`).

---

## Architecture

```text
+-----------------------+
|  Client (redis-cli)   |
+-----------+-----------+
            | (TCP Port 6379)
            v
+-----------------------+
|   tokio::net::Tcp     |  <-- Asynchronous Connection Listener
+-----------+-----------+
            | (Raw Stream)
            v
+-----------------------+
|     RESP Parser       |  <-- Deserializes Bytes into RESP Frames
+-----------+-----------+
            | (Command Dispatch)
            v
+-----------------------+         +-----------------------+
|  Storage Engine (Db)  | <-----> | Active Reaper Task    |
|  - In-Memory State    |         | (Periodic 500ms Tick) |
+-----------+-----------+         +-----------------------+
            |
            | (Mutations logged)
            v
+-----------------------+
|    AOF Persistence    |  <-- appendonly.aof (Startup Replay & Sync)
+-----------+-----------+
            | (Execution Result)
            v
+-----------------------+
|    RESP Serializer    |  <-- Encodes to RESP Frame (+OK, :Int, $Bulk, *Arr)
+-----------------------+
```

---

## Supported Commands

| Command | Syntax | Description |
|---|---|---|
| **PING** | `PING [message]` | Checks server health or echoes back text. |
| **SET** | `SET <key> <value> [EX seconds]` | Sets string value for a key with optional TTL. |
| **GET** | `GET <key>` | Retrieves key value, returning nil if missing or expired. |
| **DEL** | `DEL <key> ...` | Deletes specified keys; returns deleted count. |
| **EXISTS** | `EXISTS <key> ...` | Returns count of existing, active keys. |
| **INCR** | `INCR <key>` | Atomically increments 64-bit signed integer value by 1. |
| **DECR** | `DECR <key>` | Atomically decrements 64-bit signed integer value by 1. |
| **EXPIRE** | `EXPIRE <key> <seconds>` | Sets timeout on key in seconds. |
| **PERSIST** | `PERSIST <key>` | Removes TTL timeout from key. |
| **TTL** | `TTL <key>` | Returns remaining seconds (-1: persistent, -2: not found). |
| **KEYS** | `KEYS <pattern>` | Matches active keys using glob syntax (`*`, `?`). |
| **MSET** | `MSET <k1> <v1> ...` | Sets multiple key-value pairs in a single operation. |
| **MGET** | `MGET <k1> <k2> ...` | Retrieves values for multiple keys in an array. |
| **DBSIZE** | `DBSIZE` | Returns total number of active keys. |
| **FLUSHDB** | `FLUSHDB` | Removes all keys from storage. |

---

## Getting Started

### Prerequisites

- Rust toolchain (`cargo` & `rustc` 1.75+)
- (Optional) `redis-cli` for testing client interactions

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
   The server binds to `127.0.0.1:6379`.

3. Test with `redis-cli`:
   ```bash
   # Ping
   redis-cli -p 6379 PING
   # Output: PONG

   # Key-Value
   redis-cli -p 6379 SET user:1 "yudha"
   redis-cli -p 6379 GET user:1
   # Output: "yudha"

   # Atomic Counter
   redis-cli -p 6379 SET counter 10
   redis-cli -p 6379 INCR counter
   # Output: (integer) 11

   # Expiration & Inspection
   redis-cli -p 6379 SET session:abc "active" EX 60
   redis-cli -p 6379 TTL session:abc
   redis-cli -p 6379 PERSIST session:abc
   redis-cli -p 6379 KEYS "session:*"

   # Batch Operations
   redis-cli -p 6379 MSET a 1 b 2 c 3
   redis-cli -p 6379 MGET a b c
   redis-cli -p 6379 DBSIZE
   ```

---

## Roadmap

- [x] In-memory thread-safe state storage (`tokio::sync::RwLock<HashMap<String, Entry>>`)
- [x] Atomic integer operations (`INCR`, `DECR`)
- [x] Key expiration and TTL with passive/lazy cleanup
- [x] Active background TTL cleaner task for stale keys
- [x] Append-Only File (AOF) persistence logging and startup replay
- [x] Bulk operations (`MSET`, `MGET`) and administrative commands (`DBSIZE`, `FLUSHDB`)
- [x] Streaming RESP framing (`BytesMut`), bugfixes, and unit test suite (`cargo test`)
- [x] `SET ... EX`, `PERSIST`, and glob `KEYS` support
- [ ] Active key eviction policies (LRU/LFU memory limits)
- [ ] Benchmark suite with `redis-benchmark`

---

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.
