//! Windows ICMP echo. Direct host reachability, separate from proxy latency.
use crate::{Error, Result};
use std::{net::IpAddr, time::Duration};
pub fn sample(host: &str) -> Result<f64> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| Error::io("icmp-dns", e))?;
    let ip = rt.block_on(async {
        let addresses =
            tokio::time::timeout(Duration::from_secs(4), tokio::net::lookup_host((host, 0)))
                .await
                .map_err(|_| Error::new("LatencyTimeout", "icmp-dns", "DNS lookup timed out"))?
                .map_err(|e| Error::io("icmp-dns", e))?;
        addresses
            .map(|a| a.ip())
            .next()
            .ok_or_else(|| Error::new("LatencyFailed", "icmp-dns", "No server address"))
    })?;
    echo(ip)
}
#[cfg(windows)]
fn echo(ip: IpAddr) -> Result<f64> {
    use windows_sys::Win32::{
        Foundation::{HANDLE, INVALID_HANDLE_VALUE},
        NetworkManagement::IpHelper::*,
        Networking::WinSock::*,
    };
    struct Handle(HANDLE);
    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe { IcmpCloseHandle(self.0) };
        }
    }
    let raw = unsafe {
        if ip.is_ipv4() {
            IcmpCreateFile()
        } else {
            Icmp6CreateFile()
        }
    };
    if raw == INVALID_HANDLE_VALUE || raw.is_null() {
        return Err(Error::io("icmp-open", std::io::Error::last_os_error()));
    }
    let handle = Handle(raw);
    let payload = *b"NetPulse";
    let mut buffer = [0u64; 64];
    let count = unsafe {
        match ip {
            IpAddr::V4(ip) => IcmpSendEcho(
                handle.0,
                u32::from_ne_bytes(ip.octets()),
                payload.as_ptr() as _,
                payload.len() as u16,
                std::ptr::null(),
                buffer.as_mut_ptr() as _,
                std::mem::size_of_val(&buffer) as u32,
                4000,
            ),
            IpAddr::V6(ip) => {
                let source = SOCKADDR_IN6 {
                    sin6_family: AF_INET6,
                    ..Default::default()
                };
                let mut destination = source;
                destination.sin6_addr.u.Byte = ip.octets();
                Icmp6SendEcho2(
                    handle.0,
                    std::ptr::null_mut(),
                    None,
                    std::ptr::null(),
                    &source,
                    &destination,
                    payload.as_ptr() as _,
                    payload.len() as u16,
                    std::ptr::null(),
                    buffer.as_mut_ptr() as _,
                    std::mem::size_of_val(&buffer) as u32,
                    4000,
                )
            }
        }
    };
    if count == 0 {
        let e = std::io::Error::last_os_error();
        return Err(Error::new(
            if e.raw_os_error() == Some(11010) {
                "LatencyTimeout"
            } else {
                "IcmpFailed"
            },
            "icmp-echo",
            format!(
                "ICMP echo failed ({})",
                e.raw_os_error().unwrap_or_default()
            ),
        ));
    }
    let (status, time) = unsafe {
        if ip.is_ipv4() {
            let reply = &*(buffer.as_ptr() as *const ICMP_ECHO_REPLY);
            (reply.Status, reply.RoundTripTime)
        } else {
            let reply = &*(buffer.as_ptr() as *const ICMPV6_ECHO_REPLY_LH);
            (reply.Status, reply.RoundTripTime)
        }
    };
    if status != 0 {
        return Err(Error::new(
            if status == 11010 {
                "LatencyTimeout"
            } else {
                "IcmpFailed"
            },
            "icmp-echo",
            format!("ICMP status {status}"),
        ));
    }
    Ok(time as f64)
}
#[cfg(not(windows))]
fn echo(_: IpAddr) -> Result<f64> {
    Err(Error::new(
        "PlatformUnsupported",
        "icmp-echo",
        "Windows ICMP API required",
    ))
}
#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn owned_ipv4_and_ipv6_echo() {
        assert!(echo("127.0.0.1".parse().unwrap()).unwrap() >= 0.0);
        assert!(echo("::1".parse().unwrap()).unwrap() >= 0.0);
    }
}
