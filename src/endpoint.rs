use socket2::SockAddr;
use std::{
    fmt,
    io::{self, Error, ErrorKind},
    mem::{self, ManuallyDrop},
    ptr,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum EndpointProto {
    Udp,
    Tcp,
    Bp,
}
impl EndpointProto {
    pub fn to_string(&self) -> String {
        match self {
            EndpointProto::Udp => format!("udp").to_string(),
            EndpointProto::Tcp => format!("tcp").to_string(),
            EndpointProto::Bp => format!("bp").to_string(),
        }
    }
}

impl fmt::Display for EndpointProto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

use crate::socket::AF_BP;
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Endpoint {
    pub proto: EndpointProto,
    pub endpoint: String,
}

impl Endpoint {
    pub fn from_str(input: &str) -> Result<Self, String> {
        // Split into scheme and addr parts
        let mut parts = input.splitn(2, ' ');
        let scheme = parts.next().ok_or("Missing scheme")?;
        let addr = parts.next().ok_or("Missing address")?;

        match scheme.to_lowercase().as_str() {
            "bp" => Ok(Endpoint {
                proto: EndpointProto::Bp,
                endpoint: addr.to_string(),
            }),
            "tcp" => Ok(Endpoint {
                proto: EndpointProto::Tcp,
                endpoint: addr.to_string(),
            }),
            "udp" => Ok(Endpoint {
                proto: EndpointProto::Udp,
                endpoint: addr.to_string(),
            }),
            _ => Err(format!("Unsupported scheme: {}", scheme)),
        }
    }
    pub fn to_string(&self) -> String {
        format!("{} {}", self.proto.to_string(), self.endpoint)
    }
}

impl fmt::Display for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

const BP_SCHEME_IPN: u32 = 1;
// const BP_SCHEME_DTN: u32 = 2;

#[repr(C)]
pub struct SockAddrBp {
    bp_family: libc::sa_family_t,
    bp_scheme: u32,
    bp_addr: BpAddr,
}

impl std::fmt::Display for SockAddrBp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sch = if self.bp_scheme == BP_SCHEME_IPN {
            "ipn"
        } else {
            "??"
        };
        match self.bp_scheme {
            BP_SCHEME_IPN => {
                let ipn_addr = unsafe { &*self.bp_addr.ipn };
                write!(f, "{}:{}.{}", sch, ipn_addr.node_id, ipn_addr.service_id)
            }
            _ => {
                write!(f, "scheme {} unknown", sch)
            }
        }
    }
}
#[repr(C)]
pub union BpAddr {
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
