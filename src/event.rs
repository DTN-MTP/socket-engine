use std::sync::{Arc, Mutex};

use crate::endpoint::Endpoint;

#[derive(Clone, Debug)]
pub enum SocketEngineEvent {
    Data(DataEvent),
    Connection(ConnectionEvent),
    Error(ErrorEvent),
}

#[derive(Clone, Debug)]
pub enum DataEvent {
    Received {
        data: Vec<u8>,
        from: Endpoint,
    },
    Sent {
        message_id: String,
        to: Endpoint,
        bytes_sent: usize,
    },
}

#[derive(Clone, Debug)]
pub enum ConnectionEvent {
    ListenerStarted {
        endpoint: Endpoint,
    },
    Established {
        remote: Endpoint,
    },
    Closed {
        remote: Option<Endpoint>,
    },
}

#[derive(Clone, Debug)]
pub enum ErrorEvent {
    ConnectionFailed {
        endpoint: Endpoint,
        reason: ConnectionFailureReason,
        message: String,
    },
    SendFailed {
        endpoint: Endpoint,
        message_id: String,
        reason: String,
    },
    ReceiveFailed {
        endpoint: Endpoint,
        reason: String,
    },
    SocketError {
        endpoint: Endpoint,
        reason: String,
    },
}

#[derive(Copy, Clone, Debug)]
pub enum ConnectionFailureReason {
    Refused,
    Timeout,
    NetworkUnreachable,
    Other,
}

impl ConnectionFailureReason {
    pub fn from_io_error_kind(kind: std::io::ErrorKind) -> Self {
        match kind {
            std::io::ErrorKind::ConnectionRefused => Self::Refused,
            std::io::ErrorKind::TimedOut => Self::Timeout,
            std::io::ErrorKind::NetworkUnreachable => Self::NetworkUnreachable,
            _ => Self::Other,
        }
    }
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
