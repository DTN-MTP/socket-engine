use crate::{
    endpoint::Endpoint,
    event::{
        notify_all_observers, DataEvent, EngineObserver,
        ErrorEvent, SocketEngineEvent,
    },
    listeners::{TcpListenerHandler, UdpListenerHandler, BpListenerHandler},
    senders::{TcpSender, UdpSender, BpSender},
};
use std::{error::Error, sync::{Arc, Mutex}};

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
        tokio::spawn(async move {
            if let Err(e) = Engine::start_async_listener(endpoint.clone(), observers.clone()).await {
                notify_all_observers(
                    &observers,
                    &SocketEngineEvent::Error(ErrorEvent::SocketError {
                        endpoint,
                        reason: e.to_string(),
                    }),
                );
            }
        });
    }

    async fn start_async_listener(
        endpoint: Endpoint,
        observers: Vec<Arc<Mutex<dyn EngineObserver + Send + Sync>>>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        match endpoint {
            Endpoint::Tcp(addr) => {
                TcpListenerHandler::start(addr, observers).await
            }
            Endpoint::Udp(addr) => {
                UdpListenerHandler::start(addr, observers).await
            }
            Endpoint::Bp(_) => {
                // Pour BP, on garde l'approche existante avec spawn_blocking
                tokio::task::spawn_blocking(move || {
                    BpListenerHandler::start_blocking(endpoint, observers)
                });
                Ok(())
            }
        }
    }

    pub fn send_async_runtime(
        &self,
        endpoint: Endpoint,
        data: Vec<u8>,
        token: String,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let observers = self.observers.clone();

        tokio::spawn(async move {
            Engine::send_async(endpoint, data, token, observers).await;
        });
        
        Ok(())
    }

    async fn send_async(
        endpoint: Endpoint,
        data: Vec<u8>,
        token: String,
        observers: Vec<Arc<Mutex<dyn EngineObserver + Send + Sync>>>,
    ) {
        notify_all_observers(
            &observers,
            &SocketEngineEvent::Data(DataEvent::Sending { 
                message_id: token.clone(), 
                to: endpoint.clone(), 
                bytes: data.len() 
            }),
        );

        match &endpoint {
            Endpoint::Tcp(addr) => {
                TcpSender::send(addr.clone(), data, token, endpoint, observers).await;
            }
            Endpoint::Udp(addr) => {
                UdpSender::send(addr.clone(), data, token, endpoint, observers).await;
            }
            Endpoint::Bp(_) => {
                BpSender::send(data, token, endpoint, observers);
            }
        }
    }
}
