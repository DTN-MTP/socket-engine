use std::{
    io::{self, Read},
    sync::{Arc, Mutex},
    thread,
};

use libc::c_int;

use crate::{encoding::{decode_proto_message_from_bytes}, proto::ProtoMessage};
use socket2::{Domain, Protocol, SockAddr, Socket, Type};

use crate::{
    endpoint::{create_bp_sockaddr_with_string, Endpoint},
    engine::{EngineObserver, TOKIO_RUNTIME},
};
pub const AF_BP: c_int = 28;

pub struct GenericSocket {
    pub socket: Socket,
    pub endpoint: Endpoint,
    pub sockaddr: SockAddr,
    pub listening: bool,
}

impl GenericSocket {
    pub fn new(eid: Endpoint) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let (domain, semtype, proto, address): (Domain, Type, Protocol, SockAddr) = match &eid {
            Endpoint::Udp(addr) => {
                let std_sock = addr.parse()?;
                (
                    Domain::for_address(std_sock),
                    Type::DGRAM,
                    Protocol::UDP,
                    SockAddr::from(std_sock),
                )
            }
            Endpoint::Tcp(addr) => {
                let std_sock = addr.parse()?;
                (
                    Domain::for_address(std_sock),
                    Type::STREAM,
                    Protocol::TCP,
                    SockAddr::from(std_sock),
                )
            }
            Endpoint::Bp(addr) => (
                Domain::from(AF_BP),
                Type::DGRAM,
                Protocol::UDP,
                create_bp_sockaddr_with_string(&addr)?,
            ),
        };

        let socket = Socket::new(domain, semtype, Some(proto))?;
        return Ok(Self {
            socket: socket,
            endpoint: eid,
            sockaddr: address,
            listening: false,
        });
    }

    pub fn start_listener(
        &mut self,
        observer: Arc<Mutex<dyn EngineObserver + Send + Sync>>,
    ) -> io::Result<()> {
        if self.listening {
            return Ok(());
        }

        self.listening = true;

        self.socket.set_nonblocking(true)?;
        self.socket.set_reuse_address(true)?;
        self.socket.bind(&SockAddr::from(self.sockaddr.clone()))?;

        match &self.endpoint {
            Endpoint::Udp(addr) | Endpoint::Bp(addr) => {
                let address = addr.clone();
                TOKIO_RUNTIME.spawn_blocking({
                    let mut socket = self.socket.try_clone()?; // Clone the socket for the async thread
                    move || loop {
                        let mut buffer: [u8; 1024] = [0; 1024];
                        match socket.read(&mut buffer) {
                            Ok(size) => {
                                println!(
                                    "UDP/BP received {} bytes on listening address {}",
                                    size, address
                                );
                                let observer_clone = observer.clone();
                                TOKIO_RUNTIME.spawn(async move {
                                    let decoded = decode_proto_message_from_bytes(&buffer[0..size]).unwrap();
                                    observer_clone.lock().unwrap().get_notification(decoded);
                                });
                            }
                            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                thread::sleep(std::time::Duration::from_millis(10));
                            }
                            Err(e) => {
                                eprintln!("UDP/BP Error: {}", e);
                                break;
                            }
                        }
                    }
                });
            }
            Endpoint::Tcp(addr) => {
                self.socket.listen(128)?;
                let address = addr.clone();
                TOKIO_RUNTIME.spawn_blocking({
                    let socket = self.socket.try_clone()?;
                    move || loop {
                        match socket.accept() {
                            Ok((stream, _peer)) => {
                                println!("TCP connection {} listening address", address);
                                let observer_for_stream = observer.clone();
                                TOKIO_RUNTIME.spawn(async move {
                                    handle_tcp_connection(stream.into(), observer_for_stream).await;
                                });
                            }
                            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                thread::sleep(std::time::Duration::from_millis(10));
                            }
                            Err(e) => {
                                eprintln!("TCP Error: {}", e);
                                break;
                            }
                        }
                    }
                });
            }
        }
        Ok(())
    }
}

async fn handle_tcp_connection(
    mut stream: std::net::TcpStream,
    observer: Arc<Mutex<dyn EngineObserver + Send + Sync>>,
) {
    let mut buffer = [0; 1024];
    match stream.read(&mut buffer) {
        Ok(size) => {
            println!("TCP received {} bytes", size);
            let decoded = decode_proto_message_from_bytes(&buffer[0..size]).unwrap();
            observer.lock().unwrap().get_notification(decoded);
        }
        Err(e) => {
            eprintln!("TCP Read Error: {}", e);
        }
    }
}
