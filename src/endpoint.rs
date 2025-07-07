use socket2::SockAddr;
use std::{
    io::{self, Error, ErrorKind},
    mem::{self, ManuallyDrop},
    ptr,
};

use crate::socket::AF_BP;
#[derive(Clone, Debug)]
pub enum Endpoint {
    Udp(String),
    Tcp(String),
    Bp(String),
}

impl Endpoint {
    pub fn to_string(&self) -> String {
        match self {
            Endpoint::Udp(s) => s.clone(),
            Endpoint::Tcp(s) => s.clone(),
            Endpoint::Bp(s) => s.clone(),
        }
    }

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
}

const BP_SCHEME_IPN: u32 = 1;
// const BP_SCHEME_DTN: u32 = 2;

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

pub fn create_bp_sockaddr_with_string(eid_string: &str) -> io::Result<SockAddr> {
    if eid_string.is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "EID string cannot be empty",
        ));
    }

    // ---- Handle "ipn:" scheme ----
    if let Some(eid_body) = eid_string.strip_prefix("ipn:") {
        let parts: Vec<&str> = eid_body.split('.').collect();
        if parts.len() != 2 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("Invalid IPN EID format: {}", eid_string),
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
    else if eid_string.starts_with("dtn:") {
        Err(Error::new(
            ErrorKind::Unsupported,
            "DTN scheme not yet implemented",
        ))
    } else {
        Err(Error::new(
            ErrorKind::InvalidInput,
            format!("Unsupported scheme in EID: {}", eid_string),
        ))
    }
}
