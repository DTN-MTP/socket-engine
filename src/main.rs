use std::env;
use std::io::{self, BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use socket_engine::endpoint::{Endpoint, EndpointProto};
use socket_engine::engine::Engine;
use socket_engine::event::EngineObserver;

fn format_endpoint(endpoint: &Endpoint) -> String {
    let addr = endpoint.endpoint.clone();
    match endpoint.proto {
        EndpointProto::Udp => format!("UDP:{}", addr),
        EndpointProto::Tcp => format!("TCP:{}", addr),
        EndpointProto::Bp => format!("BP:{}", addr),
    }
}

static WAITING_FOR_INPUT: AtomicBool = AtomicBool::new(false);

struct Obs;

impl EngineObserver for Obs {
    fn on_engine_event(&mut self, event: socket_engine::event::SocketEngineEvent) {
        // Clear current line if we're waiting for input
        if WAITING_FOR_INPUT.load(Ordering::Relaxed) {
            print!("\r\x1b[K"); // Clear current line
        }

        match event {
            socket_engine::event::SocketEngineEvent::Data(data_event) => match data_event {
                socket_engine::event::DataEvent::Received { data, from } => {
                    println!(
                        "[RECV] From {}: \"{}\"",
                        format_endpoint(&from),
                        String::from_utf8_lossy(&data).trim()
                    );
                }
                socket_engine::event::DataEvent::Sent {
                    message_id: _,
                    to,
                    bytes_sent,
                } => {
                    println!("[SENT] To {} ({} bytes)", format_endpoint(&to), bytes_sent);
                }
                socket_engine::event::DataEvent::Sending {
                    message_id,
                    to,
                    bytes,
                } => {
                    println!(
                        "[SENDING] To {} ({} bytes, token: {})",
                        to, bytes, message_id
                    );
                }
            },
            socket_engine::event::SocketEngineEvent::Connection(conn_event) => match conn_event {
                socket_engine::event::ConnectionEvent::ListenerStarted { endpoint } => {
                    println!("[INFO] Listener started on {}", format_endpoint(&endpoint));
                }
                socket_engine::event::ConnectionEvent::Established { remote } => {
                    println!(
                        "[INFO] Connection established with {}",
                        format_endpoint(&remote)
                    );
                }
                socket_engine::event::ConnectionEvent::Closed { remote } => {
                    if let Some(remote) = remote {
                        println!("[INFO] Connection closed with {}", format_endpoint(&remote));
                    } else {
                        println!("[INFO] Connection closed");
                    }
                }
            },
            socket_engine::event::SocketEngineEvent::Error(err_event) => match err_event {
                socket_engine::event::ErrorEvent::ConnectionFailed {
                    endpoint,
                    reason: _,
                    token,
                } => {
                    println!(
                        "[ERROR] Connection failed to {}: {}",
                        format_endpoint(&endpoint),
                        token
                    );
                }
                socket_engine::event::ErrorEvent::SendFailed {
                    endpoint,
                    token,
                    reason,
                } => {
                    println!(
                        "[ERROR] Send failed to {} for id {}: {}",
                        format_endpoint(&endpoint),
                        token,
                        reason
                    );
                }
                socket_engine::event::ErrorEvent::ReceiveFailed { endpoint, reason } => {
                    println!(
                        "[ERROR] Receive failed from {}: {}",
                        format_endpoint(&endpoint),
                        reason
                    );
                }
                socket_engine::event::ErrorEvent::SocketError { endpoint, reason } => {
                    println!(
                        "[ERROR] Socket error on {}: {}",
                        format_endpoint(&endpoint),
                        reason
                    );
                }
            },
        }

        // Redisplay prompt if we were waiting for input
        if WAITING_FOR_INPUT.load(Ordering::Relaxed) {
            print!("Enter message: ");
            io::stdout().flush().unwrap();
        }
    }
}

fn main() -> io::Result<()> {
    // --- 1) parse CLI argument
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <local-endpoint> <distant-endpoint>", args[0]);
        eprintln!(
            "Example: {} \"udp 127.0.0.1:8888\" \"udp 127.0.0.1:9999\"",
            args[0]
        );
        std::process::exit(1);
    }

    let local_endpoint = match Endpoint::from_str(&args[1]) {
        Ok(ep) => ep,
        Err(e) => {
            eprintln!("[ERROR] Invalid local endpoint `{}`: {}", args[1], e);
            std::process::exit(1);
        }
    };
    let distant_endpoint = match Endpoint::from_str(&args[2]) {
        Ok(ep) => ep,
        Err(e) => {
            eprintln!("[ERROR] Invalid distant endpoint `{}`: {}", args[2], e);
            std::process::exit(1);
        }
    };

    println!("Socket Engine Starting...");
    println!("Local endpoint:  {}", format_endpoint(&local_endpoint));
    println!("Remote endpoint: {}", format_endpoint(&distant_endpoint));
    println!("─────────────────────────────────────────");
    println!("Type 'quit' or 'exit' to stop the program");
    println!();

    // --- 2) create engine + observer
    let observer = Arc::new(Mutex::new(Obs));
    let mut engine = Engine::new();
    engine.add_observer(observer);
    let _ = engine.start_listener_async(local_endpoint.clone());

    // Give some time for the listener to start
    std::thread::sleep(std::time::Duration::from_millis(100));

    // --- 3) read lines from stdin
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut line = String::new();
    loop {
        WAITING_FOR_INPUT.store(true, Ordering::Relaxed);
        print!("Enter message: ");
        io::stdout().flush().unwrap(); // Force flush to show prompt immediately

        line.clear();
        let n = reader.read_line(&mut line)?;
        WAITING_FOR_INPUT.store(false, Ordering::Relaxed);

        if n == 0 {
            // EOF
            println!("Goodbye!");
            break;
        }
        // strip trailing newline
        let text = line.trim_end().to_string();

        if text.is_empty() {
            continue; // Skip empty messages
        }

        if text == "quit" || text == "exit" {
            println!("Goodbye!");
            break;
        }

        // --- 4) wrap in ProtoMessage + send
        if let Err(err) = engine.send_async(
            local_endpoint.clone(),
            distant_endpoint.clone(),
            text.into_bytes(),
            "msg".to_string(),
        ) {
            eprintln!("[ERROR] Failed to send message: {}", err);
        }
    }

    Ok(())
}
