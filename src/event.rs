use std::sync::{Arc, Mutex};

use crate::endpoint::Endpoint;

#[derive(Clone)]
pub enum SocketEngineEvent {
    Info(EventSocket),
    Error(ErrorEventSocket),
}

#[derive(Clone, Debug)]
pub enum EventSocket {
    Tcp(TcpEvent),
    Udp(UdpEvent),
    General(GeneralSocketEvent),
}

#[derive(Clone, Debug)]
pub enum GeneralSocketEvent {
    DataReceived(Vec<u8>, Endpoint),
    DataSent(String, Endpoint),
    
}

#[derive(Clone, Debug)]
pub enum ErrorEventSocket {
    Tcp(TcpErrorEvent),
    General(GeneralSocketErrorEvent),
}
#[derive(Clone, Debug)]
pub enum GeneralSocketErrorEvent {
    ConnectionError(String, Endpoint),
    SendFailed(String, String, Endpoint),
    ReceiveFailed(String, Endpoint),
}

#[derive(Clone, Debug)]
pub enum TcpErrorEvent {
    ConnectionRefused(String, Endpoint),
    ConnectionTimeout(String, Endpoint),
}

#[derive(Clone, Debug)]
pub enum UdpEvent {
    PacketSizeSent(u64),
    PacketSizeReceived(u64),
}

#[derive(Clone, Debug)]
pub enum TcpEvent {
    ConnectionEstablished(String),
    
    ListenerStarted(String),
    ConnectionClosed(String),
}

pub trait EngineObserver: Send + Sync {
    fn on_engine_event(&mut self, event: SocketEngineEvent);
}

pub fn notify_all_observers(
    observers: &Vec<Arc<Mutex<dyn EngineObserver + Send + Sync>>>,
    event: &SocketEngineEvent,
) {
    for obs in observers {
        obs.lock().unwrap().on_engine_event(event.clone());
    }
}
