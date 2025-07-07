use crate::{endpoint::Endpoint, event::{self, EngineObserver, SocketEngineEvent}, socket::GenericSocket};
use once_cell::sync::Lazy;
use std::{
    io::Write,
    sync::{Arc, Mutex},
};
use tokio::runtime::Runtime;
pub static TOKIO_RUNTIME: Lazy<Runtime> =
    Lazy::new(|| Runtime::new().expect("Failed to create Tokio runtime"));

pub struct Engine {
    observer: Arc<Mutex<dyn EngineObserver + Send + Sync>>,
}

impl Engine {
    pub fn new(observer: Arc<Mutex<dyn EngineObserver + Send + Sync>>) -> Self {
        Self { observer }
    }

    pub fn start_listener(&self, endpoint: Endpoint) {
        match GenericSocket::new(endpoint) {
            Ok(mut sock) => {
                if let Err(_e) = sock.start_listener(self.observer.clone()) {
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
        TOKIO_RUNTIME.spawn(async move {
            let mut generic_socket = GenericSocket::new(endpoint).unwrap();

            match generic_socket.endpoint {
                Endpoint::Bp(_) | Endpoint::Udp(_) => {
                    generic_socket
                        .socket
                        .send_to(&data.as_slice(), &generic_socket.sockaddr.clone())
                        .unwrap();
                }
                Endpoint::Tcp(_) => {
                    generic_socket
                        .socket
                        .connect(&generic_socket.sockaddr.clone())
                        .unwrap();
                    generic_socket.socket.write_all(&data.as_slice()).unwrap();
                    generic_socket.socket.flush().unwrap();
                    generic_socket
                        .socket
                        .shutdown(std::net::Shutdown::Both)
                        .unwrap();
                }
            }
        });
        self.observer.lock().unwrap().notify(SocketEngineEvent::Sent(data_uuid));
        Ok(())
    }
}
