//! Read-only evidence: never reset Windows networking or infer success from missing data.
use crate::{
    model::Preferences,
    network::{ConnectionState, Snapshot},
};
use serde::Serialize;
use std::{
    net::UdpSocket,
    path::Path,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Serialize)]
pub struct Check {
    pub name: String,
    pub status: &'static str,
    pub detail: String,
    pub repair: Option<&'static str>,
}
#[derive(Serialize)]
pub struct Report {
    pub checks: Vec<Check>,
    pub captured_at: u64,
    pub state: ConnectionState,
}
fn row(name: &str, status: &'static str, detail: impl Into<String>) -> Check {
    Check {
        name: name.into(),
        status,
        detail: detail.into(),
        repair: None,
    }
}
fn check_files(name: &str, directory: &Path, files: &[&str], repair: &'static str) -> Check {
    let unavailable: Vec<_> = files
        .iter()
        .copied()
        .filter(|file| {
            let path = directory.join(file);
            let Ok(metadata) = path.symlink_metadata() else {
                return true;
            };
            if !metadata.is_file() || metadata.len() == 0 {
                return true;
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if metadata.file_attributes() & 0x400 != 0 {
                    return true;
                }
            }
            std::fs::File::open(path).is_err()
        })
        .collect();
    if unavailable.is_empty() {
        row(
            name,
            "observed",
            "Required files are present and readable. / Необходимые файлы найдены и доступны для чтения.",
        )
    } else {
        let mut result = row(
            name,
            "warning",
            format!(
                "Missing or unreadable / Отсутствуют или недоступны: {}",
                unavailable.join(", ")
            ),
        );
        result.repair = Some(repair);
        result
    }
}
fn unix() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}
fn stamp(bytes: &[u8]) -> f64 {
    u32::from_be_bytes(bytes[..4].try_into().unwrap()) as f64 - 2208988800.0
        + u32::from_be_bytes(bytes[4..8].try_into().unwrap()) as f64 / 4294967296.0
}
fn ntp_reply(packet: &[u8], nonce: &[u8], start: f64, end: f64, elapsed: f64) -> Option<f64> {
    if packet.len() < 48
        || packet[0] >> 6 == 3
        || packet[0] & 7 != 4
        || !matches!((packet[0] >> 3) & 7, 3 | 4)
        || !(1..=15).contains(&packet[1])
        || &packet[24..32] != nonce
        || elapsed > 1.0
        || ((end - start) - elapsed).abs() > 0.1
    {
        return None;
    }
    let receive = stamp(&packet[32..40]);
    let transmit = stamp(&packet[40..48]);
    if receive < 1577836800.0 || transmit < receive || transmit - receive > elapsed + 0.01 {
        return None;
    }
    Some(((receive - start) + (transmit - end)) / 2.0)
}
fn clock_sample(address: &str) -> Option<f64> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    socket
        .set_write_timeout(Some(Duration::from_secs(2)))
        .ok()?;
    socket.connect(address).ok()?;
    let mut request = [0u8; 48];
    request[0] = 0x23;
    let nonce = uuid::Uuid::new_v4();
    request[40..48].copy_from_slice(&nonce.as_bytes()[..8]);
    let start = unix();
    let timer = Instant::now();
    socket.send(&request).ok()?;
    let mut response = [0u8; 512];
    let size = socket.recv(&mut response).ok()?;
    ntp_reply(
        &response[..size],
        &request[40..48],
        start,
        unix(),
        timer.elapsed().as_secs_f64(),
    )
}
fn clock_result(a: Option<f64>, b: Option<f64>) -> Check {
    match (a, b) {
        (Some(a), Some(b)) if (a - b).abs() < 1.0 => row(
            "Clock / Время",
            if a.abs().max(b.abs()) > 2.0 {
                "warning"
            } else {
                "observed"
            },
            format!(
                "NTP estimate / оценка: {:+.2} s. UDP is unauthenticated; verify in Windows Date & Time. / Проверьте и синхронизируйте время в Windows; это не доказательство причины TLS-ошибки.",
                (a + b) / 2.0
            ),
        ),
        _ => row(
            "Clock / Время",
            "unknown",
            "NTP unavailable, slow or inconsistent. Accuracy is NOT confirmed. / Точность времени не подтверждена: UDP может быть заблокирован. Откройте Дата и время → Синхронизировать сейчас.",
        ),
    }
}
pub fn settings(saved: &Preferences, snapshot: &Snapshot) -> Vec<Check> {
    let wanted = serde_json::to_value(saved).unwrap_or_default();
    let applied = snapshot
        .applied_preferences
        .as_ref()
        .and_then(|p| serde_json::to_value(p).ok());
    let mut rows = vec![];
    for name in [
        "tun",
        "system_proxy",
        "proxy_port",
        "dns",
        "dns_route",
        "mtu",
        "ipv6_mode",
        "fake_ip",
        "route_mode",
        "applications",
        "application_groups",
        "bypass_domains",
        "routing_rules",
        "sniffing",
        "sniff_excluded_apps",
        "fragmentation",
        "multiplex",
        "transport_engine",
        "tls_defaults",
        "probe_url",
    ] {
        let status = match &applied {
            None => "unknown",
            Some(p) if p[name] == wanted[name] => "configured",
            _ => "warning",
        };
        rows.push(row(name,status,match status {
            "configured"=>"Saved and applied values match; this does not prove traffic behaviour. / Сохранённое совпадает с применённым; прохождение трафика требует отдельной проверки.",
            "warning"=>"Saved and applied values differ. Reconnect to apply; active sessions will close. / Настройки расходятся. Переподключитесь для применения; текущие соединения прервутся.",
            _=>"No active configuration to compare. / Нет активной конфигурации для сравнения.",
        }));
    }
    rows
}
pub fn probe_vpn(route: &crate::diagnostics::DiagnosticRoute) -> Option<bool> {
    let port = route.vpn_port?;
    let success = (|| {
        let proxy = reqwest::Proxy::all(format!("http://127.0.0.1:{port}"))
            .ok()?
            .basic_auth("netpulse", &route.password);
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .proxy(proxy)
            .timeout(Duration::from_secs(6))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .ok()?;
        let response = client
            .get("https://www.gstatic.com/generate_204")
            .send()
            .ok()?;
        Some(response.status() == reqwest::StatusCode::NO_CONTENT)
    })();
    Some(success.unwrap_or(false))
}

pub fn run(
    saved: Preferences,
    snapshot: Snapshot,
    root: &Path,
    engine_path: &Path,
    external: bool,
    vpn: Option<bool>,
) -> Report {
    let captured_at = unix() as u64;
    let comparison = settings(&saved, &snapshot);
    let mut checks = Vec::new();
    if let Some(error) = &snapshot.error {
        checks.insert(
            0,
            row(
                "Last connection error / Последняя ошибка",
                "warning",
                format!("{} ({})", error.code, error.stage),
            ),
        );
    }
    checks.insert(
        0,
        row(
            "Configuration / Конфигурация",
            if saved.validate().is_ok() {
                "observed"
            } else {
                "warning"
            },
            "Local validation only / Локальная проверка допустимости значений",
        ),
    );
    checks.push(check_files(
        "Client files / Файлы клиента",
        root,
        &[
            "NetPulse.exe",
            "netpulsed.exe",
            "NetPulseService.exe",
            "Qt6Core.dll",
            "Qt6Gui.dll",
            "Qt6Qml.dll",
            "platforms/qwindows.dll",
        ],
        "installer",
    ));
    let engine_files = ["sing-box.exe", "libcronet.dll", "xray.exe"];
    checks.push(check_files(
        "Bundled engines / Встроенные ядра",
        &root.join("cores"),
        &engine_files,
        "installer",
    ));
    if let Some(directory) = engine_path.parent() {
        checks.push(check_files(
            "Runtime engines / Рабочие ядра",
            directory,
            &engine_files,
            "engines",
        ));
    }
    #[cfg(windows)]
    {
        match crate::proxy::read() {
            Ok(p) => {
                let expected = snapshot
                    .applied_preferences
                    .as_ref()
                    .filter(|p| p.system_proxy);
                let matches = expected.is_some_and(|e| {
                    p.flags & 2 != 0
                        && p.flags & 4 == 0
                        && p.server == format!("127.0.0.1:{}", e.proxy_port)
                });
                checks.push(row("Windows proxy / Прокси Windows",if matches || (expected.is_none() && p.flags&6==0){"observed"}else{"warning"},
                    if matches {"Windows points to NetPulse / Windows использует адрес NetPulse"} else if p.flags&6!=0 {"Proxy/PAC is active or differs from NetPulse. Check other VPN/proxy apps; foreign settings are not overwritten. / Прокси или PAC включён либо отличается. Проверьте другие VPN; чужие настройки не заменяются."} else {"System proxy is disabled / Системный прокси выключен"}));
            }
            Err(_) => checks.push(row(
                "Windows proxy / Прокси Windows",
                "unknown",
                "Cannot read Windows proxy / Не удалось прочитать настройки Windows",
            )),
        }
        match crate::routes::inspect() {
            Ok(view) => {
                let active = view
                    .interfaces
                    .iter()
                    .filter(|i| i.up && i.alias == "NetPulse")
                    .collect::<Vec<_>>();
                let routed = active
                    .iter()
                    .any(|i| view.routes.iter().any(|r| r.luid == i.luid && !r.loopback));
                let wanted = snapshot.applied_preferences.as_ref().is_some_and(|p| p.tun);
                if wanted {
                    use windows_sys::Win32::{
                        NetworkManagement::IpHelper::*, Networking::WinSock::AF_INET,
                    };
                    let mtu = active.first().and_then(|i| {
                        let mut row = MIB_IPINTERFACE_ROW::default();
                        row.Family = AF_INET;
                        row.InterfaceIndex = i.index;
                        (unsafe { GetIpInterfaceEntry(&mut row) } == 0).then_some(row.NlMtu)
                    });
                    let expected = snapshot.applied_preferences.as_ref().unwrap().mtu as u32;
                    checks.push(row("MTU Windows",match mtu {Some(m) if m==expected=>"observed",Some(_)=>"warning",None=>"unknown"},format!("IPv4 MTU: {mtu:?}; configured: {expected}. / MTU адаптера и применённое значение. Это не измерение Path MTU.")));
                }
                checks.push(row("TUN adapter/routes / Адаптер и маршруты TUN",if wanted && (active.len()!=1 || !routed){"warning"}else{"observed"},format!("NetPulse adapters up: {}; routes: {}. / Активных адаптеров: {}; маршруты: {}. При расхождении проверьте службу TUN в настройках.",active.len(),routed,active.len(),routed)));
                let defaults = view
                    .routes
                    .iter()
                    .filter(|r| {
                        r.prefix == 0
                            && !r.loopback
                            && view
                                .interfaces
                                .iter()
                                .any(|i| i.luid == r.luid && i.up && i.alias != "NetPulse")
                    })
                    .count();
                checks.push(row("Uplink / Внешняя сеть",if defaults==0{"warning"}else{"observed"},format!("Active default routes outside NetPulse: {defaults}. / Активных маршрутов вне NetPulse: {defaults}. Это не проверка доступности Интернета.")));
            }
            Err(_) => checks.push(row(
                "Windows routes / Маршруты Windows",
                "unknown",
                "Cannot inspect interfaces and routes / Не удалось прочитать адаптеры и маршруты",
            )),
        }
        if saved.tun || snapshot.tun_active {
            match crate::service::status() {
                Ok(s)=> {
                    let healthy=s.installed && s.error.is_none() && (!snapshot.tun_active ||
                        (s.running && s.owned_by_caller && s.tun_ready && s.version==env!("CARGO_PKG_VERSION")));
                    checks.push(row("TUN service / Служба TUN",if healthy{"observed"}else{"warning"},format!("Installed: {}, running: {}, TUN ready: {}. / Установлена: {}, запущена: {}, TUN готов: {}. При расхождении проверьте обслуживание службы.",s.installed,s.running,s.tun_ready,s.installed,s.running,s.tun_ready)));
                    if s.installed && let Ok(directory)=crate::service::protected_directory() {
                        checks.push(check_files("TUN files / Файлы TUN", &directory, &["NetPulseService.exe", "sing-box.exe", "libcronet.dll", "xray.exe"], "service"));
                    }
                },
                Err(_)=>checks.push(row("TUN service / Служба TUN","unknown","Service status unavailable; check permissions / Состояние службы недоступно; проверьте права")),
            }
        }
        use windows_service::{
            service::{ServiceAccess, ServiceState},
            service_manager::{ServiceManager, ServiceManagerAccess},
        };
        let time_service =
            ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
                .and_then(|m| m.open_service("W32Time", ServiceAccess::QUERY_STATUS))
                .and_then(|s| s.query_status());
        checks.push(match time_service {
            Ok(s)=>row("Windows Time / Служба времени",if s.current_state==ServiceState::Running{"observed"}else{"warning"},"Service state only; running does not prove synchronization. / Проверяется состояние службы; работающая служба не подтверждает точность часов. Если остановлена, проверьте автоматическое время Windows."),
            Err(_)=>row("Windows Time / Служба времени","unknown","Cannot query time service / Не удалось прочитать состояние службы времени"),
        });
    }
    checks.push(if external {
        clock_result(
            clock_sample("162.159.200.1:123"),
            clock_sample("162.159.200.123:123"),
        )
    } else {
        row(
            "Clock / Время",
            "unknown",
            "External NTP test not requested / Внешняя проверка времени не запускалась",
        )
    });
    if let Some(success) = vpn {
        checks.push(row("VPN HTTPS",if success{"observed"}else{"warning"},if success{
            "HTTPS through the active VPN returned 204. / HTTPS через действующий VPN прошёл. Это проверка одной цели, не всех приложений."
        }else{"HTTPS through VPN failed or returned an unexpected status. This alone does not identify DNS, clock, TLS, firewall or server failure. / HTTPS через VPN не подтверждён. Сопоставьте время, DNS, TLS и доступность сервера; причина не установлена."}));
    }
    checks.extend(comparison);
    Report {
        checks,
        captured_at,
        state: snapshot.state,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_clock_data_never_passes() {
        assert_eq!(clock_result(None, None).status, "unknown");
        assert_eq!(clock_result(Some(0.0), Some(4.0)).status, "unknown");
    }
    #[test]
    fn four_second_drift_is_reported() {
        assert_eq!(clock_result(Some(4.0), Some(4.1)).status, "warning");
    }
    #[test]
    fn missing_and_empty_files_are_reported() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("present.dll"), b"contents").unwrap();
        std::fs::write(directory.path().join("empty.dll"), b"").unwrap();
        let check = check_files(
            "fixture",
            directory.path(),
            &["present.dll", "empty.dll", "missing.dll"],
            "installer",
        );
        assert_eq!(check.status, "warning");
        assert_eq!(check.repair, Some("installer"));
        assert!(check.detail.contains("empty.dll") && check.detail.contains("missing.dll"));
        assert!(!check.detail.contains("present.dll"));
    }
    #[test]
    fn settings_mismatch_and_missing_runtime_are_distinct() {
        let saved = Preferences::default();
        let mut s = Snapshot {
            state: ConnectionState::Disconnected,
            active: None,
            engine_pid: None,
            metrics: None,
            metrics_error: None,
            error: None,
            probe_ms: None,
            system_proxy_active: false,
            tun_active: false,
            applied_preferences: None,
            recovery_pid: None,
        };
        assert!(settings(&saved, &s).iter().all(|r| r.status == "unknown"));
        let mut applied = saved.clone();
        applied.mtu = 1400;
        s.applied_preferences = Some(applied);
        let rows = settings(&saved, &s);
        assert_eq!(
            rows.iter().find(|r| r.name == "mtu").unwrap().status,
            "warning"
        );
        assert_eq!(
            rows.iter().find(|r| r.name == "dns").unwrap().status,
            "configured"
        );
    }
    #[test]
    fn rejects_wrong_nonce_unsynchronised_server_and_clock_jump() {
        let mut p = [0u8; 48];
        p[0] = 0x24;
        p[1] = 1;
        p[24..32].copy_from_slice(&[1; 8]);
        let t = 1700000000u32 + 2208988800;
        p[32..36].copy_from_slice(&t.to_be_bytes());
        p[40..44].copy_from_slice(&t.to_be_bytes());
        assert!(ntp_reply(&p, &[1; 8], 1700000000.0, 1700000000.1, 0.1).is_some());
        assert!(ntp_reply(&p, &[2; 8], 1700000000.0, 1700000000.1, 0.1).is_none());
        assert!(ntp_reply(&p, &[1; 8], 1700000000.0, 1700000004.0, 0.1).is_none());
        p[0] |= 0xc0;
        assert!(ntp_reply(&p, &[1; 8], 1700000000.0, 1700000000.1, 0.1).is_none());
    }
}
