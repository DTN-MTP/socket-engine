use crate::{
    endpoint::{Endpoint, EndpointProto},
    event::{
        notify_all_observers, ConnectionEvent, ConnectionFailureReason, DataEvent, EngineObserver,
        ErrorEvent, SocketEngineEvent,
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
        TOKIO_RUNTIME.spawn_blocking({
            let observers = self.observers.clone();
            let endpoint_clone = endpoint.clone();
            move || match GenericSocket::new(endpoint) {
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
        endpoint: Endpoint,
        data: Vec<u8>,
        token: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let observers = self.observers.clone();
        TOKIO_RUNTIME.spawn(async move {
            let mut generic_socket = GenericSocket::new(endpoint).unwrap();
            let endpoint_ref = &generic_socket.endpoint;
            let data_uuid_ref = &token;

            notify_all_observers(
                &observers,
                &&SocketEngineEvent::Data(DataEvent::Sending {
                    message_id: data_uuid_ref.clone(),
                    to: endpoint_ref.clone(),
                    bytes: data.len(),
                }),
            );

            match generic_socket.endpoint.proto {
                EndpointProto::Bp | EndpointProto::Udp => {
                    if let Err(err) = generic_socket
                        .socket
                        .send_to(&data.as_slice(), &generic_socket.sockaddr)
                    {
                        notify_all_observers(
                            &observers,
                            &SocketEngineEvent::Error(ErrorEvent::SendFailed {
                                endpoint: endpoint_ref.clone(),
                                token: data_uuid_ref.clone(),
                                reason: err.to_string(),
                            }),
                        );
                    } else {
                        notify_all_observers(
                            &observers,
                            &SocketEngineEvent::Data(DataEvent::Sent {
                                message_id: data_uuid_ref.clone(),
                                to: endpoint_ref.clone(),
                                bytes_sent: data.len(),
                            }),
                        );
                    }
                }
                EndpointProto::Tcp => {
                    if let Err(err) = generic_socket.socket.connect(&generic_socket.sockaddr) {
                        if err.kind() == std::io::ErrorKind::ConnectionRefused {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Error(ErrorEvent::ConnectionFailed {
                                    endpoint: endpoint_ref.clone(),
                                    reason: ConnectionFailureReason::Refused,
                                    token: data_uuid_ref.clone(),
                                }),
                            );
                        } else if err.kind() == std::io::ErrorKind::TimedOut {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Error(ErrorEvent::ConnectionFailed {
                                    endpoint: endpoint_ref.clone(),
                                    reason: ConnectionFailureReason::Timeout,
                                    token: data_uuid_ref.clone(),
                                }),
                            );
                        } else {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Error(ErrorEvent::ConnectionFailed {
                                    endpoint: endpoint_ref.clone(),
                                    reason: ConnectionFailureReason::Other,
                                    token: data_uuid_ref.clone(),
                                }),
                            );
                        }
                    } else {
                        notify_all_observers(
                            &observers,
                            &SocketEngineEvent::Connection(ConnectionEvent::Established {
                                remote: endpoint_ref.clone(), // Remote is the target we're connecting to
                            }),
                        );

                        if let Err(err) = generic_socket.socket.write_all(&data.as_slice()) {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Error(ErrorEvent::SendFailed {
                                    endpoint: endpoint_ref.clone(),
                                    token: data_uuid_ref.clone(),
                                    reason: err.to_string(),
                                }),
                            );
                        } else {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Data(DataEvent::Sent {
                                    message_id: data_uuid_ref.clone(),
                                    to: endpoint_ref.clone(),
                                    bytes_sent: data.len(),
                                }),
                            );
                        }

                        if let Err(err) = generic_socket.socket.flush() {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Error(ErrorEvent::SendFailed {
                                    endpoint: endpoint_ref.clone(),
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
