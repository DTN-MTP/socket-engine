# Socket Engine

A flexible Rust library for multi-protocol socket operations supporting UDP, TCP, and Bundle Protocol (DTN) with event-driven architecture.

## Features

- **Multi-protocol support**: UDP, TCP, and Bundle Protocol
- **Event-driven architecture**: Observer pattern for flexible event handling
- **Async/await support**: Built on Tokio for high performance
- **Easy to use**: Simple API for common networking tasks

## Quick Start

### Interactive Test Application

```bash
# UDP - Simple connectionless protocol
cargo run --example app -- -p udp -l 8888 -d 9999

# TCP - Reliable connection-based protocol  
cargo run --example app -- -p tcp -l 8888 -d 9999

# Bundle Protocol - DTN for delay-tolerant networks
cargo run --example app -- -p bp -L ipn:1.1 -D ipn:2.1

# Custom IP address
cargo run --example app -- -p udp -l 8888 -d 9999 -i 192.168.1.100

# Show help
cargo run --example app -- --help
```

### Library Usage

```rust
use socket_engine::{Engine, endpoint::Endpoint, event::EngineObserver};

// Create engine
let mut engine = Engine::new();

// Add observer
engine.add_observer(your_observer);

// Start listening
let endpoint = Endpoint::from_str("udp 127.0.0.1:8888")?;
engine.start_listener_async(endpoint);

// Send data
let target = Endpoint::from_str("udp 127.0.0.1:9999")?;
engine.send_async_runtime(target, b"Hello".to_vec(), "msg-1".to_string())?;
```

## Supported Protocols

- **UDP**: Fast, connectionless, unreliable
- **TCP**: Reliable, connection-based, ordered
- **Bundle Protocol (BP)**: DTN for delay/disruption-tolerant networks

## CLI Options

| Long form | Short | Description |
|-----------|-------|-------------|
| `--protocol` | `-p` | Protocol (udp, tcp, bp) |
| `--local-port` | `-l` | Local port to listen on |
| `--dist-port` | `-d` | Remote port to send to |
| `--ip` | `-i` | IP address (default: 127.0.0.1) |
| `--local-addr` | `-L` | Local address for Bundle Protocol |
| `--dist-addr` | `-D` | Remote address for Bundle Protocol |