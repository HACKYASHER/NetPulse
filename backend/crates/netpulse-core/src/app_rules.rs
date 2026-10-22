//! Windows application identities, independent of path casing and Discord versions.
use std::collections::BTreeSet;

fn quote_path(path: &str) -> String {
    let mut result = String::new();
    for ch in path.chars() {
        if ch == '\\' || ch == '/' {
            result.push_str(r"[\\/]");
        } else {
            if r".+*?()|[]{}^$".contains(ch) {
                result.push('\\');
            }
            result.push(ch);
        }
    }
    result
}

/// Windows EXE rules follow an executable name across installs and updates.
pub fn executable_name(path: &str) -> String {
    path.trim().trim_matches('"').rsplit(['\\', '/']).next().unwrap_or("").to_lowercase()
}
pub fn valid_executable(value: &str) -> bool {
    let name = executable_name(value);
    name.len() > 4 && name.len() <= 260 && name.ends_with(".exe")
        && !name.chars().any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
}
pub fn process_patterns(paths: &[String]) -> Vec<String> {
    paths.iter().map(|p|format!(r"(?i)(?:^|[\\/]){}$",quote_path(&executable_name(p))))
        .collect::<BTreeSet<_>>().into_iter().collect()
}

/// Internal runtime bypasses retain the full verified executable path.
pub fn exact_process_patterns(paths:&[String])->Vec<String> { paths.iter().map(|p|format!("(?i)^{}$",quote_path(p))).collect() }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn executable_rules_ignore_install_directory_and_escape_names() {
        assert_eq!(executable_name(r"C:\Apps\Steam.EXE"),"steam.exe");
        assert_eq!(process_patterns(&[r"C:\Apps (test)\a+.exe".into(),"A+.exe".into()]),[r"(?i)(?:^|[\\/])a\+\.exe$"]);
        assert!(valid_executable("steam.exe"));
        assert!(!valid_executable("*.exe"));
        assert!(!valid_executable("bad.exe:stream"));
        assert_eq!(exact_process_patterns(&[r"C:\Trusted\xray.exe".into()]),[r"(?i)^C:[\\/]Trusted[\\/]xray\.exe$"]);
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "Requires pinned engine and owned routing client; loopback only, no system proxy or TUN changes"]
    fn engine_routes_updated_discord_updater_helpers_and_case_insensitive_paths() {
        use crate::{
            engine::{Engine, configuration},
            model::{Preferences, RouteMode},
            runtime::free_port,
        };
        use std::{
            io::{Read, Write},
            net::TcpListener,
            sync::{
                Arc,
                atomic::{AtomicBool, Ordering},
            },
            time::{Duration, Instant},
        };
        let dir = tempfile::tempdir().unwrap();
        let clients = [
            (r"Discord\app-2.0.0\Discord.exe", true),
            (r"Discord\Update.exe", false),
            (
                r"Discord\app-2.0.0\modules\discord_utils-2\DiscordSystemHelper.exe",
                false,
            ),
            (r"Other\Update.exe", false),
            (r"Discord\app-2.0.0\unrelated.exe", false),
            (r"DiscordCopy\app-2.0.0\Discord.exe", true),
            (r"Apps (test)\Tool+.exe", true),
            (r"Apps (test)\Tool++.exe", false),
            (
                r"Roblox\Versions\version-abcdef12\RobloxPlayerBeta.exe",
                true,
            ),
            (
                r"Roblox\Versions\version-aabb1122\RobloxPlayerInstaller.exe",
                false,
            ),
            (r"Roblox\Versions\version-abcdef12\unrelated.exe", false),
            (
                r"RobloxCopy\Versions\version-abcdef12\RobloxPlayerBeta.exe",
                true,
            ),
        ];
        let template = std::env::var_os("NETPULSE_ROUTING_CLIENT").unwrap();
        for (relative, _) in &clients {
            let path = dir.path().join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::copy(&template, path).unwrap();
        }
        let origin = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin_port = origin.local_addr().unwrap().port();
        origin.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let server = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(30);
            let mut requests = 0;
            while !stopping.load(Ordering::SeqCst) && Instant::now() < deadline {
                if let Ok((mut socket, _)) = origin.accept() {
                    // Winsock accepts inherit FIONBIO from the listening socket.
                    // Switch the accepted stream back before reading headers.
                    socket.set_nonblocking(false).unwrap();
                    socket
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    // TCP may split headers across reads. Closing before the
                    // complete request arrives resets the connection on Windows
                    // and falsely makes a correctly routed request look like 502.
                    let mut request = Vec::new();
                    let mut block = [0; 2048];
                    while !request.ends_with(b"\r\n\r\n") && request.len() < 8192 {
                        match socket.read(&mut block) {
                            Ok(0) | Err(_) => break,
                            Ok(n) => request.extend_from_slice(&block[..n]),
                        }
                    }
                    let _=socket.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    requests += 1;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            requests
        });
        let engine_path =
            std::path::PathBuf::from(std::env::var_os("NETPULSE_TEST_ENGINE").unwrap());
        let profile =
            crate::imports::parse_uri(&format!("socks5://127.0.0.1:{}", free_port().unwrap()))
                .unwrap();
        for mode in [RouteMode::Whitelist, RouteMode::Blacklist] {
            let port = free_port().unwrap();
            let prefs = Preferences {
                applications: vec![
                    dir.path()
                        .join(r"Roblox\Versions\version-00112233\RobloxPlayerBeta.exe")
                        .to_string_lossy()
                        .to_lowercase(),
                    dir.path()
                        .join(r"Discord\app-1.0.0\Discord.exe")
                        .to_string_lossy()
                        .to_lowercase(),
                    dir.path()
                        .join(r"Apps (test)\Tool+.exe")
                        .to_string_lossy()
                        .to_lowercase(),
                ],
                applications_migrated: true,
                route_mode: mode.clone(),
                proxy_port: port,
                system_proxy: true,
                ..Preferences::default()
            };
            let mut config =
                configuration(&profile, &prefs, free_port().unwrap(), "api", "fixture").unwrap();
            config["log"]["level"] = serde_json::json!("info");
            Engine::validate(&engine_path, &config).unwrap();
            let mut engine = Engine::spawn(&engine_path, &config, false).unwrap();
            for _ in 0..50 {
                if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            for (relative, selected) in &clients {
                let output = std::process::Command::new(dir.path().join(relative))
                    .args([port.to_string(), origin_port.to_string()])
                    .output()
                    .unwrap();
                assert!(output.status.success());
                let status = String::from_utf8_lossy(&output.stdout);
                let direct = if mode == RouteMode::Whitelist {
                    !selected
                } else {
                    *selected
                };
                assert_eq!(
                    status
                        .lines()
                        .any(|line| line.starts_with("HTTP/") && line.contains(" 204 ")),
                    direct,
                    "mode={mode:?}, client={relative}, response={status}, engine={}",
                    engine.diagnostic(&config)
                );
            }
            engine.stop().unwrap();
        }
        stop.store(true, Ordering::SeqCst);
        assert_eq!(server.join().unwrap(), clients.len());
    }
}
