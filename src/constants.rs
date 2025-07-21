pub mod protocol {
    use libc::c_int;
    pub const AF_BP: c_int = 28;
    pub const BP_SCHEME_IPN: u32 = 1;
}

pub mod buffer {
    pub const TCP_BUFFER_SIZE: usize = 4096;
    pub const UDP_MAX_DATAGRAM_SIZE: usize = 65507;
}

pub mod timeout {
    use std::time::Duration;

    pub const DEFAULT_CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);
    pub const POLLING_INTERVAL: Duration = Duration::from_millis(10);
}
