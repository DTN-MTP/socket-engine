use std::sync::{Arc, Mutex};
use tokio::{net::{TcpStream, UdpSocket}, io::AsyncWriteExt};
use socket2::{Domain, Protocol, Socket, Type};

use crate::{
    constants::protocol::AF_BP,
    endpoint::{create_bp_sockaddr_with_string, Endpoint},
    event::{
        notify_all_observers, ConnectionEvent, ConnectionFailureReason, DataEvent, EngineObserver,
        ErrorEvent, SocketEngineEvent,
    },
};

pub struct TcpSender;

impl TcpSender {
    pub async fn send(
        addr: String,
        data: Vec<u8>,
        token: String,
        endpoint: Endpoint,
        observers: Vec<Arc<Mutex<dyn EngineObserver + Send + Sync>>>,
    ) {
        match TcpStream::connect(&addr).await {
            Ok(mut stream) => {
                notify_all_observers(
                    &observers,
                    &SocketEngineEvent::Connection(ConnectionEvent::Established {
                        remote: endpoint.clone(),
                    }),
                );

                match stream.write_all(&data).await {
                    Ok(_) => {
                        notify_all_observers(
                            &observers,
                            &SocketEngineEvent::Data(DataEvent::Sent {
                                message_id: token.clone(),
                                to: endpoint.clone(),
                                bytes_sent: data.len(),
                            }),
                        );
                        
                        let _ = stream.shutdown().await;
                        notify_all_observers(
                            &observers,
                            &SocketEngineEvent::Connection(ConnectionEvent::Closed {
                                remote: Some(endpoint.clone()),
                            }),
                        );
                    }
                    Err(e) => {
                        notify_all_observers(
                            &observers,
                            &SocketEngineEvent::Error(ErrorEvent::SendFailed {
                                endpoint: endpoint.clone(),
                                token: token.clone(),
                                reason: e.to_string(),
                            }),
                        );
                    }
                }
            }
            Err(e) => {
                let reason = if e.kind() == std::io::ErrorKind::ConnectionRefused {
                    ConnectionFailureReason::Refused
                } else if e.kind() == std::io::ErrorKind::TimedOut {
                    ConnectionFailureReason::Timeout
                } else {
                    ConnectionFailureReason::Other
                };

                notify_all_observers(
                    &observers,
                    &SocketEngineEvent::Error(ErrorEvent::ConnectionFailed {
                        endpoint: endpoint.clone(),
                        reason,
                        token: e.to_string(),
                    }),
                );
            }
        }
    }
}

pub struct UdpSender;

impl UdpSender {
    pub async fn send(
        addr: String,
        data: Vec<u8>,
        token: String,
        endpoint: Endpoint,
        observers: Vec<Arc<Mutex<dyn EngineObserver + Send + Sync>>>,
    ) {
        match UdpSocket::bind("0.0.0.0:0").await {
            Ok(socket) => {
                match socket.send_to(&data, &addr).await {
                    Ok(_) => {
                        notify_all_observers(
                            &observers,
                            &SocketEngineEvent::Data(DataEvent::Sent {
                                message_id: token.clone(),
                                to: endpoint.clone(),
                                bytes_sent: data.len(),
                            }),
                        );
                    }
                    Err(e) => {
                        notify_all_observers(
                            &observers,
                            &SocketEngineEvent::Error(ErrorEvent::SendFailed {
                                endpoint: endpoint.clone(),
                                token: token.clone(),
                                reason: e.to_string(),
                            }),
                        );
                    }
                }
            }
            Err(e) => {
                notify_all_observers(
                    &observers,
                    &SocketEngineEvent::Error(ErrorEvent::SendFailed {
                        endpoint: endpoint.clone(),
                        token: token.clone(),
                        reason: format!("Failed to create UDP socket: {}", e),
                    }),
                );
            }
        }
    }
}

pub struct BpSender;

impl BpSender {
    pub fn send(
        data: Vec<u8>,
        token: String,
        endpoint: Endpoint,
        observers: Vec<Arc<Mutex<dyn EngineObserver + Send + Sync>>>,
    ) {
        match &endpoint {
            Endpoint::Bp(addr) => {
                match Self::create_bp_socket(addr) {
                    Ok((socket, sockaddr)) => {
                        if let Err(err) = socket.send_to(&data, &sockaddr) {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Error(ErrorEvent::SendFailed {
                                    endpoint: endpoint.clone(),
                                    token: token.clone(),
                                    reason: err.to_string(),
                                }),
                            );
                        } else {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Data(DataEvent::Sent {
                                    message_id: token.clone(),
                                    to: endpoint.clone(),
                                    bytes_sent: data.len(),
                                }),
                            );
                        }
                    }
                    Err(e) => {
                        notify_all_observers(
                            &observers,
                            &SocketEngineEvent::Error(ErrorEvent::SendFailed {
                                endpoint: endpoint.clone(),
                                token: token.clone(),
                                reason: e.to_string(),
                            }),
                        );
                    }
                }
            }
            _ => {
                notify_all_observers(
                    &observers,
                    &SocketEngineEvent::Error(ErrorEvent::SendFailed {
                        endpoint: endpoint.clone(),
                        token: token.clone(),
                        reason: "BpSender can only handle BP endpoints".to_string(),
                    }),
                );
            }
        }
    }
    
    fn create_bp_socket(addr: &str) -> Result<(Socket, socket2::SockAddr), Box<dyn std::error::Error + Send + Sync>> {
        let address = create_bp_sockaddr_with_string(addr)?;
        let socket = Socket::new(
            Domain::from(AF_BP),
            Type::DGRAM,
            Some(Protocol::from(0)),
        )?;
        Ok((socket, address))
    }
}
