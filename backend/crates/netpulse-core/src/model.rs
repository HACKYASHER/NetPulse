use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use zeroize::Zeroize;

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub outbound: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outbound_override: Option<Value>,
    #[serde(default)]
    pub subscription_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport_override: Option<crate::transport_options::ProfileTransport>,
}
impl Drop for Profile {
    fn drop(&mut self) {
        fn wipe(v: &mut Value) {
            match v {
                Value::String(s) => s.zeroize(),
                Value::Object(m) => m.values_mut().for_each(wipe),
                Value::Array(a) => a.iter_mut().for_each(wipe),
                _ => (),
            }
        }
        wipe(&mut self.outbound);
        if let Some(v) = &mut self.outbound_override {
            wipe(v);
        }
    }
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProfileSummary {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub host: String,
    pub port: u16,
    pub subscription_id: Option<String>,
    pub latency_ms: Option<u64>,
    pub latency_method: Option<String>,
    pub latency_at: Option<u64>,
    pub latency_status: Option<String>,
}
impl Profile {
    pub fn requires_xray(&self) -> bool {
        self.outbound["transport"]["type"] == "xhttp"
            || self.outbound["transport"]["mode"] == "multi"
    }
    pub fn summary(&self) -> ProfileSummary {
        let o = self.outbound_override.as_ref().unwrap_or(&self.outbound);
        ProfileSummary {
            id: self.id.clone(),
            name: self.name.clone(),
            protocol: o["type"].as_str().unwrap_or("").into(),
            host: o["server"].as_str().unwrap_or("").into(),
            port: o["server_port"].as_u64().unwrap_or(0) as u16,
            subscription_id: self.subscription_id.clone(),
            latency_ms: None,
            latency_method: None,
            latency_at: None,
            latency_status: None,
        }
    }
    pub fn validate(&self) -> Result<()> {
        if self.transport_override.is_some() || self.outbound_override.is_some() {
            self.resolved()?;
        }
        crate::imports::validate_outbound_schema(&self.outbound)?;
        let fail = |s| Error::new("InvalidProfile", "profile-validate", s);
        if self.name.is_empty() || self.name.len() > 256 || self.name.chars().any(char::is_control)
        {
            return Err(fail("Profile name must contain 1–256 printable bytes"));
        }
        let o = &self.outbound;
        if serde_json::to_vec(o).map_or(true, |v| v.len() > 65536) {
            return Err(fail("Profile exceeds 64 KiB"));
        }
        let host = o["server"].as_str().unwrap_or("");
        if host.is_empty()
            || host.len() > 253
            || host
                .chars()
                .any(|c| c.is_whitespace() || c.is_control() || "/\\@?#".contains(c))
        {
            return Err(fail("Invalid server host"));
        }
        let port = o["server_port"].as_u64().unwrap_or(0);
        if port == 0 || port > 65535 {
            return Err(fail("Server port must be 1–65535"));
        }
        let kind = o["type"].as_str().unwrap_or("");
        if ![
            "vless",
            "trojan",
            "hysteria2",
            "shadowsocks",
            "socks",
            "http",
        ]
        .contains(&kind)
        {
            return Err(fail("Unsupported protocol"));
        }
        if kind == "vless" && uuid::Uuid::parse_str(o["uuid"].as_str().unwrap_or("")).is_err() {
            return Err(fail("VLESS UUID is invalid"));
        }
        if ["trojan", "hysteria2", "shadowsocks"].contains(&kind)
            && o["password"].as_str().unwrap_or("").is_empty()
        {
            return Err(fail("Protocol requires a password"));
        }
        if o["tls"]["insecure"].as_bool() == Some(true) {
            return Err(fail("TLS certificate verification must remain enabled"));
        }
        if ["vless", "trojan", "hysteria2"].contains(&kind) && o["tls"]["enabled"] != true {
            return Err(fail("This protocol requires TLS"));
        }
        let flow = o["flow"].as_str().unwrap_or("");
        if !flow.is_empty() && flow != "xtls-rprx-vision" {
            return Err(fail("Only XTLS Vision flow is supported"));
        }
        if !flow.is_empty() && (!o["transport"].is_null() || kind != "vless") {
            return Err(fail("Vision requires VLESS TCP"));
        }
        if kind == "shadowsocks"
            && ![
                "aes-128-gcm",
                "aes-256-gcm",
                "chacha20-ietf-poly1305",
                "2022-blake3-aes-128-gcm",
                "2022-blake3-aes-256-gcm",
                "2022-blake3-chacha20-poly1305",
            ]
            .contains(&o["method"].as_str().unwrap_or(""))
        {
            return Err(fail("Unsupported Shadowsocks cipher"));
        }
        if let Some(reality) = o["tls"].get("reality") {
            use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
            if URL_SAFE_NO_PAD
                .decode(reality["public_key"].as_str().unwrap_or(""))
                .map(|v| v.len())
                .ok()
                != Some(32)
            {
                return Err(fail("Reality public key must encode 32 bytes"));
            }
            let sid = reality["short_id"].as_str().unwrap_or("");
            if sid.len() > 16 || sid.len() % 2 != 0 || !sid.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(fail(
                    "Reality short ID must be up to 16 hexadecimal characters",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RouteMode {
    #[default]
    Global,
    Whitelist,
    Blacklist,
    Direct,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    pub language: String,
    pub theme: String,
    pub accent: String,
    pub surface: String,
    pub radius: u8,
    pub density: String,
    pub motion: bool,
    pub route_mode: RouteMode,
    pub applications: Vec<String>,
    pub bypass_domains: Vec<String>,
    pub proxy_port: u16,
    pub system_proxy: bool,
    pub tun: bool,
    pub dns: Dns,
    pub dns_route: String,
    pub mtu: u16,
    pub network_preset: String,
    pub applications_migrated: bool,
    pub probe_url: String,
    pub transport_engine: String,
    pub tls_defaults: Option<crate::transport_options::ProfileTransport>,
    pub fake_ip: bool,
    pub routing_rules: Vec<crate::advanced_routing::Rule>,
    pub sniff_excluded_apps: Vec<String>,
    pub sniffing: crate::advanced_routing::Sniffing,
    pub fragmentation: crate::transport_options::Fragmentation,
    pub multiplex: crate::transport_options::Multiplex,
    pub ping_method: String,
    pub ping_proxy_mode: String,
    pub selected_profile: Option<String>,
    pub favorites: Vec<String>,
    pub whitelist: Vec<String>,
    pub blacklist: Vec<String>,
    pub app_view: String,
    pub server_view: String,
    pub autostart: bool,
    pub close_to_tray: bool,
    pub minimize_to_tray: bool,
    pub application_groups: Vec<ApplicationGroup>,
    pub auto_reconnect: bool,
    pub auto_failover: bool,
    pub selection_samples: u8,
    pub logging_level: String,
    pub window: WindowState,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct WindowState {
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ApplicationGroup {
    pub name: String,
    pub paths: Vec<String>,
}
impl Default for WindowState {
    fn default() -> Self {
        Self {
            width: 1240,
            height: 850,
            maximized: false,
        }
    }
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            language: "ru".into(),
            theme: "dark".into(),
            accent: "#8B7CF8".into(),
            surface: "#12151E".into(),
            radius: 16,
            density: "comfortable".into(),
            motion: true,
            route_mode: RouteMode::Global,
            applications: vec![],
            bypass_domains: vec![],
            proxy_port: 2080,
            system_proxy: false,
            tun: false,
            dns: Dns::System,
            dns_route: "direct".into(),
            mtu: 1500,
            network_preset: "custom".into(),
            applications_migrated: false,
            probe_url: "https://www.gstatic.com/generate_204".into(),
            transport_engine: "auto".into(),
            tls_defaults: None,
            fake_ip: false,
            routing_rules: vec![],
            sniff_excluded_apps: vec![],
            sniffing: Default::default(),
            fragmentation: Default::default(),
            multiplex: Default::default(),
            ping_method: "tcp_connect_rtt".into(),
            ping_proxy_mode: "keepalive".into(),
            selected_profile: None,
            favorites: vec![],
            whitelist: vec![],
            blacklist: vec![],
            app_view: "table".into(),
            server_view: "table".into(),
            autostart: false,
            close_to_tray: true,
            minimize_to_tray: false,
            application_groups: vec![],
            auto_reconnect: false,
            auto_failover: false,
            selection_samples: 10,
            logging_level: "INFO".into(),
            window: WindowState::default(),
        }
    }
}
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum Dns {
    #[default]
    System,
    Udp {
        server: String,
    },
    Https {
        server: String,
        path: String,
    },
    Tls {
        server: String,
    },
}
impl Preferences {
    pub fn for_startup(self) -> Self {
        let mut p = self.normalized();
        if !p.auto_reconnect {
            p.tun = false;
            p.system_proxy = false;
        }
        p
    }

    pub fn normalized(mut self) -> Self {
        if !self.applications_migrated {
            self.applications.extend(self.whitelist.clone());
            self.applications.extend(self.blacklist.clone());
            self.applications_migrated = true;
        }
        self.whitelist.clear();
        self.blacklist.clear();
        if self.route_mode == RouteMode::Direct {
            self.route_mode = RouteMode::Global;
        }

        for view in [&mut self.app_view, &mut self.server_view] {
            if view.as_str() != "table" {
                *view = if view.as_str() == "list" {
                    "table".into()
                } else {
                    "medium".into()
                };
            }
        }

        self.applications = self.applications.iter().map(|p| crate::app_rules::executable_name(p)).collect();
        for group in &mut self.application_groups { group.paths = group.paths.iter().map(|p| crate::app_rules::executable_name(p)).collect(); }
        for paths in [
            &mut self.applications,
            &mut self.whitelist,
            &mut self.blacklist,
        ] {
            paths.sort_by_key(|p| p.to_lowercase());
            paths.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        }
        self.accent = self.accent.to_uppercase();
        self.surface = self.surface.to_uppercase();
        self.favorites.sort();
        self.bypass_domains = self
            .bypass_domains
            .iter()
            .map(|v| crate::routing::normalize_domain(v))
            .filter(|v| !v.is_empty())
            .collect();
        self.bypass_domains.sort();
        self.bypass_domains.dedup();
        self.favorites.dedup();
        for group in &mut self.application_groups {
            group.paths.sort_by_key(|p| p.to_lowercase());
            group.paths.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        }
        self
    }
    pub fn process_paths(&self) -> &[String] {
        &self.applications
    }
    pub fn effective_tun(&self) -> bool {
        self.tun
    }
    pub fn validate(&self) -> Result<()> {
        crate::transport_options::validate(self)?;
        crate::advanced_routing::validate(self)?;
        let fail = |s| Error::new("InvalidSettings", "settings-validate", s);
        if self.bypass_domains.len() > 1000
            || self
                .bypass_domains
                .iter()
                .any(|v| !crate::routing::valid_domain(v))
        {
            return Err(fail(
                "Исключения: до 1000 доменов без схемы, порта и пути (например example.com)",
            ));
        }
        if !(5..=15).contains(&self.selection_samples) {
            return Err(fail("Server selection requires 5–15 samples"));
        }
        if !(1280..=9000).contains(&self.mtu)
            || !["direct", "vpn"].contains(&self.dns_route.as_str())
            || !["balanced", "compatibility", "games", "custom"]
                .contains(&self.network_preset.as_str())
        {
            return Err(fail(
                "MTU must be 1280–9000; select a valid DNS route and preset",
            ));
        }
        if self.dns_route == "vpn" && self.dns == Dns::System {
            return Err(fail(
                "DNS through VPN requires an explicit UDP, TLS or HTTPS resolver",
            ));
        }
        if self.favorites.len() > 20000
            || self.application_groups.len() > 100
            || self.application_groups.iter().any(|g| {
                g.name.trim().is_empty()
                    || g.name.len() > 128
                    || g.name.chars().any(char::is_control)
                    || g.paths.len() > 200
                    || g.paths.iter().any(|p| {
                        !crate::app_rules::valid_executable(p)
                            || p.chars().any(char::is_control)
                    })
            })
        {
            return Err(fail("Invalid favorites or application groups"));
        }
        let views = [
            "huge", "large", "medium", "small", "list", "table", "tiles", "content", "cards",
        ];
        if !views.contains(&self.app_view.as_str())
            || !views.contains(&self.server_view.as_str())
            || !["DEBUG", "INFO", "WARNING", "ERROR"].contains(&self.logging_level.as_str())
        {
            return Err(fail("Unknown application view or log level"));
        }
        if self.window.width < 960
            || self.window.width > 7680
            || self.window.height < 680
            || self.window.height > 4320
        {
            return Err(fail("Invalid saved window dimensions"));
        }
        if !["ru", "en"].contains(&self.language.as_str())
            || !["dark", "light", "system", "custom"].contains(&self.theme.as_str())
        {
            return Err(fail("Unknown language or theme"));
        }
        for color in [&self.accent, &self.surface] {
            if color.len() != 7
                || !color.starts_with('#')
                || !color[1..].chars().all(|c| c.is_ascii_hexdigit())
            {
                return Err(fail("Colors must use #RRGGBB"));
            }
        }
        if self.radius > 32
            || !["compact", "comfortable"].contains(&self.density.as_str())
            || self.proxy_port < 1024
        {
            return Err(fail("Invalid radius, density or local proxy port"));
        }
        for (paths, limit) in [
            (&self.applications, 600),
            (&self.whitelist, 200),
            (&self.blacklist, 200),
        ] {
            if paths.len() > limit
                || paths.iter().any(|p| {
                    !crate::app_rules::valid_executable(p)
                        || p.chars().any(char::is_control)
                })
            {
                return Err(fail(
                    "Rules require executable names ending in .exe (maximum 600 shared entries)",
                ));
            }
        }
        if !["auto", "sing-box", "xray"].contains(&self.transport_engine.as_str()) {
            return Err(fail("Unknown transport engine"));
        }
        if ![
            "proxy_https_rtt",
            "proxy_https_head_rtt",
            "tcp_connect_rtt",
            "icmp_rtt",
        ]
        .contains(&self.ping_method.as_str())
            || !["double", "keepalive"].contains(&self.ping_proxy_mode.as_str())
        {
            return Err(fail("Unknown ping method or proxy mode"));
        }
        let probe = url::Url::parse(&self.probe_url).map_err(|_| fail("Invalid probe URL"))?;
        if probe.scheme() != "https"
            || probe.host_str().is_none()
            || !probe.username().is_empty()
            || probe.password().is_some()
        {
            return Err(fail(
                "Probe URL must use authenticated TLS without URL credentials",
            ));
        }
        match &self.dns {
            Dns::System => (),
            Dns::Udp { server } => {
                if server.parse::<std::net::IpAddr>().is_err() {
                    return Err(fail("UDP DNS requires an IP address"));
                }
            }
            Dns::Https { server, path } => {
                if !path.starts_with('/')
                    || path.contains(['\r', '\n'])
                    || server.is_empty()
                    || server.contains(['/', ':', '@'])
                {
                    return Err(fail("Invalid HTTPS DNS host/path"));
                }
            }
            Dns::Tls { server } => {
                if server.is_empty() || server.contains(['/', ':', '@']) {
                    return Err(fail("Invalid TLS DNS host"));
                }
            }
        }
        Ok(())
    }
    pub fn dns_server(&self) -> Value {
        let mut server = match &self.dns {
            Dns::System => json!({"type":"local","tag":"resolver"}),
            Dns::Udp { server } => json!({"type":"udp","tag":"resolver","server":server}),
            Dns::Https { server, path } => {
                json!({"type":"https","tag":"resolver","server":server,"path":path})
            }
            Dns::Tls { server } => json!({"type":"tls","tag":"resolver","server":server}),
        };
        if self.dns != Dns::System {
            server["detour"] = json!(if self.dns_route == "vpn" {
                "proxy"
            } else {
                "direct"
            });
            if server["server"]
                .as_str()
                .is_some_and(|s| s.parse::<std::net::IpAddr>().is_err())
            {
                server["domain_resolver"] = json!("bootstrap");
            }
        }
        server
    }
}

#[cfg(test)]
mod effective_tun_tests {
    use super::*;
    #[test]
    fn routing_lists_are_independent_of_manual_switches() {
        for mode in [
            RouteMode::Global,
            RouteMode::Whitelist,
            RouteMode::Blacklist,
            RouteMode::Direct,
        ] {
            for tun in [false, true] {
                for system_proxy in [false, true] {
                    let p = Preferences {
                        route_mode: mode.clone(),
                        tun,
                        system_proxy,
                        ..Preferences::default()
                    };
                    p.validate().unwrap();
                    assert_eq!(p.effective_tun(), tun);
                    let profile =
                        crate::imports::parse_uri("socks5://127.0.0.1:1080#Fixture").unwrap();
                    let config =
                        crate::engine::configuration(&profile, &p, 24567, "api", "proxy").unwrap();
                    assert_eq!(
                        config["inbounds"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|v| v["type"] == "tun"),
                        p.effective_tun()
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod launch_tests {
    use super::*;
    #[test]
    fn fresh_install_uses_tcp_but_saved_ping_choice_is_preserved() {
        assert_eq!(Preferences::default().ping_method, "tcp_connect_rtt");
        let saved = Preferences {
            ping_method: "proxy_https_head_rtt".into(),
            ..Preferences::default()
        };
        assert_eq!(saved.for_startup().ping_method, "proxy_https_head_rtt");
    }
    #[test]
    fn manual_switches_reset_without_changing_lists_or_autoconnect_intent() {
        let saved = Preferences {
            tun: true,
            system_proxy: true,
            whitelist: vec![r"C:\Apps\fixture.exe".into()],
            route_mode: RouteMode::Whitelist,
            ..Preferences::default()
        };
        let launch = saved.clone().for_startup();
        assert!(!launch.tun && !launch.system_proxy);
        assert_eq!(launch.applications, saved.whitelist.iter().map(|p|crate::app_rules::executable_name(p)).collect::<Vec<_>>());
        assert!(!launch.effective_tun());
        let automatic = Preferences {
            auto_reconnect: true,
            ..saved
        }
        .for_startup();
        assert!(automatic.tun && automatic.system_proxy);
    }
}

#[cfg(test)]
mod shared_network_tests {
    use super::*;
    #[test]
    fn old_app_lists_migrate_once_and_survive_mode_changes() {
        let p = Preferences {
            applications: vec![r"C:\Apps\one.exe".into()],
            whitelist: vec![r"C:\Apps\two.exe".into()],
            blacklist: vec![r"c:\apps\ONE.exe".into()],
            route_mode: RouteMode::Direct,
            ..Preferences::default()
        }
        .normalized();
        assert_eq!(p.applications.len(), 2);
        assert!(p.whitelist.is_empty() && p.blacklist.is_empty());
        assert_eq!(p.route_mode, RouteMode::Global);
        for mode in [
            RouteMode::Global,
            RouteMode::Whitelist,
            RouteMode::Blacklist,
        ] {
            let next = Preferences {
                route_mode: mode,
                ..p.clone()
            }
            .normalized();
            assert_eq!(next.process_paths(), p.applications);
            assert!(!next.effective_tun());
        }
        let full = Preferences {
            applications: (0..200).map(|i| format!(r"C:\Apps\a{i}.exe")).collect(),
            whitelist: (0..200).map(|i| format!(r"C:\Apps\b{i}.exe")).collect(),
            blacklist: (0..200).map(|i| format!(r"C:\Apps\c{i}.exe")).collect(),
            ..Preferences::default()
        }
        .normalized();
        assert_eq!(full.applications.len(), 600);
        full.validate().unwrap();
        let removed = Preferences {
            applications: vec![],
            ..p
        }
        .normalized();
        assert!(removed.process_paths().is_empty());
    }
    #[test]
    fn mtu_and_dns_route_validate_and_build_without_dns_bootstrap_cycle() {
        let profile = crate::imports::parse_uri("socks5://fixture:pass@example.org:1080").unwrap();
        for dns_route in ["direct", "vpn"] {
            let p = Preferences {
                mtu: 1400,
                tun: true,
                dns_route: dns_route.into(),
                dns: Dns::Https {
                    server: "dns.example.org".into(),
                    path: "/dns-query".into(),
                },
                ..Preferences::default()
            };
            let c = crate::engine::configuration(&profile, &p, 2081, "api", "proxy").unwrap();
            assert_eq!(c["inbounds"][1]["mtu"], 1400);
            assert_eq!(
                c["dns"]["servers"][0]["detour"],
                if dns_route == "vpn" {
                    "proxy"
                } else {
                    "direct"
                }
            );
            assert_eq!(c["dns"]["servers"][0]["domain_resolver"], "bootstrap");
            assert_eq!(c["route"]["default_domain_resolver"], "bootstrap");
        }
        for mtu in [0, 1279, 9001] {
            assert!(
                Preferences {
                    mtu,
                    ..Preferences::default()
                }
                .validate()
                .is_err()
            );
        }
        assert!(
            Preferences {
                dns_route: "vpn".into(),
                ..Preferences::default()
            }
            .validate()
            .is_err()
        );
    }
}
