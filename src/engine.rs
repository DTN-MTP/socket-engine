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

    fn create_socket_and_store(
        &mut self,
        endpoint: Endpoint,
    ) -> Result<GenericSocket, Box<dyn std::error::Error + Send + Sync>> {
        let socket = match GenericSocket::new(endpoint.clone()) {
            Ok(sock) => sock,
            Err(e) => {
                return Err(e);
            }
        };

        match socket.try_clone() {
            Ok(sock) => self.sockets.insert(endpoint.clone(), sock),
            Err(e) => {
                return Err(Box::new(e));
            }
        };
        return Ok(socket);
    }

    pub fn start_listener_async(&mut self, endpoint: Endpoint) {
        let res = self.create_socket_and_store(endpoint.clone());

        TOKIO_RUNTIME.spawn_blocking({
            let observers = self.observers.clone();
            let endpoint_clone = endpoint.clone();
            move || match res {
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

    fn try_reuse_socket(
        &self,
        source_opt: Option<Endpoint>,
        dest: Endpoint,
    ) -> Result<GenericSocket, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(source) = source_opt {
            if dest.proto == EndpointProto::Bp || dest.proto == EndpointProto::Udp {
                if let Some(existing_sock) = self.sockets.get(&source) {
                    return existing_sock.try_clone().map_err(Into::into);
                }
            }
        }
        // Should be safe as we do not bind
        GenericSocket::new(dest).map_err(Into::into)
    }

    pub fn send_async(
        &self,
        source_endpoint: Option<Endpoint>,
        target_endpoint: Endpoint,
        data: Vec<u8>,
        token: String,
    ) {
        let observers = self.observers.clone();
        let target_endpoint_clone = target_endpoint.clone();
        let generic_socket_res = self.try_reuse_socket(source_endpoint, target_endpoint);

        let sock_addr = endpoint_to_sockaddr(target_endpoint_clone.clone()).unwrap();

        TOKIO_RUNTIME.spawn(async move {
            let data_uuid_ref = &token;

            let mut generic_socket = match generic_socket_res {
                Ok(generic_socket) => generic_socket,
                Err(e) => {
                    notify_all_observers(
                        &observers,
                        &&SocketEngineEvent::Error(ErrorEvent::SocketError {
                            endpoint: target_endpoint_clone,
                            reason: e.to_string(),
                        }),
                    );
                    return;
                }
            };

            notify_all_observers(
                &observers,
                &SocketEngineEvent::Data(DataEvent::Sending {
                    message_id: data_uuid_ref.clone(),
                    to: target_endpoint_clone.clone(),
                    bytes: data.len(),
                }),
            );

            match generic_socket.endpoint.proto {
                EndpointProto::Bp | EndpointProto::Udp => {
                    if let Err(err) = generic_socket.socket.send_to(&data.as_slice(), &sock_addr) {
                        notify_all_observers(
                            &observers,
                            &SocketEngineEvent::Error(ErrorEvent::SendFailed {
                                endpoint: target_endpoint_clone.clone(),
                                token: data_uuid_ref.clone(),
                                reason: err.to_string(),
                            }),
                        );
                    } else {
                        notify_all_observers(
                            &observers,
                            &SocketEngineEvent::Data(DataEvent::Sent {
                                message_id: data_uuid_ref.clone(),
                                to: target_endpoint_clone.clone(),
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
                                    endpoint: target_endpoint_clone.clone(),
                                    reason: ConnectionFailureReason::Refused,
                                    token: data_uuid_ref.clone(),
                                }),
                            );
                        } else if err.kind() == std::io::ErrorKind::TimedOut {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Error(ErrorEvent::ConnectionFailed {
                                    endpoint: target_endpoint_clone.clone(),
                                    reason: ConnectionFailureReason::Timeout,
                                    token: data_uuid_ref.clone(),
                                }),
                            );
                        } else {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Error(ErrorEvent::ConnectionFailed {
                                    endpoint: target_endpoint_clone.clone(),
                                    reason: ConnectionFailureReason::Other,
                                    token: data_uuid_ref.clone(),
                                }),
                            );
                        }
                    } else {
                        notify_all_observers(
                            &observers,
                            &SocketEngineEvent::Connection(ConnectionEvent::Established {
                                remote: target_endpoint_clone.clone(), // Remote is the target we're connecting to
                            }),
                        );

                        if let Err(err) = generic_socket.socket.write_all(&data.as_slice()) {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Error(ErrorEvent::SendFailed {
                                    endpoint: target_endpoint_clone.clone(),
                                    token: data_uuid_ref.clone(),
                                    reason: err.to_string(),
                                }),
                            );
                        } else {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Data(DataEvent::Sent {
                                    message_id: data_uuid_ref.clone(),
                                    to: target_endpoint_clone.clone(),
                                    bytes_sent: data.len(),
                                }),
                            );
                        }

                        if let Err(err) = generic_socket.socket.flush() {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Error(ErrorEvent::SendFailed {
                                    endpoint: target_endpoint_clone.clone(),
                                    token: data_uuid_ref.clone(),
                                    reason: err.to_string(),
                                }),
                            );
                        }

                        if let Err(err) = generic_socket.socket.shutdown(std::net::Shutdown::Both) {
                            notify_all_observers(
                                &observers,
                                &SocketEngineEvent::Error(ErrorEvent::SendFailed {
                                    endpoint: target_endpoint_clone.clone(),
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
    }
}
