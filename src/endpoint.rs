use socket2::SockAddr;
use std::{
    fmt,
    io::{self, Error, ErrorKind},
    mem::{self, ManuallyDrop},
    ptr,
};

use crate::constants::protocol::{AF_BP, BP_SCHEME_IPN};
#[derive(Clone, Debug, PartialEq)]
pub enum Endpoint {
    Udp(String),
    Tcp(String),
    Bp(String),
}

impl Endpoint {
    pub fn from_str(input: &str) -> Result<Self, String> {
        // Split into scheme and addr parts
        let mut parts = input.splitn(2, ' ');
        let scheme = parts.next().ok_or("Missing scheme")?;
        let addr = parts.next().ok_or("Missing address")?;

        match scheme.to_lowercase().as_str() {
            "bp" => Ok(Endpoint::Bp(addr.to_string())),
            "tcp" => Ok(Endpoint::Tcp(addr.to_string())),
            "udp" => Ok(Endpoint::Udp(addr.to_string())),
            _ => Err(format!("Unsupported scheme: {}", scheme)),
        }
    }
    pub fn to_string(&self) -> String{
        match self {
            Endpoint::Udp(addr) => format!("udp {}", addr).to_string(),
            Endpoint::Tcp(addr) => format!("tcp {}", addr).to_string(),
            Endpoint::Bp(addr) => format!("bp {}", addr).to_string(),
        }
    }
}

impl fmt::Display for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Endpoint::Udp(s) => write!(f, "{}", s),
            Endpoint::Tcp(s) => write!(f, "{}", s),
            Endpoint::Bp(s) => write!(f, "{}", s),
        }
    }
}

#[repr(C)]
struct SockAddrBp {
    bp_family: libc::sa_family_t,
    bp_scheme: u32,
    bp_addr: BpAddr,
}

#[repr(C)]
union BpAddr {
    ipn: ManuallyDrop<IpnAddr>,
    // Extend with other schemes like DTN if needed
}

#[repr(C)]
struct IpnAddr {
    node_id: u32,
    service_id: u32,
}

pub fn create_bp_sockaddr_with_string(endpoint_string: &str) -> io::Result<SockAddr> {
    if endpoint_string.is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "Endpoint string cannot be empty",
        ));
    }

    // ---- Handle "ipn:" scheme ----
    if let Some(endpoint_body) = endpoint_string.strip_prefix("ipn:") {
        let parts: Vec<&str> = endpoint_body.split('.').collect();
        if parts.len() != 2 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("Invalid IPN endpoint format: {}", endpoint_string),
            ));
        }

        let node_id: u32 = parts[0]
            .parse()
            .map_err(|_| Error::new(ErrorKind::InvalidInput, "Invalid node ID"))?;
        let service_id: u32 = parts[1]
            .parse()
            .map_err(|_| Error::new(ErrorKind::InvalidInput, "Invalid service ID"))?;

        let sockaddr_bp = SockAddrBp {
            bp_family: AF_BP as libc::sa_family_t,
            bp_scheme: BP_SCHEME_IPN,
            bp_addr: BpAddr {
                ipn: ManuallyDrop::new(IpnAddr {
                    node_id,
                    service_id,
                }),
            },
        };

        let mut sockaddr_storage: libc::sockaddr_storage = unsafe { mem::zeroed() };
        unsafe {
            ptr::copy_nonoverlapping(
                &sockaddr_bp as *const SockAddrBp as *const u8,
                &mut sockaddr_storage as *mut _ as *mut u8,
                mem::size_of::<SockAddrBp>(),
            );
        }

        let addr_len = mem::size_of::<SockAddrBp>() as libc::socklen_t;
        let address = unsafe { SockAddr::new(sockaddr_storage, addr_len) };
        Ok(address)
    }
    // ---- Handle unsupported or unimplemented schemes ----
    else if endpoint_string.starts_with("dtn:") {
        Err(Error::new(
            ErrorKind::Unsupported,
            "DTN scheme not yet implemented",
        ))
    } else {
        Err(Error::new(
            ErrorKind::InvalidInput,
            format!("Unsupported scheme in endpoint: {}", endpoint_string),
        ))
    }
}
