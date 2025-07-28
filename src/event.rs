use std::sync::{Arc, Mutex};

use crate::endpoint::Endpoint;

#[cfg(feature = "with_delay")]
use crate::engine::TOKIO_RUNTIME;
#[cfg(feature = "with_delay")]
use std::env;
#[cfg(feature = "with_delay")]
use tokio::time::{sleep, Duration};

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
    Sending {
        message_id: String,
        to: Endpoint,
        bytes: usize,
    },
    Sent {
        message_id: String,
        to: Endpoint,
        bytes_sent: usize,
    },
}

#[derive(Clone, Debug)]
pub enum ConnectionEvent {
    ListenerStarted { endpoint: Endpoint },
    Established { remote: Endpoint },
    Closed { remote: Option<Endpoint> },
}

#[derive(Clone, Debug)]
pub enum ErrorEvent {
    ConnectionFailed {
        endpoint: Endpoint,
        reason: ConnectionFailureReason,
        token: String,
    },
    SendFailed {
        endpoint: Endpoint,
        token: String,
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
    #[cfg(feature = "with_delay")]
    let delay_ms = env::var("ENGINE_RECEIVE_DELAY_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(1000);
    for obs in observers {
        #[cfg(feature = "with_delay")]
        {
            if let SocketEngineEvent::Data(DataEvent::Received { .. }) = event {
                let obs_clone = obs.clone();
                let event_clone = event.clone();
                TOKIO_RUNTIME.spawn(async move {
                    sleep(Duration::from_millis(delay_ms)).await;
                    obs_clone.lock().unwrap().on_engine_event(event_clone);
                });
                continue;
            }
        }
        obs.lock().unwrap().on_engine_event(event.clone());
    }
}
