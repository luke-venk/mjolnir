use if_addrs::{IfAddr, get_if_addrs};
use std::fmt;
use std::net::IpAddr;

#[derive(Debug)]
pub enum ResolveError {
    InterfaceNotFound,
    NoIpOnInterface,
    TooManyIpOnInterface,
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolveError::InterfaceNotFound => write!(f, "interface not found"),
            ResolveError::NoIpOnInterface => write!(f, "interface has no IP address"),
            ResolveError::TooManyIpOnInterface => {
                write!(f, "interface had multiple IP address options")
            }
        }
    }
}

pub fn resolve_iface_to_ip(iface_name: &str) -> Result<IpAddr, ResolveError> {
    let ifaces = get_if_addrs().map_err(|_| ResolveError::InterfaceNotFound)?;
    let mut candidates = Vec::new();
    for iface in ifaces {
        if iface.name != iface_name {
            continue;
        }
        if let IfAddr::V4(v4) = iface.addr {
            candidates.push(IpAddr::V4(v4.ip));
        }
    }
    if candidates.is_empty() {
        return Err(ResolveError::NoIpOnInterface);
    }
    if candidates.len() > 1 {
        return Err(ResolveError::TooManyIpOnInterface);
    }

    Ok(candidates[0])
}
