# Socket Engine

## Overview

This repository provides a flexible socket engine in Rust, supporting UDP, TCP, and custom BP (Backplane) protocols. It abstracts socket operations and event handling, making it easy to build networked applications with event notification.

---

### Engine Interface

The `Engine` struct is the main entry point for interacting with the socket engine. It manages a list of observers and provides methods to:

- Add observers (`add_observer`)
- Start listening for incoming data on a given endpoint (`start_listener_async`)
- Send data asynchronously to a specified endpoint (`send_async`)

---

### Observer Pattern

The repository implements the observer pattern to decouple event producers (sockets) from event consumers (observers). Observers implement the `EngineObserver` trait and can be registered with the engine. When network events occur (such as data received, connection established, errors, etc.), all registered observers are notified via the `notify_all_observers` function.

This design allows for flexible event handling, enabling multiple components to react to network events independently.

---

### Usage

To use the socket engine, you can run the provided example with either UDP or TCP protocols. The command line arguments specify the protocol and endpoints for listening and sending data.

```sh
# --- UDP Usage ---
cargo run -- "udp 127.0.0.1:8888" "udp 127.0.0.1:9999" # Peer 1
cargo run -- "udp 127.0.0.1:9999" "udp 127.0.0.1:8888" # Peer 2
# --- TCP Usage ---
cargo run -- "tcp 127.0.0.1:9999" "tcp 127.0.0.1:8888" # Peer 1
cargo run -- "tcp 127.0.0.1:8888" "tcp 127.0.0.1:9999" # Peer 2
```