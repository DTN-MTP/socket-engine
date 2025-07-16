use crate::{
    endpoint::Endpoint,
    event::{
        notify_all_observers, EngineObserver, ErrorEventSocket, EventSocket,
        GeneralSocketErrorEvent, GeneralSocketEvent, SocketEngineEvent, TcpErrorEvent, TcpEvent,
        UdpEvent,
    },
    socket::GenericSocket,
};
use once_cell::sync::Lazy;
use std::{
    io::Write,
    sync::{Arc, Mutex},
};
use tokio::runtime::Runtime;

pub static TOKIO_RUNTIME: Lazy<Runtime> =
    Lazy::new(|| Runtime::new().expect("Failed to create Tokio runtime"));

pub struct Engine {
    observers: Vec<Arc<Mutex<dyn EngineObserver + Send + Sync>>>,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            observers: Vec::new(),
        }
    }
    pub fn add_observer(&mut self, obs: Arc<Mutex<dyn EngineObserver + Send + Sync>>) {
        self.observers.push(obs);
        }

    pub fn start_listener_async(&self, endpoint: Endpoint) {
        let observers = self.observers.clone();
        let endpoint_clone = endpoint.clone();
        
        // TODO: should remove spawn_blocking and use async directly
        TOKIO_RUNTIME.spawn_blocking(move || {
            match GenericSocket::new(endpoint) {
                Ok(mut sock) => {
                    if let Err(e) = sock.start_listener(observers.clone()) {
                        notify_all_observers(
                            &observers,
                            &SocketEngineEvent::Error(ErrorEventSocket::General(
                                GeneralSocketErrorEvent::ConnectionError(
                                    e.to_string(),
                                    sock.endpoint.clone(),
                                ),
                            )),
                        );
                    } else {
                        if let Endpoint::Tcp(_) = sock.endpoint {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Info(EventSocket::Tcp(TcpEvent::ListenerStarted(
                                    sock.endpoint.to_string(),
                                ))),
                            );
                        }
                    }
                }
                Err(_) => {
                    notify_all_observers(
                        &observers,
                        &SocketEngineEvent::Error(ErrorEventSocket::General(
                            GeneralSocketErrorEvent::ConnectionError(
                                "Failed to create socket".to_string(),
                                endpoint_clone,
                            ),
                        )),
                    );
                }
            }
        });
    }
    pub fn send_async(
        &self,
        endpoint: Endpoint,
        data: Vec<u8>,
        data_uuid: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let observers = self.observers.clone();
        TOKIO_RUNTIME.spawn(async move {
            let mut generic_socket = GenericSocket::new(endpoint).unwrap();

            match generic_socket.endpoint {
                Endpoint::Bp(_) | Endpoint::Udp(_) => {
                    if let Err(err) = generic_socket
                        .socket
                        .send_to(&data.as_slice(), &generic_socket.sockaddr.clone())
                    {
                        notify_all_observers(
                            &observers,
                            &SocketEngineEvent::Error(ErrorEventSocket::General(
                                GeneralSocketErrorEvent::SendFailed(
                                    err.to_string(),
                                    data_uuid.clone(),
                                    generic_socket.endpoint.clone(),
                                ),
                            )),
                        );
                    } else {
                        notify_all_observers(
                            &observers,
                            &SocketEngineEvent::Info(EventSocket::Udp(UdpEvent::PacketSizeSent(
                                data.len() as u64,
                            ))),
                        );
                    }
                }
                Endpoint::Tcp(_) => {
                    
                    if let Err(err) = generic_socket
                        .socket
                        .connect(&generic_socket.sockaddr.clone())
                    {
                        if err.kind() == std::io::ErrorKind::ConnectionRefused {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Error(ErrorEventSocket::Tcp(
                                    TcpErrorEvent::ConnectionRefused(
                                        err.to_string(),
                                        generic_socket.endpoint.clone(),
                                    ),
                                )),
                            );
                        } 
                        if err.kind() == std::io::ErrorKind::TimedOut {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Error(ErrorEventSocket::Tcp(
                                    TcpErrorEvent::ConnectionTimeout(
                                        err.to_string(),
                                        generic_socket.endpoint.clone(),
                                    ),
                                )),
                            );
                        } else {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Error(ErrorEventSocket::General(
                                    GeneralSocketErrorEvent::ConnectionError(
                                        err.to_string(),
                                        generic_socket.endpoint.clone(),
                                    ),
                                )),
                            );
                        }
                    } else {
                        
                        notify_all_observers(
                            &observers,
                            &SocketEngineEvent::Info(EventSocket::Tcp(TcpEvent::ConnectionEstablished(
                                generic_socket.endpoint.to_string(),
                            ))),
                        );
 
                        if let Err(err) = generic_socket.socket.write_all(&data.as_slice()) {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Error(ErrorEventSocket::General(
                                    GeneralSocketErrorEvent::SendFailed(
                                        err.to_string(),
                                        data_uuid.clone(),
                                        generic_socket.endpoint.clone(),
                                    ),
                                )),
                            );
                        } else {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Info(EventSocket::General(
                                    GeneralSocketEvent::DataSent(
                                        data_uuid.clone(),
                                        generic_socket.endpoint.clone(),
                                    ),
                                )),
                            );
                        }

                        if let Err(err) = generic_socket.socket.flush() {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Error(ErrorEventSocket::General(
                                    GeneralSocketErrorEvent::SendFailed(
                                        err.to_string(),
                                        data_uuid.clone(),
                                        generic_socket.endpoint.clone(),
                                    ),
                                )),
                            );
                        }

                        
                       
                        if let Err(err) = generic_socket.socket.shutdown(std::net::Shutdown::Both) {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Error(ErrorEventSocket::General(
                                    GeneralSocketErrorEvent::SendFailed(
                                        format!("Shutdown failed: {}", err),
                                        data_uuid.clone(),
                                        generic_socket.endpoint.clone(),
                                    ),
                                )),
                            );
                        } else {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Info(EventSocket::Tcp(TcpEvent::ConnectionClosed(
                                    data_uuid.clone(),
                                ))),
                            );
                        }
                    }
                }
            }

        });
        Ok(())
    }
}
