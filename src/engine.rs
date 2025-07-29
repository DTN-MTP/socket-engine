use crate::{
    endpoint::{Endpoint, EndpointProto},
    event::{
        notify_all_observers, ConnectionEvent, ConnectionFailureReason, DataEvent, EngineObserver,
        ErrorEvent, SocketEngineEvent,
    },
    socket::{endpoint_to_sockaddr, GenericSocket},
};

use once_cell::sync::Lazy;
use std::{
    collections::HashMap,
    io::Write,
    sync::{Arc, Mutex},
};
use tokio::runtime::Runtime;

pub static TOKIO_RUNTIME: Lazy<Runtime> =
    Lazy::new(|| Runtime::new().expect("Failed to create Tokio runtime"));

pub struct Engine {
    observers: Vec<Arc<Mutex<dyn EngineObserver + Send + Sync>>>,
    sockets: HashMap<Endpoint, GenericSocket>,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            observers: Vec::new(),
            sockets: HashMap::new(),
        }
    }
    pub fn add_observer(&mut self, obs: Arc<Mutex<dyn EngineObserver + Send + Sync>>) {
        self.observers.push(obs);
    }

    pub fn start_listener_async(&mut self, endpoint: Endpoint) {
        let socket = match GenericSocket::new(endpoint.clone()) {
            Ok(sock) => sock,
            Err(e) => {
                notify_all_observers(
                    &self.observers,
                    &SocketEngineEvent::Error(ErrorEvent::SocketError {
                        endpoint: endpoint.clone(),
                        reason: e.to_string(),
                    }),
                );
                return;
            }
        };

        match socket.try_clone() {
            Ok(sock) => self.sockets.insert(endpoint.clone(), sock),
            Err(e) => {
                notify_all_observers(
                    &self.observers,
                    &SocketEngineEvent::Error(ErrorEvent::SocketError {
                        endpoint: endpoint.clone(),
                        reason: e.to_string(),
                    }),
                );
                return;
            }
        };

        TOKIO_RUNTIME.spawn_blocking({
            let observers = self.observers.clone();
            let endpoint_clone = endpoint.clone();
            move || match socket.try_clone() {
                Ok(mut sock) => {
                    if let Err(e) = sock.start_listener(observers.clone()) {
                        notify_all_observers(
                            &observers,
                            &SocketEngineEvent::Error(ErrorEvent::SocketError {
                                endpoint: sock.endpoint.clone(),
                                reason: e.to_string(),
                            }),
                        );
                    } else {
                        if let EndpointProto::Tcp = sock.endpoint.proto {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Connection(ConnectionEvent::ListenerStarted {
                                    endpoint: sock.endpoint.clone(),
                                }),
                            );
                        }
                    }
                }
                Err(e) => {
                    notify_all_observers(
                        &observers,
                        &SocketEngineEvent::Error(ErrorEvent::SocketError {
                            endpoint: endpoint_clone,
                            reason: e.to_string(),
                        }),
                    );
                }
            }
        });
    }

    pub fn send_async(
        &self,
        source_endpoint: Endpoint,
        endpoint: Endpoint,
        data: Vec<u8>,
        token: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let observers = self.observers.clone();
        let target_endpoint = endpoint.clone();
        let mut generic_socket = if endpoint.proto == EndpointProto::Bp {
            match self.sockets.get(&source_endpoint) {
                Some(existing_sock) => {
                    let Ok(found_sock) = existing_sock.try_clone() else {
                        notify_all_observers(
                            &observers,
                            &SocketEngineEvent::Error(ErrorEvent::SocketError {
                                endpoint: endpoint.clone(),
                                reason: "Unable to clone listening socket".to_string(),
                            }),
                        );
                        return Ok(());
                    };
                    found_sock
                }
                None => GenericSocket::new(endpoint).unwrap(),
            }
        } else {
            GenericSocket::new(endpoint).unwrap()
        };

        let sock_addr = endpoint_to_sockaddr(target_endpoint.clone()).unwrap();

        TOKIO_RUNTIME.spawn(async move {
            let data_uuid_ref = &token;

            notify_all_observers(
                &observers,
                &&SocketEngineEvent::Data(DataEvent::Sending {
                    message_id: data_uuid_ref.clone(),
                    to: target_endpoint.clone(),
                    bytes: data.len(),
                }),
            );

            match generic_socket.endpoint.proto {
                EndpointProto::Bp | EndpointProto::Udp => {
                    if let Err(err) = generic_socket.socket.send_to(&data.as_slice(), &sock_addr) {
                        notify_all_observers(
                            &observers,
                            &SocketEngineEvent::Error(ErrorEvent::SendFailed {
                                endpoint: target_endpoint.clone(),
                                token: data_uuid_ref.clone(),
                                reason: err.to_string(),
                            }),
                        );
                    } else {
                        notify_all_observers(
                            &observers,
                            &SocketEngineEvent::Data(DataEvent::Sent {
                                message_id: data_uuid_ref.clone(),
                                to: target_endpoint.clone(),
                                bytes_sent: data.len(),
                            }),
                        );
                    }
                }
                EndpointProto::Tcp => {
                    if let Err(err) = generic_socket.socket.connect(&sock_addr) {
                        if err.kind() == std::io::ErrorKind::ConnectionRefused {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Error(ErrorEvent::ConnectionFailed {
                                    endpoint: target_endpoint.clone(),
                                    reason: ConnectionFailureReason::Refused,
                                    token: data_uuid_ref.clone(),
                                }),
                            );
                        } else if err.kind() == std::io::ErrorKind::TimedOut {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Error(ErrorEvent::ConnectionFailed {
                                    endpoint: target_endpoint.clone(),
                                    reason: ConnectionFailureReason::Timeout,
                                    token: data_uuid_ref.clone(),
                                }),
                            );
                        } else {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Error(ErrorEvent::ConnectionFailed {
                                    endpoint: target_endpoint.clone(),
                                    reason: ConnectionFailureReason::Other,
                                    token: data_uuid_ref.clone(),
                                }),
                            );
                        }
                    } else {
                        notify_all_observers(
                            &observers,
                            &SocketEngineEvent::Connection(ConnectionEvent::Established {
                                remote: target_endpoint.clone(), // Remote is the target we're connecting to
                            }),
                        );

                        if let Err(err) = generic_socket.socket.write_all(&data.as_slice()) {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Error(ErrorEvent::SendFailed {
                                    endpoint: target_endpoint.clone(),
                                    token: data_uuid_ref.clone(),
                                    reason: err.to_string(),
                                }),
                            );
                        } else {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Data(DataEvent::Sent {
                                    message_id: data_uuid_ref.clone(),
                                    to: target_endpoint.clone(),
                                    bytes_sent: data.len(),
                                }),
                            );
                        }

                        if let Err(err) = generic_socket.socket.flush() {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Error(ErrorEvent::SendFailed {
                                    endpoint: target_endpoint.clone(),
                                    token: data_uuid_ref.clone(),
                                    reason: err.to_string(),
                                }),
                            );
                        }

                        if let Err(err) = generic_socket.socket.shutdown(std::net::Shutdown::Both) {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Error(ErrorEvent::SendFailed {
                                    endpoint: generic_socket.endpoint.clone(),
                                    token: data_uuid_ref.clone(),
                                    reason: format!("Shutdown failed: {}", err),
                                }),
                            );
                        } else {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Connection(ConnectionEvent::Closed {
                                    remote: Some(generic_socket.endpoint.clone()),
                                }),
                            );
                        }
                    }
                }
            }
        });
        Ok(())
    }
}
