use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub enum SocketEngineEvent {
    Reception(Vec<u8>),
    Sent(String),
    SentError((String, String)), // err msg, uuid
                                 //TODO: Info / Error
}

pub trait EngineObserver: Send + Sync {
    fn notify(&mut self, event: SocketEngineEvent);
}

pub fn notify_all(
    observers: &Vec<Arc<Mutex<dyn EngineObserver + Send + Sync>>>,
    event: &SocketEngineEvent,
) {
    for obs in observers {
        obs.lock().unwrap().notify(event.clone());
    }
}
