###### socket-engine

A minimal socket engine for tcp/udp/bp coms

```
# With udp (localhost, listen on 8888 send to 9999)
cargo run -- "udp 127.0.0.1:8888" "udp 127.0.0.1:9999"
# Same for tcp
cargo run -- "tcp 127.0.0.1:8888" "tcp 127.0.0.1:9999"
# With bp (listen and send to service id 2, send from node 10 to node 30)
cargo run -- "bp ipn:10.2" "bp ipn:30.2"
```