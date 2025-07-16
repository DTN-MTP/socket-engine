use std::{
    io::{self, Read},
    sync::{Arc, Mutex},
    thread,
};

use libc::c_int;

use socket2::{Domain, Protocol, SockAddr, Socket, Type};

use crate::{
    endpoint::{create_bp_sockaddr_with_string, Endpoint},
    engine::TOKIO_RUNTIME,
    event::{
        notify_all_observers, ConnectionEvent, DataEvent, EngineObserver, ErrorEvent,
        SocketEngineEvent,
    },
};
pub const AF_BP: c_int = 28;

pub struct GenericSocket {
    pub socket: Socket,
    pub endpoint: Endpoint,
    pub sockaddr: SockAddr,
    pub listening: bool,
}

impl GenericSocket {
    pub fn new(endpoint: Endpoint) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let (domain, semtype, proto, address): (Domain, Type, Protocol, SockAddr) = match &endpoint
        {
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
            endpoint: endpoint,
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
                let endpoint_clone = self.endpoint.clone();
                TOKIO_RUNTIME.spawn_blocking({
                    let mut socket = self.socket.try_clone()?;
                    let observers_cloned = observers.clone();
                    move || loop {
                        let mut buffer: [u8; 65507] = [0; 65507];

                        match socket.read(&mut buffer) {
                            Ok(size) => {
                                // Convert to Vec<u8> for consistency
                                let data = buffer[..size].to_vec();
                                notify_all_observers(
                                    &observers_cloned,
                                    &SocketEngineEvent::Data(DataEvent::Received {
                                        data,
                                        from: endpoint_clone.clone(),
                                    }),
                                );
                            }
                            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                thread::sleep(std::time::Duration::from_millis(10));
                            }
                            Err(_e) => {
                                // TODO: Not sur if this is the best way to handle errors
                                notify_all_observers(
                                    &observers_cloned,
                                    &SocketEngineEvent::Error(ErrorEvent::ReceiveFailed {
                                        endpoint: endpoint_clone.clone(),
                                        reason: "UDP/BP read error".to_string(),
                                    }),
                                );
                                continue;
                            }
                        }
                    }
                });
            }

            Endpoint::Tcp(_addr) => {
                self.socket.listen(128)?;
                let endpoint_clone = self.endpoint.clone();
                TOKIO_RUNTIME.spawn_blocking({
                    let socket = self.socket.try_clone()?;
                    move || loop {
                        match socket.accept() {
                            Ok((stream, peer_addr)) => {
                                let client_addr = match peer_addr.as_socket() {
                                    Some(addr) => format!("{}:{}", addr.ip(), addr.port()),
                                    None => format!("{:?}", peer_addr),
                                };
                                // TODO: should we add ConnectionAccepted event?
                                notify_all_observers(
                                    &observers,
                                    &SocketEngineEvent::Connection(ConnectionEvent::Established {
                                        remote: Endpoint::Tcp(client_addr),
                                    }),
                                );
                                let observers_cloned = observers.clone();
                                let endpoint_for_handler = endpoint_clone.clone();
                                TOKIO_RUNTIME.spawn(async move {
                                    handle_tcp_connection(
                                        stream.into(),
                                        &observers_cloned,
                                        endpoint_for_handler,
                                    )
                                    .await;
                                });
                            }
                            Err(e) if e.kind() == io::ErrorKind::Interrupted => {
                                notify_all_observers(
                                    &observers,
                                    &SocketEngineEvent::Connection(ConnectionEvent::Closed {
                                        remote: None,
                                    }),
                                );
                            }
                            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                                thread::sleep(std::time::Duration::from_millis(10));
                            }

                            Err(e) => {
                                notify_all_observers(
                                    &observers,
                                    &SocketEngineEvent::Error(ErrorEvent::SocketError {
                                        endpoint: endpoint_clone.clone(),
                                        reason: e.to_string(),
                                    }),
                                );
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
    observers: &Vec<Arc<Mutex<dyn EngineObserver + Send + Sync>>>,
    local_endpoint: Endpoint,
) {
    let peer_addr = match stream.peer_addr() {
        Ok(addr) => addr,
        Err(_) => {
            notify_all_observers(
                observers,
                &SocketEngineEvent::Error(ErrorEvent::SocketError {
                    endpoint: local_endpoint.clone(),
                    reason: "Failed to get peer address".to_string(),
                }),
            );
            return;
        }
    };

    let peer_endpoint = Endpoint::Tcp(format!("{}:{}", peer_addr.ip(), peer_addr.port()));
    let mut buffer = [0; 1024];

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => {
                notify_all_observers(
                    observers,
                    &SocketEngineEvent::Connection(ConnectionEvent::Closed {
                        remote: Some(peer_endpoint.clone()),
                    }),
                );
                break;
            }
            Ok(size) => {
                let received_data = buffer[..size].to_vec();

                notify_all_observers(
                    observers,
                    &SocketEngineEvent::Data(DataEvent::Received {
                        data: received_data,
                        from: peer_endpoint.clone(),
                    }),
                );
            }
            Err(_e) => {
                notify_all_observers(
                    observers,
                    &SocketEngineEvent::Error(ErrorEvent::ReceiveFailed {
                        endpoint: local_endpoint,
                        reason: format!("{}", peer_endpoint),
                    }),
                );
                break;
            }
        }
    }
}
