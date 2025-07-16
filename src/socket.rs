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
        notify_all_observers, EngineObserver, ErrorEventSocket, EventSocket, GeneralSocketErrorEvent, GeneralSocketEvent, SocketEngineEvent, TcpEvent
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
                                    &SocketEngineEvent::Info(EventSocket::General(
                                        GeneralSocketEvent::DataReceived(
                                            data,
                                            endpoint_clone.clone(),
                                        ),
                                    )),
                                );
                            }
                            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                thread::sleep(std::time::Duration::from_millis(10));
                            }
                            Err(_e) => {
                                // TODO: Not sur if this is the best way to handle errors
                                notify_all_observers(
                                    &observers_cloned,
                                    &SocketEngineEvent::Error(ErrorEventSocket::General(
                                        GeneralSocketErrorEvent::ConnectionError(
                                            "UDP/BP read error".to_string(),
                                            endpoint_clone.clone(),
                                        ),
                                    )),
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
                            Ok((stream, _peer)) => {
                                // TODO: should we add ConnectionAccepted event?
                                notify_all_observers(
                                    &observers,
                                    &SocketEngineEvent::Info(EventSocket::Tcp(
                                        TcpEvent::ConnectionEstablished(endpoint_clone.to_string()),
                                    )),
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
                                    &SocketEngineEvent::Info(EventSocket::Tcp(
                                        TcpEvent::ConnectionClosed(endpoint_clone.to_string()),
                                    )),
                                );
                            }
                            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                                thread::sleep(std::time::Duration::from_millis(10));
                            }

                            Err(e) => {
                                notify_all_observers(
                                    &observers,
                                    &SocketEngineEvent::Error(ErrorEventSocket::General(
                                        GeneralSocketErrorEvent::ConnectionError(
                                            e.to_string(),
                                            endpoint_clone.clone(),
                                        ),
                                    )),
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
    _local_endpoint: Endpoint,
) {
    let peer_addr = match stream.peer_addr() {
        Ok(addr) => addr,
        Err(_) => {
            notify_all_observers(
                observers,
                &SocketEngineEvent::Error(ErrorEventSocket::General(
                    GeneralSocketErrorEvent::ConnectionError(
                        "Failed to get peer address".to_string(),
                        _local_endpoint.clone(),
                    ),
                )),
            );
            return;
        }
    };

    let peer_endpoint = Endpoint::Tcp(format!("{}:{}", peer_addr.ip(), peer_addr.port()));
    let mut full_data = Vec::new();
    let mut buffer = [0; 1024];

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => {
                notify_all_observers(
                    observers,
                    &SocketEngineEvent::Info(EventSocket::Tcp(TcpEvent::ConnectionClosed(
                        peer_endpoint.to_string(),
                    ))),
                );
                break;
            }
            Ok(size) => {
                let received_data = buffer[..size].to_vec();
                full_data.extend_from_slice(&received_data);
                
                // Envoyer l'événement immédiatement quand des données sont reçues
                notify_all_observers(
                    observers,
                    &SocketEngineEvent::Info(EventSocket::General(
                        GeneralSocketEvent::DataReceived(received_data, peer_endpoint.clone()),
                    )),
                );
            }
            Err(_e) => {
                notify_all_observers(
                    observers,
                    &SocketEngineEvent::Error(ErrorEventSocket::General(
                        GeneralSocketErrorEvent::ReceiveFailed(
                            peer_endpoint.to_string(),
                            _local_endpoint,
                        ),
                    )),
                );
                break;
            }
        }
    }
}
