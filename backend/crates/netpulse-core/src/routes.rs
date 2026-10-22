//! Inspect IPv4/IPv6 routes through IP Helper. Never flush a system route table.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use windows_sys::Win32::{NetworkManagement::IpHelper::*, Networking::WinSock::*};
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Interface {
    pub luid: u64,
    pub index: u32,
    pub guid: String,
    pub alias: String,
    pub up: bool,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub kind: u32,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct Route {
    pub luid: u64,
    pub index: u32,
    pub destination: String,
    pub prefix: u8,
    pub gateway: String,
    pub metric: u32,
    pub protocol: i32,
    pub origin: i32,
    pub loopback: bool,
    pub publish: bool,
    pub immortal: bool,
    pub autoconfigure: bool,
    pub site_prefix: u8,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NetworkView {
    pub interfaces: Vec<Interface>,
    pub routes: Vec<Route>,
}
pub fn interface_has_ipv6_default(view: &NetworkView, alias: &str) -> bool {
    view.interfaces
        .iter()
        .filter(|i| i.up && i.alias == alias)
        .any(|i| {
            view.routes
                .iter()
                .any(|r| r.luid == i.luid && r.destination == "::" && r.prefix == 0 && !r.loopback)
        })
}
fn ip(addr: &SOCKADDR_INET) -> String {
    unsafe {
        if addr.si_family == AF_INET {
            std::net::Ipv4Addr::from(addr.Ipv4.sin_addr.S_un.S_addr.to_ne_bytes()).to_string()
        } else if addr.si_family == AF_INET6 {
            std::net::Ipv6Addr::from(addr.Ipv6.sin6_addr.u.Byte).to_string()
        } else {
            "unknown".into()
        }
    }
}
fn route(row: &MIB_IPFORWARD_ROW2) -> Route {
    Route {
        luid: unsafe { row.InterfaceLuid.Value },
        index: row.InterfaceIndex,
        destination: ip(&row.DestinationPrefix.Prefix),
        prefix: row.DestinationPrefix.PrefixLength,
        gateway: ip(&row.NextHop),
        metric: row.Metric,
        protocol: row.Protocol,
        origin: row.Origin,
        loopback: row.Loopback,
        publish: row.Publish,
        immortal: row.Immortal,
        autoconfigure: row.AutoconfigureAddress,
        site_prefix: row.SitePrefixLength,
    }
}
fn native_routes() -> Result<Vec<MIB_IPFORWARD_ROW2>> {
    let mut ptr = std::ptr::null_mut();
    let code = unsafe { GetIpForwardTable2(AF_UNSPEC, &mut ptr) };
    if code != 0 {
        return Err(Error::new(
            "WindowsRoutingError",
            "routes-read",
            format!("GetIpForwardTable2 returned {code}"),
        ));
    }
    let rows =
        unsafe { std::slice::from_raw_parts((*ptr).Table.as_ptr(), (*ptr).NumEntries as usize) }
            .to_vec();
    unsafe { FreeMibTable(ptr as _) };
    Ok(rows)
}
pub fn inspect() -> Result<NetworkView> {
    let mut ptr = std::ptr::null_mut();
    let code = unsafe { GetIfTable2(&mut ptr) };
    if code != 0 {
        return Err(Error::new(
            "WindowsInterfaceError",
            "interfaces-read",
            format!("GetIfTable2 returned {code}"),
        ));
    }
    let rows =
        unsafe { std::slice::from_raw_parts((*ptr).Table.as_ptr(), (*ptr).NumEntries as usize) };
    let interfaces = rows
        .iter()
        .map(|r| {
            let end = r
                .Alias
                .iter()
                .position(|v| *v == 0)
                .unwrap_or(r.Alias.len());
            Interface {
                luid: unsafe { r.InterfaceLuid.Value },
                index: r.InterfaceIndex,
                guid: format!(
                    "{:08x}-{:04x}-{:04x}-{:02x?}",
                    r.InterfaceGuid.data1,
                    r.InterfaceGuid.data2,
                    r.InterfaceGuid.data3,
                    r.InterfaceGuid.data4
                ),
                alias: String::from_utf16_lossy(&r.Alias[..end]),
                up: r.OperStatus == 1,
                description: String::from_utf16_lossy(
                    &r.Description[..r
                        .Description
                        .iter()
                        .position(|v| *v == 0)
                        .unwrap_or(r.Description.len())],
                ),
                kind: r.Type,
            }
        })
        .collect();
    unsafe { FreeMibTable(ptr as _) };
    Ok(NetworkView {
        interfaces,
        routes: native_routes()?.iter().map(route).collect(),
    })
}
pub fn ensure_no_tun_conflict() -> Result<NetworkView> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
    loop {
        let view = inspect()?;
        match validate_tun_reuse(&view) {
            Ok(()) => return Ok(view),
            Err(error) if std::time::Instant::now() >= deadline => return Err(error),
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(100)),
        }
    }
}
/// The pinned Windows backend opens an existing Wintun adapter before starting
/// its exclusive session. A dormant adapter is not an active connection.
fn validate_tun_reuse(view: &NetworkView) -> Result<()> {
    for interface in &view.interfaces {
        if interface.alias.eq_ignore_ascii_case("NetPulse") {
            let wintun = interface.kind == 53
                && ["wintun", "sing-tun"]
                    .iter()
                    .any(|name| interface.description.to_ascii_lowercase().contains(name));
            if interface.up || !wintun {
                return Err(Error::new(
                    "TunInterfaceConflict",
                    "tun-prepare",
                    "Адаптер NetPulse занят другим подключением или не является Wintun. Завершите старый клиент и отключите его VPN; чужие адаптеры не удаляются.",
                ));
            }
        }
    }
    Ok(())
}
/// Only rows captured on this session's own LUID/GUID are eligible for deletion.
/// Changes to another route or a modified metric are preserved.
pub fn cleanup_owned(owned: &NetworkView) -> Result<usize> {
    let current = inspect()?;
    let mut removed = 0;
    for row in native_routes()? {
        let record = route(&row);
        let ours = owned
            .interfaces
            .iter()
            .any(|i| i.luid == record.luid && i.alias == "NetPulse");
        let same_interface = current.interfaces.iter().any(|i| {
            i.luid == record.luid
                && owned
                    .interfaces
                    .iter()
                    .any(|o| o.luid == i.luid && o.guid == i.guid && o.alias == "NetPulse")
        });
        if ours && same_interface && owned.routes.contains(&record) {
            let code = unsafe { DeleteIpForwardEntry2(&row) };
            if code != 0 && code != 1168 {
                return Err(Error::new(
                    "RouteRestoreFailed",
                    "tun-cleanup",
                    format!("Deleting an owned route returned Windows error {code}"),
                ));
            }
            removed += 1;
        }
    }
    Ok(removed)
}
pub fn capture_owned() -> Result<NetworkView> {
    let mut view = inspect()?;
    view.interfaces.retain(|i| i.alias == "NetPulse");
    let luids: Vec<_> = view.interfaces.iter().map(|i| i.luid).collect();
    view.routes.retain(|r| luids.contains(&r.luid));
    Ok(view)
}
pub fn default_routes_present(view: &NetworkView) -> bool {
    let present = |destination: &str, prefix: u8| {
        view.routes.iter().any(|r| {
            r.destination == destination
                && r.prefix == prefix
                && view.interfaces.iter().any(|i| i.up && i.luid == r.luid)
        })
    };
    (present("0.0.0.0", 0) || (present("0.0.0.0", 1) && present("128.0.0.0", 1)))
        && (present("::", 0) || (present("::", 1) && present("8000::", 1)))
}
/// Bind diagnostic TCP to the best live default interface outside userspace TUN adapters.
/// IP_UNICAST_IF uses network byte order; IPV6_UNICAST_IF uses host byte order.
pub fn bind_outside_tun(
    socket: &tokio::net::TcpSocket,
    destination: std::net::IpAddr,
) -> std::io::Result<()> {
    use std::os::windows::io::AsRawSocket;
    let view = inspect().map_err(std::io::Error::other)?;
    let selected = diagnostic_interface(&view, destination).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NetworkUnreachable,
            "No default interface outside userspace TUN; use HTTPS through profile",
        )
    })?;
    let value = if destination.is_ipv4() {
        selected.to_be()
    } else {
        selected
    };
    let (level, option) = if destination.is_ipv4() {
        (IPPROTO_IP, IP_UNICAST_IF)
    } else {
        (IPPROTO_IPV6, IPV6_UNICAST_IF)
    };
    if unsafe {
        setsockopt(
            socket.as_raw_socket() as _,
            level,
            option,
            &value as *const u32 as _,
            4,
        )
    } != 0
    {
        return Err(std::io::Error::from_raw_os_error(unsafe {
            WSAGetLastError()
        }));
    }
    Ok(())
}

/// Use the physical/default uplink for transport sockets rather than recapturing
/// an isolated probe or Xray bridge in an already active userspace TUN.
pub fn outbound_interface() -> Option<String> {
    let view = inspect().ok()?;
    let index = diagnostic_interface(&view, "1.1.1.1".parse().ok()?)
        .or_else(|| diagnostic_interface(&view, "2606:4700:4700::1111".parse().ok()?))?;
    view.interfaces
        .iter()
        .find(|i| i.index == index)
        .map(|i| i.alias.clone())
}
fn diagnostic_interface(view: &NetworkView, destination: std::net::IpAddr) -> Option<u32> {
    view.routes
        .iter()
        .filter(|r| r.prefix == 0 && (r.destination == "0.0.0.0") == destination.is_ipv4())
        .filter_map(|r| {
            view.interfaces
                .iter()
                .find(|i| {
                    i.luid == r.luid
                        && i.up
                        && !i.alias.eq_ignore_ascii_case("NetPulse")
                        && !matches!(i.kind, 24 | 53)
                        && !["wintun", "sing-tun"]
                            .iter()
                            .any(|name| i.description.to_ascii_lowercase().contains(name))
                })
                .map(|i| (r, i))
        })
        .min_by_key(|(r, i)| {
            let mut row: MIB_IPINTERFACE_ROW = unsafe { std::mem::zeroed() };
            row.Family = if destination.is_ipv4() {
                AF_INET
            } else {
                AF_INET6
            };
            row.InterfaceIndex = i.index;
            let metric = if unsafe { GetIpInterfaceEntry(&mut row) } == 0 {
                row.Metric
            } else {
                0
            };
            r.metric.saturating_add(metric)
        })
        .map(|(_, i)| i.index)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ipv6_default_must_belong_to_selected_up_physical_interface() {
        let mut view = NetworkView {
            interfaces: vec![Interface {
                luid: 1,
                index: 1,
                guid: "fixture".into(),
                alias: "Ethernet".into(),
                up: true,
                description: "fixture".into(),
                kind: 6,
            }],
            routes: vec![Route {
                luid: 2,
                destination: "::".into(),
                prefix: 0,
                ..Default::default()
            }],
        };
        assert!(!interface_has_ipv6_default(&view, "Ethernet"));
        view.routes[0].luid = 1;
        assert!(interface_has_ipv6_default(&view, "Ethernet"));
        view.interfaces[0].up = false;
        assert!(!interface_has_ipv6_default(&view, "Ethernet"));
    }
    #[test]
    fn tcp_diagnostic_ignores_other_vpn_tun_even_with_a_better_metric() {
        let interface = |luid, kind, alias: &str, description: &str| Interface {
            luid,
            index: luid as u32,
            guid: format!("fixture-{luid}"),
            alias: alias.into(),
            description: description.into(),
            kind,
            up: true,
        };
        let view = NetworkView {
            interfaces: vec![
                interface(9991, 53, "foreign-tun", "sing-tun Tunnel"),
                interface(9992, 6, "Ethernet", "Ethernet fixture"),
                interface(9993, 53, "NetPulse", "Wintun"),
            ],
            routes: vec![
                Route {
                    luid: 9991,
                    prefix: 0,
                    destination: "0.0.0.0".into(),
                    metric: 0,
                    ..Route::default()
                },
                Route {
                    luid: 9992,
                    prefix: 0,
                    destination: "0.0.0.0".into(),
                    metric: 100,
                    ..Route::default()
                },
            ],
        };
        assert_eq!(
            diagnostic_interface(&view, "203.0.113.1".parse().unwrap()),
            Some(9992)
        );
        let only_tun = NetworkView {
            interfaces: view
                .interfaces
                .into_iter()
                .filter(|i| i.kind == 53)
                .collect(),
            routes: view.routes,
        };
        assert_eq!(
            diagnostic_interface(&only_tun, "203.0.113.1".parse().unwrap()),
            None
        );
    }
    #[test]
    fn windows_route_snapshot_read_only() {
        let v = inspect().unwrap();
        assert!(!v.interfaces.is_empty());
        assert!(v.routes.iter().all(|r| r.prefix <= 128));
    }
    #[test]
    fn readiness_requires_live_interface_and_both_ip_families() {
        let interface = Interface {
            description: "Wintun Userspace Tunnel".into(),
            kind: 53,
            luid: 10,
            index: 1,
            guid: "owned".into(),
            alias: "NetPulse".into(),
            up: true,
        };
        let route = |destination: &str, prefix| Route {
            luid: 10,
            index: 1,
            destination: destination.into(),
            prefix,
            gateway: "0.0.0.0".into(),
            metric: 0,
            ..Route::default()
        };
        let mut v = NetworkView {
            interfaces: vec![interface],
            routes: vec![route("172.31.255.0", 30)],
        };
        assert!(!default_routes_present(&v));
        v.routes = vec![route("0.0.0.0", 1), route("128.0.0.0", 1), route("::", 1)];
        assert!(!default_routes_present(&v));
        v.routes.push(route("8000::", 1));
        assert!(default_routes_present(&v));
        v.interfaces[0].up = false;
        assert!(!default_routes_present(&v));
    }
    #[test]
    fn dormant_wintun_can_be_reused_but_live_or_foreign_adapters_cannot() {
        let mut view = NetworkView {
            interfaces: vec![Interface {
                luid: 42,
                index: 7,
                guid: "fixture".into(),
                alias: "NetPulse".into(),
                up: false,
                description: "Wintun Userspace Tunnel".into(),
                kind: 53,
            }],
            routes: vec![],
        };
        assert!(validate_tun_reuse(&view).is_ok());
        view.interfaces[0].up = true;
        assert_eq!(
            validate_tun_reuse(&view).unwrap_err().code,
            "TunInterfaceConflict"
        );
        view.interfaces[0].up = false;
        view.interfaces[0].description = "Ethernet".into();
        assert!(validate_tun_reuse(&view).is_err());
        view.interfaces[0].alias = "Other VPN".into();
        assert!(validate_tun_reuse(&view).is_ok());
    }
}
