use std::{fmt::{Display, Formatter, Result}, sync::{Arc, Mutex}};

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

impl DataEvent {
    pub fn endpoint(&self) -> &Endpoint {
        match self {
            Self::Received { from, .. } => from,
            Self::Sending { to, .. } | Self::Sent { to, .. } => to,
        }
    }
    
    pub fn byte_count(&self) -> usize {
        match self {
            Self::Received { data, .. } => data.len(),
            Self::Sending { bytes, .. } => *bytes,
            Self::Sent { bytes_sent, .. } => *bytes_sent,
        }
    }
}

#[derive(Clone, Debug)]
pub enum ConnectionEvent {
    ListenerStarted { endpoint: Endpoint },
    Established { remote: Endpoint },
    Closed { remote: Option<Endpoint> },
}

impl ConnectionEvent {
    pub fn endpoint(&self) -> Option<&Endpoint> {
        match self {
            Self::ListenerStarted { endpoint } => Some(endpoint),
            Self::Established { remote } => Some(remote),
            Self::Closed { remote } => remote.as_ref(),
        }
    }
    
    pub fn is_connection_established(&self) -> bool {
        matches!(self, Self::Established { .. })
    }
    
    pub fn is_connection_closed(&self) -> bool {
        matches!(self, Self::Closed { .. })
    }
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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ConnectionFailureReason {
    Refused,
    Timeout,
    NetworkUnreachable,
    Other,
}

impl ConnectionFailureReason {

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Refused => "connection refused",
            Self::Timeout => "timeout",
            Self::NetworkUnreachable => "network unreachable",
            Self::Other => "other",
        }
    }
}

impl Display for ConnectionFailureReason {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", self.as_str())
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
        if let Ok(mut observer) = obs.lock() {
            observer.on_engine_event(event.clone());
        }
        // Note: On ignore silencieusement les échecs de verrouillage
        // pour éviter de bloquer le système en cas d'observateur défaillant
    }
}
