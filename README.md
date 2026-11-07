# oxide-kv

[![Rust](https://img.shields.io/badge/Rust-1.75+-orange.svg?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square)](LICENSE)
[![Tokio](https://img.shields.io/badge/Runtime-Tokio-brightgreen.svg?style=flat-square)](https://tokio.rs/)
[![Protocol](https://img.shields.io/badge/Protocol-RESP-red.svg?style=flat-square)](https://redis.io/docs/reference/protocol-spec/)

A lightweight, concurrent, in-memory key-value storage engine implementing the Redis Serialization Protocol (RESP), built from scratch in Rust using the Tokio asynchronous runtime.

---

## Overview

`oxide-kv` is an educational systems programming project designed to explore low-level asynchronous networking, protocol parsing, thread-safe data structures, and in-memory storage mechanisms. It implements a core subset of Redis commands, featuring non-blocking I/O, concurrent read-write access via `tokio::sync::RwLock`, atomic arithmetic mutations, and passive key expiration.

---

## Features

- **Asynchronous Network Engine:** Built on `tokio` for efficient, non-blocking TCP connection handling and task scheduling.
- **RESP Protocol Parser:** Zero-copy byte buffer parser supporting Redis Serialization Protocol primitives (Simple Strings, Errors, Integers, Bulk Strings, and Arrays).
- **Concurrent Storage:** Thread-safe state managed by `tokio::sync::RwLock<HashMap<String, Entry>>`, enabling safe concurrent reads and serialized writes.
- **Atomic Operations:** Integer arithmetic (`INCR` and `DECR`) with range validation and RESP-compliant integer replies.
- **Passive Key Expiration:** Time-To-Live (TTL) tracking with lazy eviction during key access, minimizing CPU overhead while preventing stale reads.
- **Client Compatibility:** Fully interoperable with standard Redis clients such as `redis-cli`, network utilities (`nc`, `telnet`), and official Redis SDKs.

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
|     RESP Parser       |  <-- Deserializes Bytes to RESP Value Enum
+-----------+-----------+
            | (Command Dispatch)
            v
+-----------------------+
|  Storage Engine (Db)  |  <-- tokio::sync::RwLock<HashMap<String, Entry>>
|  - Passive Expiration |  <-- Validates expires_at (Instant)
+-----------+-----------+
            | (Execution Result)
            v
+-----------------------+
|    RESP Serializer    |  <-- Encodes to RESP Frame (+OK, :Int, $Bulk, etc.)
+-----------------------+
```

---

## Supported Commands

| Command | Syntax | Description |
|---|---|---|
| **PING** | `PING [message]` | Tests server liveness or echoes back an argument. |
| **SET** | `SET <key> <value>` | Sets string value for a key. |
| **GET** | `GET <key>` | Returns string value or nil if not found / expired. |
| **DEL** | `DEL <key> ...` | Deletes one or more keys; returns deleted count. |
| **EXISTS** | `EXISTS <key> ...` | Returns count of existing, unexpired keys. |
| **INCR** | `INCR <key>` | Atomically increments 64-bit signed integer value by 1. |
| **DECR** | `DECR <key>` | Atomically decrements 64-bit signed integer value by 1. |
| **EXPIRE** | `EXPIRE <key> <seconds>` | Sets timeout on a key in seconds. |
| **TTL** | `TTL <key>` | Returns remaining seconds until expiration (-1: persistent, -2: not found). |

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
   The server listens on `127.0.0.1:6379`.

3. Test with `redis-cli`:
   ```bash
   # Basic connectivity
   redis-cli -p 6379 PING
   # Output: PONG

   # Key-value operations
   redis-cli -p 6379 SET user:1 "yudha"
   # Output: OK

   redis-cli -p 6379 GET user:1
   # Output: "yudha"

   # Atomic counters
   redis-cli -p 6379 SET counter 10
   redis-cli -p 6379 INCR counter
   # Output: (integer) 11

   # Expiration and TTL
   redis-cli -p 6379 EXPIRE counter 60
   # Output: (integer) 1

   redis-cli -p 6379 TTL counter
   # Output: (integer) 58
   ```

---

## Roadmap

- [x] In-memory thread-safe state storage (`tokio::sync::RwLock<HashMap<String, Entry>>`)
- [x] Atomic integer operations (`INCR`, `DECR`)
- [x] Key expiration and TTL with passive/lazy cleanup
- [ ] Active background TTL cleaner task for stale keys
- [ ] Append-Only File (AOF) persistence logging
- [ ] Performance benchmarking using `redis-benchmark`

---

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
