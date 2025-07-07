pub enum SocketEngineEvent {
    Reception(Vec<u8>),
}

pub trait EngineObserver: Send + Sync {
    fn notify(&mut self, event: SocketEngineEvent);
}
