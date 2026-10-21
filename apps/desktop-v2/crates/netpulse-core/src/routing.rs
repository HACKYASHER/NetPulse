//! Portable routing-only document; never includes subscription URLs or switches.
use crate::{
    Error, Result,
    model::{ApplicationGroup, Preferences, RouteMode},
};
use serde::{Deserialize, Serialize};

pub fn normalize_domain(raw: &str) -> String {
    let raw = raw.trim().trim_end_matches('.');
    let raw = raw
        .strip_prefix("*.")
        .or_else(|| raw.strip_prefix('.'))
        .unwrap_or(raw);
    if raw.is_empty() {
        return String::new();
    }
    if raw.contains(['/', ':', '@', '?', '#', '%']) {
        return raw.to_lowercase();
    }
    match url::Host::parse(raw) {
        Ok(url::Host::Domain(s)) => s.to_lowercase(),
        _ => raw.to_lowercase(),
    }
}
pub fn valid_domain(raw: &str) -> bool {
    let value = normalize_domain(raw);
    !value.is_empty()
        && value.len() <= 253
        && value.parse::<std::net::IpAddr>().is_err()
        && value.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
        && !raw.contains(['/', ':', '@', '?', '#', '%'])
}
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct RoutingSettings {
    pub version: u8,
    pub route_mode: RouteMode,
    pub applications: Vec<String>,
    pub application_groups: Vec<ApplicationGroup>,
    pub bypass_domains: Vec<String>,
    #[serde(default)]
    pub routing_rules: Vec<crate::advanced_routing::Rule>,
    #[serde(default)]
    pub sniff_excluded_apps: Vec<String>,
}
impl RoutingSettings {
    pub fn from_preferences(p: &Preferences) -> Self {
        Self {
            version: 1,
            route_mode: p.route_mode.clone(),
            applications: p.applications.clone(),
            application_groups: p.application_groups.clone(),
            bypass_domains: p.bypass_domains.clone(),
            routing_rules: p.routing_rules.clone(),
            sniff_excluded_apps: p.sniff_excluded_apps.clone(),
        }
    }
    pub fn apply(self, p: &Preferences) -> Result<Preferences> {
        if self.version != 1 || self.route_mode == RouteMode::Direct {
            return Err(Error::new(
                "RoutingVersion",
                "routing-import",
                "Неподдерживаемая версия или режим маршрутизации",
            ));
        }
        let next = Preferences {
            route_mode: self.route_mode,
            applications: self.applications,
            application_groups: self.application_groups,
            bypass_domains: self.bypass_domains,
            routing_rules: self.routing_rules,
            sniff_excluded_apps: self.sniff_excluded_apps,
            applications_migrated: true,
            whitelist: vec![],
            blacklist: vec![],
            ..p.clone()
        }
        .normalized();
        next.validate()?;
        Ok(next)
    }
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 2 * 1024 * 1024 {
            return Err(Error::new(
                "RoutingLimit",
                "routing-import",
                "Файл маршрутизации больше 2 МиБ",
            ));
        }
        let value: Self = serde_json::from_slice(bytes).map_err(|_| {
            Error::new(
                "RoutingInvalid",
                "routing-import",
                "Неверный формат файла маршрутизации",
            )
        })?;
        let checked = value.apply(&Preferences::default())?;
        Ok(Self::from_preferences(&checked))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn domain_normalization_and_invalid_values() {
        assert_eq!(normalize_domain(" *.Example.COM. "), "example.com");
        assert!(valid_domain("пример.рф"));
        for v in [
            "",
            "https://example.com",
            "example.com:443",
            "a/b",
            "a@b",
            "-bad.test",
            "127.0.0.1",
            "a..b",
            "a%2eb",
        ] {
            assert!(!valid_domain(v), "{v}");
        }
    }
    #[test]
    fn routing_roundtrip_preserves_unrelated_settings() {
        let base = Preferences {
            tun: true,
            system_proxy: true,
            mtu: 1400,
            accent: "#123456".into(),
            ..Preferences::default()
        };
        let source = Preferences {
            route_mode: RouteMode::Whitelist,
            applications: vec![r"C:\Apps\test.exe".into()],
            bypass_domains: vec!["*.Example.com".into()],
            ..Preferences::default()
        };
        let bytes = serde_json::to_vec(&RoutingSettings::from_preferences(&source)).unwrap();
        let restored = RoutingSettings::parse(&bytes)
            .unwrap()
            .apply(&base)
            .unwrap();
        assert!(restored.tun && restored.system_proxy);
        assert_eq!(restored.mtu, 1400);
        assert_eq!(restored.accent, base.accent);
        assert_eq!(restored.bypass_domains, ["example.com"]);
        assert_eq!(restored.applications, source.applications);
        assert_eq!(restored.route_mode, RouteMode::Whitelist);
        let mut bad = serde_json::to_value(RoutingSettings::from_preferences(&source)).unwrap();
        bad["version"] = 99.into();
        assert!(RoutingSettings::parse(&serde_json::to_vec(&bad).unwrap()).is_err());
        bad["version"] = 1.into();
        bad["subscription_url"] = "https://example.org/secret".into();
        assert!(RoutingSettings::parse(&serde_json::to_vec(&bad).unwrap()).is_err());
    }
    #[test]
    fn bypass_precedes_process_rules_in_every_mode() {
        let profile = crate::imports::parse_uri("socks5://127.0.0.1:1080").unwrap();
        for mode in [
            RouteMode::Global,
            RouteMode::Whitelist,
            RouteMode::Blacklist,
        ] {
            let p = Preferences {
                route_mode: mode,
                applications: vec![r"C:\Apps\fixture.exe".into()],
                bypass_domains: vec!["example.com".into()],
                ..Preferences::default()
            };
            let c = crate::engine::configuration(&profile, &p, 24567, "api", "proxy").unwrap();
            let rules = c["route"]["rules"].as_array().unwrap();
            let bypass = rules
                .iter()
                .position(|r| r["domain_suffix"].is_array())
                .unwrap();
            assert_eq!(rules[bypass]["outbound"], "direct");
            if let Some(process) = rules
                .iter()
                .position(|r| r["process_path_regex"].is_array())
            {
                assert!(bypass < process);
            }
            assert_eq!(c["dns"]["rules"][0]["server"], "bootstrap");
            assert_eq!(c["dns"]["reverse_mapping"], true);
        }
    }
    #[test]
    #[ignore = "Requires pinned engine; validates rules without touching system networking"]
    fn bundled_core_accepts_domain_bypass_configuration() {
        let path = std::path::PathBuf::from(std::env::var("NETPULSE_TEST_ENGINE").unwrap());
        let profile = crate::imports::parse_uri("socks5://127.0.0.1:1080").unwrap();
        let p = Preferences {
            tun: true,
            route_mode: RouteMode::Whitelist,
            applications: vec![r"C:\Apps\fixture.exe".into()],
            bypass_domains: vec!["example.com".into()],
            ..Preferences::default()
        };
        let c = crate::engine::configuration(&profile, &p, 24567, "api", "proxy").unwrap();
        crate::engine::Engine::validate(&path, &c).unwrap();
    }
    #[test]
    #[ignore = "Requires pinned engine; owned loopback HTTP origin, no system routes"]
    fn excluded_domain_reaches_origin_while_other_traffic_uses_unreachable_proxy() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            time::Duration,
        };
        let origin = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin_port = origin.local_addr().unwrap().port();
        origin.set_nonblocking(true).unwrap();
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let server_stop = stop.clone();
        let server = std::thread::spawn(move || {
            let mut requests = 0;
            let end = std::time::Instant::now() + Duration::from_secs(10);
            while !server_stop.load(std::sync::atomic::Ordering::SeqCst)
                && std::time::Instant::now() < end
            {
                if let Ok((mut socket, _)) = origin.accept() {
                    socket
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    let mut bytes = [0; 2048];
                    let _ = socket.read(&mut bytes);
                    socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nbypass").unwrap();
                    requests += 1;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            requests
        });
        let path = std::path::PathBuf::from(std::env::var("NETPULSE_TEST_ENGINE").unwrap());
        let unavailable = crate::runtime::free_port().unwrap();
        let profile =
            crate::imports::parse_uri(&format!("socks5://127.0.0.1:{unavailable}")).unwrap();
        let port = crate::runtime::free_port().unwrap();
        let p = Preferences {
            proxy_port: port,
            bypass_domains: vec!["localhost".into()],
            ..Preferences::default()
        };
        let mut c = crate::engine::configuration(
            &profile,
            &p,
            crate::runtime::free_port().unwrap(),
            "api",
            "fixture",
        )
        .unwrap();
        // This routing fixture must not depend on external DNS answering
        // "localhost"; some system resolvers forward it and return NXDOMAIN.
        c["dns"]["servers"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "type":"hosts","tag":"fixture-hosts","predefined":{"localhost":"127.0.0.1"}
            }));
        c["route"]["default_domain_resolver"] = serde_json::json!("fixture-hosts");
        let mut engine = crate::engine::Engine::spawn(&path, &c, false).unwrap();
        let client = reqwest::blocking::Client::builder()
            .proxy(
                reqwest::Proxy::all(format!("http://127.0.0.1:{port}"))
                    .unwrap()
                    .basic_auth("netpulse", "fixture"),
            )
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        let mut response = None;
        for _ in 0..20 {
            if let Ok(r) = client
                .get(format!("http://localhost:{origin_port}/"))
                .send()
                && r.status().is_success()
            {
                response = Some(r);
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(response.unwrap().text().unwrap(), "bypass");
        let other = client
            .get(format!("http://127.0.0.1:{origin_port}/"))
            .send();
        assert!(other.is_err() || !other.unwrap().status().is_success());
        engine.stop().unwrap();
        stop.store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(server.join().unwrap(), 1);
    }
}
