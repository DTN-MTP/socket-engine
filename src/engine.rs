use crate::{
    endpoint::Endpoint,
    event::{notify_all, EngineObserver, SocketEngineEvent},
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
        match GenericSocket::new(endpoint) {
            Ok(mut sock) => {
                if let Err(_e) = sock.start_listener(self.observers.clone()) {
                    // eprintln!("Failed to start listener for endpoint {}", e);
                    todo!();
                    // Continue with next endpoint instead of failing completely
                }
            }
            Err(_e) => {
                // eprintln!("Failed to create socket for endpoint {:?}:", e);
                // Continue with next endpoint instead of failing
                todo!();
            }
        }
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
                        notify_all(
                            &observers,
                            &SocketEngineEvent::SentError((err.to_string(), data_uuid.clone())),
                        );
                    }
                }
                Endpoint::Tcp(_) => {
                    if let Err(err) = generic_socket
                        .socket
                        .connect(&generic_socket.sockaddr.clone())
                    {
                        notify_all(
                            &observers,
                            &SocketEngineEvent::SentError((err.to_string(), data_uuid.clone())),
                        );
                    }
                    generic_socket.socket.write_all(&data.as_slice()).unwrap();
                    generic_socket.socket.flush().unwrap();
                    if let Err(err) = generic_socket.socket.shutdown(std::net::Shutdown::Both) {
                        notify_all(
                            &observers,
                            &SocketEngineEvent::SentError((err.to_string(), data_uuid.clone())),
                        );
                    }
                }
            }
            notify_all(&observers, &SocketEngineEvent::Sent(data_uuid.clone()));
        });
        Ok(())
    }
}
