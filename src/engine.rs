use crate::{encoding::encode_proto_message, endpoint::Endpoint, socket::GenericSocket};
use once_cell::sync::Lazy;
use prost::Message;
use std::{
    io::Write,
    sync::{Arc, Mutex},
};
use tokio::runtime::Runtime;
pub static TOKIO_RUNTIME: Lazy<Runtime> =
    Lazy::new(|| Runtime::new().expect("Failed to create Tokio runtime"));

use crate::proto::ProtoMessage;

// pub fn build_msg() -> ProtoMessage {
//     ProtoMessage {
//         uuid: "1".to_string(),
//         sender_uuid: "user1".to_string(),
//         timestamp: 1234567890,
//         room_uuid: "room42".to_string(),
//         content: Some(Content::Text(TextMessage {
//             content: "Hello!".to_string(),
//         })),
//     }
// }

pub trait EngineObserver: Send + Sync {
    fn get_notification(&mut self, message: ProtoMessage);
}

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
                if let Err(e) = sock.start_listener(self.observer.clone()) {
                    eprintln!("Failed to start listener for endpoint {}", e);
                    // Continue with next endpoint instead of failing completely
                }
            }
            Err(e) => {
                eprintln!("Failed to create socket for endpoint {:?}:", e);
                // Continue with next endpoint instead of failing
            }
        }
    }

    pub fn send(
        &self,
        endpoint: Endpoint,
        message: ProtoMessage,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Encode message

            let buf = encode_proto_message(&message)?;

            TOKIO_RUNTIME.spawn(async move {
            let mut generic_socket = GenericSocket::new(endpoint).unwrap();

            match generic_socket.endpoint {
                Endpoint::Bp(_) | Endpoint::Udp(_) => {
                    generic_socket
                        .socket
                        .send_to(&buf.as_slice(), &generic_socket.sockaddr.clone())
                        .unwrap();
                }
                Endpoint::Tcp(_) => {
                    generic_socket
                        .socket
                        .connect(&generic_socket.sockaddr.clone())
                        .unwrap();
                    generic_socket.socket.write_all(&buf.as_slice()).unwrap();
                    generic_socket.socket.flush().unwrap();
                    generic_socket
                        .socket
                        .shutdown(std::net::Shutdown::Both)
                        .unwrap();
                }
            }
        });
        Ok(())
    }
}
