use std::{
    io::{self, Read},
    sync::{Arc, Mutex},
    thread,
};

use libc::c_int;

use socket2::{Domain, Protocol, SockAddr, Socket, Type};

use crate::{
    endpoint::{create_bp_sockaddr_with_string, Endpoint},
    engine::{Engine, TOKIO_RUNTIME},
    event::{notify_all, EngineObserver, SocketEngineEvent},
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
        observers: Vec<Arc<Mutex<dyn EngineObserver + Send + Sync>>>,
    ) -> io::Result<()> {
        if self.listening {
            return Ok(());
        }

        self.listening = true;

        self.socket.set_nonblocking(true)?;
        self.socket.set_reuse_address(true)?;
        self.socket.bind(&SockAddr::from(self.sockaddr.clone()))?;

        match &self.endpoint {
            Endpoint::Udp(_addr) | Endpoint::Bp(_addr) => {
                TOKIO_RUNTIME.spawn_blocking({
                    let mut socket = self.socket.try_clone()?;
                    let observers_cloned = observers.clone();
                    move || loop {
                        let mut buffer: [u8; 65507] = [0; 65507];

                        match socket.read(&mut buffer) {
                            Ok(size) => {
                                // Convert to Vec<u8> for consistency
                                let data = buffer[..size].to_vec();
                                notify_all(&observers_cloned, &SocketEngineEvent::Reception(data))
                            }
                            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                thread::sleep(std::time::Duration::from_millis(10));
                            }
                            Err(_e) => {
                                todo!();
                            }
                        }
                    }
                });
            }

            Endpoint::Tcp(_addr) => {
                self.socket.listen(128)?;
                TOKIO_RUNTIME.spawn_blocking({
                    let socket = self.socket.try_clone()?;
                    move || loop {
                        match socket.accept() {
                            Ok((stream, _peer)) => {
                                let observers_cloned = observers.clone();
                                TOKIO_RUNTIME.spawn(async move {
                                    handle_tcp_connection(stream.into(), &observers_cloned).await;
                                });
                            }
                            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                thread::sleep(std::time::Duration::from_millis(10));
                            }
                            Err(_e) => {
                                todo!();
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
    observers: &Vec<Arc<Mutex<dyn EngineObserver + Send + Sync>>>,
) {
    let mut full_data = Vec::new();
    let mut buffer = [0; 1024];

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => {
                break;
            }
            Ok(size) => {
                full_data.extend_from_slice(&buffer[..size]);
            }
            Err(_e) => {
                todo!();
            }
        }
    }

    if !full_data.is_empty() {
        notify_all(observers, &SocketEngineEvent::Reception(full_data));
    }
}
