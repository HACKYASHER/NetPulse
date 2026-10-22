//! Read-only adapter inspection and real IPv4 DF probes. A timeout is not
//! evidence of fragmentation. Userspace proxy health is not a PMTU measurement.
use crate::{Error, Result};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
#[derive(Clone, Serialize)]
pub struct Adapter {
    pub name: String,
    pub kind: String,
    pub up: bool,
    pub mtu: u32,
    pub addresses: Vec<String>,
    pub selected: bool,
}
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Echo {
    Reply,
    TooBig,
    Unknown,
}
#[derive(Serialize, Default, Debug)]
pub struct PathResult {
    pub confirmed_mtu: Option<u16>,
    pub largest_confirmed: Option<u16>,
    pub probes: usize,
    pub note: String,
}
#[derive(Serialize)]
pub struct Report {
    pub adapters: Vec<Adapter>,
    pub target: String,
    pub path: PathResult,
    pub suggested_mtu: Option<u16>,
    pub estimate: bool,
    pub recommendation_source: String,
    pub base_mtu: Option<u16>,
    pub reserve: u16,
    pub tunnel_active: bool,
    pub tunnel_https_ok: Option<bool>,
    pub tunnel_note: String,
}
/// A TUN candidate is always an estimate, even when its input PMTU was measured.
/// An active physical adapter is a useful fallback when ICMP is filtered or TUN
/// intercepts echoes. Never use the virtual adapter's own MTU as its input.
pub fn recommendation(
    path: &PathResult,
    adapters: &[Adapter],
) -> (Option<u16>, &'static str, Option<u16>) {
    let (base, source) = if let Some(n) = path.confirmed_mtu {
        (Some(n), "measured_path")
    } else {
        (
            adapters
                .iter()
                .find(|a| {
                    a.selected
                        && a.up
                        && a.kind != "TUN"
                        && a.mtu >= 1280
                        && a.addresses.iter().any(|ip| {
                            ip.parse::<std::net::Ipv4Addr>().is_ok_and(|ip| {
                                !ip.is_unspecified() && !ip.is_link_local() && !ip.is_loopback()
                            })
                        })
                })
                .map(|a| a.mtu.min(9000) as u16),
            "physical_adapter",
        )
    };
    (
        base.map(|n| n.saturating_sub(80).clamp(1280, 1420)),
        if base.is_some() {
            source
        } else {
            "unavailable"
        },
        base,
    )
}
pub fn search(
    upper: u16,
    cancel: &AtomicBool,
    mut probe: impl FnMut(u16) -> Echo,
) -> Result<PathResult> {
    let mut count = 0;
    let mut check = |size| -> Result<Echo> {
        if cancel.load(Ordering::SeqCst) {
            return Err(Error::new("Cancelled", "mtu", "Проверка отменена"));
        }
        count += 1;
        let first = probe(size);
        if first == Echo::Unknown {
            count += 1;
            Ok(probe(size))
        } else {
            Ok(first)
        }
    };
    if check(60)? != Echo::Reply {
        return Ok(PathResult { probes: count, note: "Даже малый ICMP пакет не получил ответа. MTU не определён: возможна фильтрация ICMP.".into(), ..Default::default() });
    }
    let mut low = 60;
    let mut high = upper.max(60);
    match check(high)? {
        Echo::Reply => {
            if check(high)? != Echo::Reply || check(high)? != Echo::Reply {
                return Ok(PathResult { largest_confirmed: Some(high), probes: count, note: "Верхняя граница не прошла повторную проверку; возможны потери или изменение маршрута.".into(), ..Default::default() });
            }
            return Ok(PathResult { confirmed_mtu: Some(high), largest_confirmed: Some(high), probes: count, note: "Пакет с DF прошёл до верхней границы выбранного интерфейса. Результат относится только к этому IPv4 маршруту; верхняя граница проверена трижды.".into() }); },
        Echo::TooBig => high -= 1,
        Echo::Unknown => return Ok(PathResult { largest_confirmed: Some(low), probes: count, note: "Большой пакет не получил ответа, но сообщение о фрагментации не получено. Точный MTU не определён.".into(), ..Default::default() }),
    }
    while low < high {
        let mid = low + (high - low).div_ceil(2);
        match check(mid)? {
            Echo::Reply => low = mid,
            Echo::TooBig => high = mid - 1,
            Echo::Unknown => return Ok(PathResult { largest_confirmed: Some(low), probes: count, note: "Проверка неоднозначна: таймаут не считается подтверждением слишком большого пакета.".into(), ..Default::default() }),
        }
    }
    // Recheck both sides of the boundary. A changing path or contradictory
    // ICMP replies must not be presented as a precise recommendation.
    for _ in 0..2 {
        if check(low)? != Echo::Reply || check(low.saturating_add(1))? != Echo::TooBig {
            return Ok(PathResult { largest_confirmed: Some(low), probes: count,
                note: "Граница нестабильна при повторной проверке; точное значение не подтверждено. Повторите анализ.".into(), ..Default::default() });
        }
    }
    Ok(PathResult {
        confirmed_mtu: Some(low),
        largest_confirmed: Some(low),
        probes: count,
        note: "Граница повторно подтверждена с обеих сторон: ответы ICMP и явные сообщения о необходимости фрагментации."
            .into(),
    })
}
#[cfg(windows)]
pub fn adapters() -> Result<Vec<Adapter>> {
    use windows_sys::Win32::{NetworkManagement::IpHelper::*, Networking::WinSock::*};
    let view = crate::routes::inspect()?;
    let selected = crate::routes::outbound_interface();
    let mut table = std::ptr::null_mut();
    let status = unsafe { GetUnicastIpAddressTable(AF_INET, &mut table) };
    let addresses = if status == 0 {
        let rows = unsafe {
            std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize)
        };
        let values: Vec<_> = rows
            .iter()
            .map(|r| {
                (r.InterfaceIndex, unsafe {
                    std::net::Ipv4Addr::from(r.Address.Ipv4.sin_addr.S_un.S_addr.to_ne_bytes())
                        .to_string()
                })
            })
            .collect();
        unsafe { FreeMibTable(table as _) };
        values
    } else {
        vec![]
    };
    Ok(view
        .interfaces
        .iter()
        .filter(|i| matches!(i.kind, 6 | 71) || i.alias.eq_ignore_ascii_case("NetPulse"))
        .map(|i| {
            let mut row: MIB_IPINTERFACE_ROW = unsafe { std::mem::zeroed() };
            row.Family = AF_INET;
            row.InterfaceIndex = i.index;
            let mtu = if unsafe { GetIpInterfaceEntry(&mut row) } == 0 {
                row.NlMtu
            } else {
                0
            };
            Adapter {
                name: i.alias.clone(),
                kind: match i.kind {
                    6 => "Ethernet",
                    71 => "Wi-Fi",
                    _ => "TUN",
                }
                .into(),
                up: i.up,
                mtu,
                addresses: addresses
                    .iter()
                    .filter(|(index, _)| *index == i.index)
                    .map(|(_, ip)| ip.clone())
                    .collect(),
                selected: selected.as_deref() == Some(&i.alias),
            }
        })
        .collect())
}
#[cfg(windows)]
pub fn df_echo(target: std::net::Ipv4Addr, size: u16) -> Echo {
    use windows_sys::Win32::{Foundation::INVALID_HANDLE_VALUE, NetworkManagement::IpHelper::*};
    let handle = unsafe { IcmpCreateFile() };
    if handle == INVALID_HANDLE_VALUE || handle.is_null() {
        return Echo::Unknown;
    }
    let payload = vec![0x4e; size.saturating_sub(28) as usize];
    let options = IP_OPTION_INFORMATION {
        Ttl: 64,
        Flags: IP_FLAG_DF as u8,
        ..Default::default()
    };
    let mut buffer =
        vec![0u64; (payload.len() + std::mem::size_of::<ICMP_ECHO_REPLY>() + 32).div_ceil(8)];
    let count = unsafe {
        IcmpSendEcho(
            handle,
            u32::from_ne_bytes(target.octets()),
            payload.as_ptr() as _,
            payload.len() as u16,
            &options,
            buffer.as_mut_ptr() as _,
            (buffer.len() * 8) as u32,
            900,
        )
    };
    let status = if count == 0 {
        std::io::Error::last_os_error()
            .raw_os_error()
            .unwrap_or_default() as u32
    } else {
        unsafe { (*(buffer.as_ptr() as *const ICMP_ECHO_REPLY)).Status }
    };
    unsafe {
        IcmpCloseHandle(handle);
    }
    match status {
        0 if count > 0 => Echo::Reply,
        IP_PACKET_TOO_BIG => Echo::TooBig,
        _ => Echo::Unknown,
    }
}
#[cfg(windows)]
pub fn analyze(target: std::net::Ipv4Addr, tun: bool, cancel: &AtomicBool) -> Result<Report> {
    if cancel.load(Ordering::SeqCst) {
        return Err(Error::new("Cancelled", "mtu", "Проверка отменена"));
    }
    let adapters = adapters()?;
    let ceiling = adapters
        .iter()
        .find(|a| a.selected && a.up && a.mtu >= 1280)
        .map(|a| a.mtu.min(9000) as u16);
    let path = if tun {
        PathResult { note: "DF-измерение внешнего пути пропущено: включён TUN. Его локальная обработка ICMP может дать ложное число. Для измерения отключите TUN и повторите; HTTPS через VPN проверяется отдельно.".into(), ..Default::default() }
    } else if let Some(upper) = ceiling {
        search(upper, cancel, |size| df_echo(target, size))?
    } else {
        PathResult {
            note: "Не найден активный физический интерфейс с IPv4 MTU.".into(),
            ..Default::default()
        }
    };
    let (suggested, source, base) = recommendation(&path, &adapters);
    Ok(Report {
        adapters,
        target: target.to_string(),
        estimate: true,
        recommendation_source: source.into(),
        base_mtu: base,
        reserve: 80,
        path,
        suggested_mtu: suggested,
        tunnel_active: tun,
        tunnel_https_ok: None,
        tunnel_note: String::new(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recommendation_survives_tun_and_filtered_icmp_without_faking_measurement() {
        let mut a = Adapter {
            name: "Ethernet".into(),
            kind: "Ethernet".into(),
            up: true,
            mtu: 1500,
            addresses: vec!["192.168.1.2".into()],
            selected: true,
        };
        let path = PathResult::default();
        assert_eq!(
            recommendation(&path, &[a.clone()]),
            (Some(1420), "physical_adapter", Some(1500))
        );
        assert_eq!(path.confirmed_mtu, None);
        a.mtu = 1492;
        assert_eq!(recommendation(&path, &[a.clone()]).0, Some(1412));
        a.mtu = 1280;
        assert_eq!(recommendation(&path, &[a.clone()]).0, Some(1280));
        a.up = false;
        assert_eq!(recommendation(&path, &[a.clone()]).0, None);
        a.up = true;
        a.kind = "TUN".into();
        assert_eq!(recommendation(&path, &[a.clone()]).0, None);
        a.kind = "Ethernet".into();
        a.addresses = vec!["169.254.1.2".into()];
        assert_eq!(recommendation(&path, &[a]).0, None);
        let measured = PathResult {
            confirmed_mtu: Some(1400),
            ..Default::default()
        };
        assert_eq!(
            recommendation(&measured, &[]),
            (Some(1320), "measured_path", Some(1400))
        );
    }
    #[test]
    fn df_search_requires_explicit_fragmentation_and_does_not_treat_loss_as_mtu() {
        let flag = AtomicBool::new(false);
        let exact = search(1500, &flag, |n| {
            if n <= 1372 { Echo::Reply } else { Echo::TooBig }
        })
        .unwrap();
        assert_eq!(exact.confirmed_mtu, Some(1372));
        assert!(exact.probes < 30);
        assert!(
            search(1500, &flag, |_| Echo::Unknown)
                .unwrap()
                .confirmed_mtu
                .is_none()
        );
        assert!(
            search(1500, &flag, |n| if n == 60 {
                Echo::Reply
            } else {
                Echo::Unknown
            })
            .unwrap()
            .confirmed_mtu
            .is_none()
        );
        assert_eq!(
            search(1500, &AtomicBool::new(true), |_| Echo::Reply)
                .unwrap_err()
                .code,
            "Cancelled"
        );
    }
    #[test]
    #[cfg(windows)]
    fn real_df_probe_uses_owned_loopback() {
        assert_eq!(df_echo(std::net::Ipv4Addr::LOCALHOST, 1500), Echo::Reply);
    }
}
