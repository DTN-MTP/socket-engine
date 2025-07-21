s# Socket Engine Examples

Interactive test application to demonstrate the socket engine capabilities.

## Usage

### UDP (Connectionless)

```bash
# Terminal 1 - Listen on 8888, send to 9999
cargo run --example app -- -p udp -l 8888 -d 9999

# Terminal 2 - Listen on 9999, send to 8888
cargo run --example app -- -p udp -l 9999 -d 8888
```

### TCP (Connection-based)

```bash
# Terminal 1
cargo run --example app -- -p tcp -l 8888 -d 9999

# Terminal 2
cargo run --example app -- -p tcp -l 9999 -d 8888
```

### Bundle Protocol (DTN)

```bash
# Terminal 1 - Node 1
cargo run --example app -- -p bp -L ipn:1.1 -D ipn:2.1

# Terminal 2 - Node 2
cargo run --example app -- -p bp -L ipn:2.1 -D ipn:1.1
```

### Advanced Options

```bash
# Custom IP address
cargo run --example app -- -p udp -l 8888 -d 9999 -i 192.168.1.100

# Long form (equivalent to short form above)
cargo run --example app -- --protocol udp --local-port 8888 --dist-port 9999

# Show help
cargo run --example app -- --help
```

## How it Works

1. Start the application in two terminals with different endpoints
2. Type messages in one terminal
3. Messages will appear in the other terminal
4. Type `quit` or `exit` to stop

## Example Session

```
Socket Engine Interactive Test Application
============================================
Local endpoint:  UDP:127.0.0.1:8888
Remote endpoint: UDP:127.0.0.1:9999
─────────────────────────────────────────
Type messages to send, 'quit' or 'exit' to stop

[INFO] Listener started on UDP:127.0.0.1:8888
Enter message: Hello World!
[SENDING] To udp 127.0.0.1:9999 (12 bytes, token: msg-1)
[SENT] To UDP:127.0.0.1:9999 (12 bytes)
Enter message: [RECV] From UDP:127.0.0.1:9999: "Hello back!"
Enter message: quit
Goodbye!
```
