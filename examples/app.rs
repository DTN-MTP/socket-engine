use clap::Parser;
use std::io::{self, BufRead, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use socket_engine::endpoint::Endpoint;
use socket_engine::engine::Engine;
use socket_engine::event::{ConnectionEvent, DataEvent, ErrorEvent, SocketEngineEvent};
use socket_engine::event::EngineObserver;

#[derive(Parser)]
#[command(name = "socket-engine-app")]
#[command(about = "Application interactive pour tester le socket engine")]
#[command(version)]
struct Cli {
    /// Protocole à utiliser (udp, tcp, bp)
    #[arg(short, long)]
    protocol: String,
    
    /// Port local pour écouter (pour UDP/TCP)
    #[arg(short = 'l', long)]
    local_port: Option<u16>,
    
    /// Port distant pour envoyer (pour UDP/TCP)
    #[arg(short = 'd', long)]
    dist_port: Option<u16>,
    
    /// Adresse IP (par défaut 127.0.0.1, pour UDP/TCP)
    #[arg(short, long, default_value = "127.0.0.1")]
    ip: String,
    
    /// Adresse locale (pour Bundle Protocol)
    #[arg(short = 'L', long)]
    local_addr: Option<String>,
    
    /// Adresse distante (pour Bundle Protocol)
    #[arg(short = 'D', long)]
    dist_addr: Option<String>,
}

fn format_endpoint(endpoint: &Endpoint) -> String {
    match endpoint {
        Endpoint::Udp(addr) => format!("UDP:{}", addr),
        Endpoint::Tcp(addr) => format!("TCP:{}", addr),
        Endpoint::Bp(addr) => format!("BP:{}", addr),
    }
}

static WAITING_FOR_INPUT: AtomicBool = AtomicBool::new(false);
static MESSAGE_COUNTER: AtomicU64 = AtomicU64::new(1);

struct SocketObserver;

impl EngineObserver for SocketObserver {
    fn on_engine_event(&mut self, event: SocketEngineEvent) {
        if WAITING_FOR_INPUT.load(Ordering::Relaxed) {
            print!("\r\x1b[K"); // Clear current line
        }

        match event {
            SocketEngineEvent::Data(data_event) => match data_event {
                DataEvent::Received { data, from } => {
                    println!(
                        "[RECV] From {}: \"{}\"",
                        format_endpoint(&from),
                        String::from_utf8_lossy(&data).trim()
                    );
                }
                DataEvent::Sent {
                    message_id: _,
                    to,
                    bytes_sent,
                } => {
                    println!("[SENT] To {} ({} bytes)", format_endpoint(&to), bytes_sent);
                }
                DataEvent::Sending { message_id, to, bytes } => {
                    println!("[SENDING] To {} ({} bytes, token: {})", to, bytes, message_id);
                },
            },
            SocketEngineEvent::Connection(conn_event) => match conn_event {
                ConnectionEvent::ListenerStarted { endpoint } => {
                    println!("[INFO] Listener started on {}", format_endpoint(&endpoint));
                }
                ConnectionEvent::Established { remote } => {
                    println!(
                        "[INFO] Connection established with {}",
                        format_endpoint(&remote)
                    );
                }
                ConnectionEvent::Closed { remote } => {
                    if let Some(remote) = remote {
                        println!("[INFO] Connection closed with {}", format_endpoint(&remote));
                    } else {
                        println!("[INFO] Connection closed");
                    }
                }
            },
            SocketEngineEvent::Error(err_event) => match err_event {
                ErrorEvent::ConnectionFailed {
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
                ErrorEvent::SendFailed {
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
                ErrorEvent::ReceiveFailed { endpoint, reason } => {
                    println!(
                        "[ERROR] Receive failed from {}: {}",
                        format_endpoint(&endpoint),
                        reason
                    );
                }
                ErrorEvent::SocketError { endpoint, reason } => {
                    println!(
                        "[ERROR] Socket error on {}: {}",
                        format_endpoint(&endpoint),
                        reason
                    );
                }
            },
        }

        if WAITING_FOR_INPUT.load(Ordering::Relaxed) {
            print!("Enter message: ");
            io::stdout().flush().unwrap();
        }
    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let cli = Cli::parse();
    
    println!("Socket Engine Interactive Test Application");
    println!("============================================");
    
    let (local_endpoint, distant_endpoint) = match cli.protocol.to_lowercase().as_str() {
        "udp" => {
            let local_port = cli.local_port.ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "local-port requis pour UDP")
            })?;
            let dist_port = cli.dist_port.ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "dist-port requis pour UDP")
            })?;
            
            let local_endpoint_str = format!("udp {}:{}", cli.ip, local_port);
            let distant_endpoint_str = format!("udp {}:{}", cli.ip, dist_port);
            
            (local_endpoint_str, distant_endpoint_str)
        },
        "tcp" => {
            let local_port = cli.local_port.ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "local-port requis pour TCP")
            })?;
            let dist_port = cli.dist_port.ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "dist-port requis pour TCP")
            })?;
            
            let local_endpoint_str = format!("tcp {}:{}", cli.ip, local_port);
            let distant_endpoint_str = format!("tcp {}:{}", cli.ip, dist_port);
            
            (local_endpoint_str, distant_endpoint_str)
        },
        "bp" => {
            let local_addr = cli.local_addr.ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "local-addr requis pour Bundle Protocol")
            })?;
            let dist_addr = cli.dist_addr.ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "dist-addr requis pour Bundle Protocol")
            })?;
            
            let local_endpoint_str = format!("bp {}", local_addr);
            let distant_endpoint_str = format!("bp {}", dist_addr);
            
            (local_endpoint_str, distant_endpoint_str)
        },
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Protocole non supporté: {}. Utilisez: udp, tcp ou bp", cli.protocol)
            ));
        }
    };

    let local_endpoint = match Endpoint::from_str(&local_endpoint) {
        Ok(ep) => ep,
        Err(e) => {
            eprintln!("[ERROR] Invalid local endpoint `{}`: {}", local_endpoint, e);
            std::process::exit(1);
        }
    };
    let distant_endpoint = match Endpoint::from_str(&distant_endpoint) {
        Ok(ep) => ep,
        Err(e) => {
            eprintln!("[ERROR] Invalid distant endpoint `{}`: {}", distant_endpoint, e);
            std::process::exit(1);
        }
    };

    println!("Local endpoint:  {}", format_endpoint(&local_endpoint));
    println!("Remote endpoint: {}", format_endpoint(&distant_endpoint));
    println!("─────────────────────────────────────────");
    println!("Type messages to send, 'quit' or 'exit' to stop");
    println!();

    // --- 2) Crée le moteur et ajoute l'observateur
    let observer = Arc::new(Mutex::new(SocketObserver));
    let mut engine = Engine::new();
    engine.add_observer(observer);
    
    // Démarre le listener sur l'endpoint local
    engine.start_listener_async(local_endpoint);

    // Laisse un peu de temps au listener pour démarrer
    std::thread::sleep(std::time::Duration::from_millis(100));

    // --- 3) Boucle de lecture des messages depuis stdin
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut line = String::new();
    
    loop {
        WAITING_FOR_INPUT.store(true, Ordering::Relaxed);
        print!("Enter message: ");
        io::stdout().flush().unwrap(); // Force l'affichage immédiat du prompt

        line.clear();
        let n = reader.read_line(&mut line)?;
        WAITING_FOR_INPUT.store(false, Ordering::Relaxed);

        if n == 0 {
            // EOF (Ctrl+D)
            println!("Goodbye!");
            break;
        }
        
        // Supprime le caractère de fin de ligne
        let text = line.trim_end().to_string();

        if text.is_empty() {
            continue; // Ignore les messages vides
        }

        if text == "quit" || text == "exit" {
            println!("Goodbye!");
            break;
        }

        // --- 4) Envoie le message vers l'endpoint distant
        let msg_id = MESSAGE_COUNTER.fetch_add(1, Ordering::Relaxed);
        if let Err(err) = engine.send_async_runtime(
            distant_endpoint.clone(),
            text.into_bytes(),
            format!("msg-{}", msg_id),
        ) {
            eprintln!("[ERROR] Failed to send message: {}", err);
        }
    }

    Ok(())
}
