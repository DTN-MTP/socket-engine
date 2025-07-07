pub enum SocketEngineEvent {
    Reception(Vec<u8>),
    SentError((String, String)), // err msg, uuid
}

pub trait EngineObserver: Send + Sync {
    fn notify(&mut self, event: SocketEngineEvent);
}
