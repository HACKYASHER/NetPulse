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

fn discord_root(path: &str) -> Option<String> {
    let parts: Vec<_> = path.split(['\\', '/']).collect();
    let leaf = parts.last()?.to_ascii_lowercase();
    if ![
        "discord.exe",
        "discordptb.exe",
        "discordcanary.exe",
        "discordsystemhelper.exe",
        "update.exe",
    ]
    .contains(&leaf.as_str())
    {
        return None;
    }
    let root = parts.iter().rposition(|s| {
        ["discord", "discordptb", "discordcanary"]
            .iter()
            .any(|name| s.eq_ignore_ascii_case(name))
    })?;
    if root < 1 || parts.iter().any(|s| *s == "." || *s == "..") {
        return None;
    }
    let rest = &parts[root + 1..];
    let is_updater = rest.len() == 1 && leaf == "update.exe";
    let is_version = rest.len() >= 2
        && rest[0]
            .to_ascii_lowercase()
            .strip_prefix("app-")
            .is_some_and(|version| {
                !version.is_empty() && version.bytes().all(|ch| ch.is_ascii_digit() || ch == b'.')
            })
        && leaf != "update.exe";
    (is_updater || is_version).then(|| parts[..=root].join("\\"))
}

pub fn process_patterns(paths: &[String]) -> Vec<String> {
    let mut patterns = BTreeSet::new();
    for path in paths {
        if let Some(root) = discord_root(path) {
            // Scope updater/helper matching to this selected installation, never
            // every Update.exe or every executable in the user's AppData.
            patterns.insert(format!(r"(?i)^{}[\\/](Update\.exe|app-[0-9][0-9.]*[\\/](?:[^\\/:]+[\\/])*(?:Discord(?:PTB|Canary)?\.exe|DiscordSystemHelper\.exe))$", quote_path(&root)));
        } else if let Some(root) = roblox_versions(path) {
            patterns.insert(format!(r"(?i)^{}[\\/]version-[a-f0-9]+[\\/](?:RobloxPlayerBeta|RobloxPlayerInstaller|RobloxPlayerLauncher|RobloxCrashHandler)\.exe$", quote_path(&root)));
        } else {
            patterns.insert(format!("(?i)^{}$", quote_path(path)));
        }
    }
    patterns.into_iter().collect()
}

fn roblox_versions(path: &str) -> Option<String> {
    let parts: Vec<_> = path.split(['\\', '/']).collect();
    if parts.len() < 4 || parts.iter().any(|s| *s == "." || *s == "..") {
        return None;
    }
    let n = parts.len();
    let exe = parts[n - 1].to_ascii_lowercase();
    if ![
        "robloxplayerbeta.exe",
        "robloxplayerinstaller.exe",
        "robloxplayerlauncher.exe",
        "robloxcrashhandler.exe",
    ]
    .contains(&exe.as_str())
        || !parts[n - 3].eq_ignore_ascii_case("versions")
    {
        return None;
    }
    let version = parts[n - 2].to_ascii_lowercase();
    let hash = version.strip_prefix("version-")?;
    if hash.is_empty() || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(parts[..n - 2].join("\\"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_recognized_installations_expand() {
        assert_eq!(
            discord_root(r"C:\Users\fixture\Discord\app-1.0.9220\Discord.exe").as_deref(),
            Some(r"C:\Users\fixture\Discord")
        );
        assert!(discord_root(r"C:\Other\Update.exe").is_none());
        assert!(discord_root(r"C:\Discord\arbitrary\Discord.exe").is_none());
        assert!(discord_root(r"C:\Discord\app-1.0\Other.exe").is_none());
        assert!(discord_root(r"C:\Discord\..\Update.exe").is_none());
        assert_eq!(
            process_patterns(&[r"C:\Apps (test)\a+.exe".into()]),
            [r"(?i)^C:[\\/]Apps \(test\)[\\/]a\+\.exe$"]
        );
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
            (r"Discord\Update.exe", true),
            (
                r"Discord\app-2.0.0\modules\discord_utils-2\DiscordSystemHelper.exe",
                true,
            ),
            (r"Other\Update.exe", false),
            (r"Discord\app-2.0.0\unrelated.exe", false),
            (r"DiscordCopy\app-2.0.0\Discord.exe", false),
            (r"Apps (test)\Tool+.exe", true),
            (r"Apps (test)\Tool++.exe", false),
            (
                r"Roblox\Versions\version-abcdef12\RobloxPlayerBeta.exe",
                true,
            ),
            (
                r"Roblox\Versions\version-aabb1122\RobloxPlayerInstaller.exe",
                true,
            ),
            (r"Roblox\Versions\version-abcdef12\unrelated.exe", false),
            (
                r"RobloxCopy\Versions\version-abcdef12\RobloxPlayerBeta.exe",
                false,
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
