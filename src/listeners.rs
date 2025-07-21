use std::{error::Error, io, mem::MaybeUninit, sync::{Arc, Mutex}};
use tokio::{net::{TcpListener, TcpStream, UdpSocket}, io::AsyncReadExt};
use socket2::{Domain, Protocol, Socket, Type};

use crate::{
    constants::{buffer::{TCP_BUFFER_SIZE, UDP_MAX_DATAGRAM_SIZE}, protocol::AF_BP},
    endpoint::{create_bp_sockaddr_with_string, Endpoint},
    event::{
        notify_all_observers, ConnectionEvent, DataEvent, EngineObserver,
        ErrorEvent, SocketEngineEvent,
    },
};

pub struct TcpListenerHandler;

impl TcpListenerHandler {
    pub async fn start(
        addr: String,
        observers: Vec<Arc<Mutex<dyn EngineObserver + Send + Sync>>>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let listener = TcpListener::bind(&addr).await?;
        let endpoint = Endpoint::Tcp(addr);
        
        notify_all_observers(
            &observers,
            &SocketEngineEvent::Connection(ConnectionEvent::ListenerStarted {
                endpoint: endpoint.clone(),
            }),
        );

        loop {
            match listener.accept().await {
                Ok((stream, peer_addr)) => {
                    let client_endpoint = Endpoint::Tcp(peer_addr.to_string());
                    
                    notify_all_observers(
                        &observers,
                        &SocketEngineEvent::Connection(ConnectionEvent::Established {
                            remote: client_endpoint.clone(),
                        }),
                    );

                    let obs_clone = observers.clone();
                    let endpoint_clone = endpoint.clone();
                    
                    // Spawn une tâche async pour chaque connection
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_tcp_stream(stream, obs_clone, endpoint_clone).await {
                            eprintln!("TCP connection error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    notify_all_observers(
                        &observers,
                        &SocketEngineEvent::Error(ErrorEvent::SocketError {
                            endpoint: endpoint.clone(),
                            reason: e.to_string(),
                        }),
                    );
                    return Err("TCP listener error".into());
                }
            }
        }
    }

    async fn handle_tcp_stream(
        mut stream: TcpStream,
        observers: Vec<Arc<Mutex<dyn EngineObserver + Send + Sync>>>,
        endpoint: Endpoint,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut buffer = [0u8; TCP_BUFFER_SIZE];
        
        loop {
            match stream.read(&mut buffer).await {
                Ok(0) => {
                    notify_all_observers(
                        &observers,
                        &SocketEngineEvent::Connection(ConnectionEvent::Closed {
                            remote: Some(endpoint.clone()),
                        }),
                    );
                    break;
                }
                Ok(size) => {
                    let data = buffer[..size].to_vec();
                    notify_all_observers(
                        &observers,
                        &SocketEngineEvent::Data(DataEvent::Received {
                            data,
                            from: endpoint.clone(),
                        }),
                    );
                }
                Err(e) => {
                    notify_all_observers(
                        &observers,
                        &SocketEngineEvent::Error(ErrorEvent::ReceiveFailed {
                            endpoint: endpoint.clone(),
                            reason: e.to_string(),
                        }),
                    );
                    break;
                }
            }
        }
        Ok(())
    }
}

pub struct UdpListenerHandler;

impl UdpListenerHandler {
    pub async fn start(
        addr: String,
        observers: Vec<Arc<Mutex<dyn EngineObserver + Send + Sync>>>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let socket = UdpSocket::bind(&addr).await?;
        let endpoint = Endpoint::Udp(addr);
        
        notify_all_observers(
            &observers,
            &SocketEngineEvent::Connection(ConnectionEvent::ListenerStarted {
                endpoint: endpoint.clone(),
            }),
        );

        let mut buffer = [0u8; UDP_MAX_DATAGRAM_SIZE];
        
        loop {
            match socket.recv_from(&mut buffer).await {
                Ok((size, peer_addr)) => {
                    let data = buffer[..size].to_vec();
                    let from_endpoint = Endpoint::Udp(peer_addr.to_string());
                    
                    notify_all_observers(
                        &observers,
                        &SocketEngineEvent::Data(DataEvent::Received {
                            data,
                            from: from_endpoint,
                        }),
                    );
                }
                Err(e) => {
                    notify_all_observers(
                        &observers,
                        &SocketEngineEvent::Error(ErrorEvent::ReceiveFailed {
                            endpoint: endpoint.clone(),
                            reason: e.to_string(),
                        }),
                    );
                    return Err("UDP listener error".into());
                }
            }
        }
    }
}

pub struct BpListenerHandler;

impl BpListenerHandler {
    pub fn start_blocking(
        endpoint: Endpoint,
        observers: Vec<Arc<Mutex<dyn EngineObserver + Send + Sync>>>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        match &endpoint {
            Endpoint::Bp(addr) => {
                let address = create_bp_sockaddr_with_string(addr)?;
                let socket = Socket::new(
                    Domain::from(AF_BP),
                    Type::DGRAM,
                    Some(Protocol::from(0)),
                )?;
                
                socket.set_nonblocking(true)?;
                socket.set_reuse_address(true)?;
                socket.bind(&address)?;
                
                notify_all_observers(
                    &observers,
                    &SocketEngineEvent::Connection(ConnectionEvent::ListenerStarted {
                        endpoint: endpoint.clone(),
                    }),
                );

                // Thread dédié pour BP car tokio ne le supporte pas nativement
                let endpoint_clone = endpoint.clone();
                let observers_clone = observers;
                
                std::thread::spawn(move || {
                    let mut buffer: [MaybeUninit<u8>; UDP_MAX_DATAGRAM_SIZE] = 
                        unsafe { MaybeUninit::uninit().assume_init() };
                    
                    loop {
                        match socket.recv(&mut buffer) {
                            Ok(size) => {
                                // Convert MaybeUninit to regular u8 array
                                let data: Vec<u8> = buffer[..size]
                                    .iter()
                                    .map(|b| unsafe { b.assume_init() })
                                    .collect();
                                notify_all_observers(
                                    &observers_clone,
                                    &SocketEngineEvent::Data(DataEvent::Received {
                                        data,
                                        from: endpoint_clone.clone(),
                                    }),
                                );
                            }
                            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                std::thread::sleep(std::time::Duration::from_millis(10));
                            }
                            Err(e) => {
                                notify_all_observers(
                                    &observers_clone,
                                    &SocketEngineEvent::Error(ErrorEvent::ReceiveFailed {
                                        endpoint: endpoint_clone.clone(),
                                        reason: e.to_string(),
                                    }),
                                );
                                break;
                            }
                        }
                    }
                });
                
                Ok(())
            }
            _ => Err("BpListenerHandler can only handle BP endpoints".into()),
        }
    }
}
