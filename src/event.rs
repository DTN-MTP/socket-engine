pub enum SocketEngineEvent {
    Reception(Vec<u8>),
    Sent(String),
}

pub trait EngineObserver: Send + Sync {
    fn notify(&mut self, event: SocketEngineEvent);
}
