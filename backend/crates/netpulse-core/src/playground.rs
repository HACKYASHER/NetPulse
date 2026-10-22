//! Read-only OS socket tables and bounded core metadata. No capture driver or DB.
use crate::{Error, Result};
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
pub struct Process {
    pub pid: u32,
    pub name: String,
    pub path: String,
}
#[derive(Serialize, Debug, Clone)]
pub struct Connection {
    pub id: String,
    pub source: String,
    pub process: String,
    pub pid: Option<u32>,
    pub network: String,
    pub local: String,
    pub destination: String,
    pub state: String,
    pub upload: Option<u64>,
    pub download: Option<u64>,
    pub route: String,
}
#[derive(Serialize)]
pub struct Analysis {
    pub connections: Vec<Connection>,
    pub process_alive: bool,
    pub errors: Vec<String>,
    pub core_available: bool,
    pub truncated: bool,
}
fn text(v: &Value, key: &str) -> String {
    v[key]
        .as_str()
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_control())
        .take(512)
        .collect()
}
pub fn core_connections(value: &Value, path: &str) -> Vec<Connection> {
    value["connections"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| {
            let m = &c["metadata"];
            let mut process = text(m, "processPath");
            if !path.is_empty()
                && process
                    .get(..path.len())
                    .is_some_and(|p| p.eq_ignore_ascii_case(path))
                && process
                    .get(path.len()..)
                    .is_some_and(|s| s.starts_with(" (") && s.ends_with(')'))
            {
                process = path.into();
            }
            if !path.is_empty() && !process.eq_ignore_ascii_case(path) {
                return None;
            }
            let host = text(m, "host");
            let dest = if host.is_empty() {
                text(m, "destinationIP")
            } else {
                host
            };
            Some(Connection {
                id: text(c, "id"),
                source: "core".into(),
                process,
                pid: m["pid"].as_u64().and_then(|p| u32::try_from(p).ok()),
                network: text(m, "network"),
                local: match (
                    text(m, "sourceIP").parse::<std::net::IpAddr>(),
                    text(m, "sourcePort").parse::<u16>(),
                ) {
                    (Ok(ip), Ok(port)) => std::net::SocketAddr::new(ip, port).to_string(),
                    _ => format!("{}:{}", text(m, "sourceIP"), text(m, "sourcePort")),
                },
                destination: format!("{}:{}", dest, text(m, "destinationPort")),
                state: "active".into(),
                upload: c["upload"].as_u64(),
                download: c["download"].as_u64(),
                route: c["chains"]
                    .as_array()
                    .map(|v| {
                        v.iter()
                            .filter_map(Value::as_str)
                            .take(8)
                            .map(|s| s.chars().take(128).collect::<String>())
                            .collect::<Vec<_>>()
                            .join(" → ")
                    })
                    .unwrap_or_default(),
            })
        })
        .take(300)
        .collect()
}

#[cfg(windows)]
fn process_path(pid: u32) -> Option<String> {
    use windows_sys::Win32::{Foundation::CloseHandle, System::Threading::*};
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return None;
    }
    let mut buffer = vec![0u16; 32768];
    let mut size = buffer.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut size) };
    unsafe {
        CloseHandle(handle);
    }
    (ok != 0).then(|| String::from_utf16_lossy(&buffer[..size as usize]))
}
#[cfg(windows)]
pub fn processes() -> Result<Vec<Process>> {
    use windows_sys::Win32::{Foundation::*, System::Diagnostics::ToolHelp::*};
    let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snap == INVALID_HANDLE_VALUE {
        return Err(Error::io(
            "playground-processes",
            std::io::Error::last_os_error(),
        ));
    }
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut rows = Vec::new();
    let mut next = unsafe { Process32FirstW(snap, &mut entry) };
    while next != 0 && rows.len() < 4096 {
        if let Some(path) = process_path(entry.th32ProcessID) {
            let name = std::path::Path::new(&path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            rows.push(Process {
                pid: entry.th32ProcessID,
                name,
                path,
            });
        }
        next = unsafe { Process32NextW(snap, &mut entry) };
    }
    unsafe {
        CloseHandle(snap);
    }
    rows.sort_by_key(|p| (p.name.to_lowercase(), p.pid));
    Ok(rows)
}
#[cfg(not(windows))]
pub fn processes() -> Result<Vec<Process>> {
    Ok(vec![])
}

#[cfg(windows)]
fn table<T: Copy>(family: u32, tcp: bool) -> Result<Vec<T>> {
    use windows_sys::Win32::{
        Foundation::ERROR_INSUFFICIENT_BUFFER, NetworkManagement::IpHelper::*,
    };
    let mut size = 0u32;
    for _ in 0..3 {
        if size > 4 * 1024 * 1024 {
            break;
        }
        let mut buffer = vec![0u64; (size as usize).div_ceil(8).max(1)];
        let code = unsafe {
            if tcp {
                GetExtendedTcpTable(
                    buffer.as_mut_ptr().cast(),
                    &mut size,
                    0,
                    family,
                    TCP_TABLE_OWNER_PID_ALL,
                    0,
                )
            } else {
                GetExtendedUdpTable(
                    buffer.as_mut_ptr().cast(),
                    &mut size,
                    0,
                    family,
                    UDP_TABLE_OWNER_PID,
                    0,
                )
            }
        };
        if code == ERROR_INSUFFICIENT_BUFFER {
            continue;
        }
        if code != 0 {
            return Err(Error::new(
                "SocketTableUnavailable",
                "playground-sockets",
                format!("Windows socket table returned {code}"),
            ));
        }
        let bytes = buffer.len() * 8;
        if size < 4 || size as usize > bytes {
            break;
        }
        let count = buffer[0] as u32 as usize;
        let row_size = std::mem::size_of::<T>();
        if count > (size as usize - 4) / row_size {
            break;
        }
        // Windows table header is a DWORD; rows may be unaligned for T.
        return Ok((0..count.min(10000))
            .map(|i| unsafe {
                std::ptr::read_unaligned(
                    buffer
                        .as_ptr()
                        .cast::<u8>()
                        .add(4 + i * row_size)
                        .cast::<T>(),
                )
            })
            .collect());
    }
    Err(Error::new(
        "SocketTableChanged",
        "playground-sockets",
        "Таблица соединений изменилась; повторите наблюдение",
    ))
}
#[cfg(windows)]
pub fn sockets(pid: u32, path: &str) -> Result<(bool, Vec<Connection>)> {
    use std::net::{Ipv4Addr, Ipv6Addr};
    use windows_sys::Win32::{
        NetworkManagement::IpHelper::*,
        Networking::WinSock::{AF_INET, AF_INET6},
    };
    if process_path(pid).is_none_or(|p| !p.eq_ignore_ascii_case(path)) {
        return Ok((false, vec![]));
    }
    let mut rows = Vec::new();
    let port = |p: u32| u16::from_be(p as u16);
    let state = |s: u32| {
        match s {
            1 => "closed",
            2 => "listen",
            3 => "syn_sent",
            4 => "syn_received",
            5 => "established",
            6 => "fin_wait_1",
            7 => "fin_wait_2",
            8 => "close_wait",
            9 => "closing",
            10 => "last_ack",
            11 => "time_wait",
            12 => "delete_tcb",
            _ => "unknown",
        }
        .to_string()
    };
    let mut add = |network: &str, local: String, destination: String, state: String| {
        let local = local.replace("%0]", "]");
        let destination = destination.replace("%0]", "]");
        if rows.len() < 300 {
            rows.push(Connection {
                id: format!("os:{pid}:{network}:{local}:{destination}"),
                source: "windows".into(),
                process: path.into(),
                pid: Some(pid),
                network: network.into(),
                local,
                destination,
                state,
                upload: None,
                download: None,
                route: "".into(),
            });
        }
    };
    for r in table::<MIB_TCPROW_OWNER_PID>(AF_INET as u32, true)?
        .into_iter()
        .filter(|r| r.dwOwningPid == pid)
    {
        add(
            "tcp",
            format!(
                "{}:{}",
                Ipv4Addr::from(r.dwLocalAddr.to_ne_bytes()),
                port(r.dwLocalPort)
            ),
            format!(
                "{}:{}",
                Ipv4Addr::from(r.dwRemoteAddr.to_ne_bytes()),
                port(r.dwRemotePort)
            ),
            state(r.dwState),
        );
    }
    for r in table::<MIB_TCP6ROW_OWNER_PID>(AF_INET6 as u32, true)?
        .into_iter()
        .filter(|r| r.dwOwningPid == pid)
    {
        add(
            "tcp",
            format!(
                "[{}%{}]:{}",
                Ipv6Addr::from(r.ucLocalAddr),
                r.dwLocalScopeId,
                port(r.dwLocalPort)
            ),
            format!(
                "[{}%{}]:{}",
                Ipv6Addr::from(r.ucRemoteAddr),
                r.dwRemoteScopeId,
                port(r.dwRemotePort)
            ),
            state(r.dwState),
        );
    }
    for r in table::<MIB_UDPROW_OWNER_PID>(AF_INET as u32, false)?
        .into_iter()
        .filter(|r| r.dwOwningPid == pid)
    {
        add(
            "udp",
            format!(
                "{}:{}",
                Ipv4Addr::from(r.dwLocalAddr.to_ne_bytes()),
                port(r.dwLocalPort)
            ),
            "".into(),
            "bound".into(),
        );
    }
    for r in table::<MIB_UDP6ROW_OWNER_PID>(AF_INET6 as u32, false)?
        .into_iter()
        .filter(|r| r.dwOwningPid == pid)
    {
        add(
            "udp",
            format!(
                "[{}%{}]:{}",
                Ipv6Addr::from(r.ucLocalAddr),
                r.dwLocalScopeId,
                port(r.dwLocalPort)
            ),
            "".into(),
            "bound".into(),
        );
    }
    Ok((true, rows))
}
#[cfg(not(windows))]
pub fn sockets(_pid: u32, _path: &str) -> Result<(bool, Vec<Connection>)> {
    Ok((false, vec![]))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_is_bounded_filtered_and_never_forwards_unknown_fields() {
        let v = serde_json::json!({"connections":[{"id":"one","metadata":{"processPath":"C:\\App.exe","host":"fixture.test","network":"tcp","destinationPort":"443","password":"secret"},"upload":10,"download":20,"chains":["proxy"]},{"id":"two","metadata":{"processPath":"C:\\Other.exe"}}]});
        let rows = core_connections(&v, "c:\\APP.exe");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].destination, "fixture.test:443");
        assert!(!serde_json::to_string(&rows).unwrap().contains("secret"));
        assert!(core_connections(&Value::Null, "").is_empty());
    }
    #[cfg(windows)]
    #[test]
    fn observes_owned_tcp_udp_ipv6_and_rejects_a_different_executable() {
        let tcp = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let udp = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        let tcp6 = std::net::TcpListener::bind("[::1]:0").unwrap();
        let udp6 = std::net::UdpSocket::bind("[::1]:0").unwrap();
        let pid = std::process::id();
        let path = process_path(pid).unwrap();
        let (alive, rows) = sockets(pid, &path).unwrap();
        assert!(alive);
        assert!(
            rows.iter()
                .any(|r| r.local == tcp.local_addr().unwrap().to_string() && r.state == "listen")
        );
        assert!(
            rows.iter()
                .any(|r| r.local == udp.local_addr().unwrap().to_string() && r.network == "udp")
        );
        assert!(!sockets(pid, "C:\\Different.exe").unwrap().0);
        assert!(
            rows.iter()
                .any(|r| r.local == tcp6.local_addr().unwrap().to_string() && r.state == "listen")
        );
        assert!(
            rows.iter()
                .any(|r| r.local == udp6.local_addr().unwrap().to_string() && r.network == "udp")
        );
    }
}
