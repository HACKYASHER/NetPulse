//! Opt-in transport settings. Defaults leave the original profile unchanged.
use crate::{
    Error, Result,
    model::{Preferences, Profile},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Fragmentation {
    pub enabled: bool,
    pub packets: String,
    pub length: String,
    pub interval: String,
    pub max_split: String,
}
impl Default for Fragmentation {
    fn default() -> Self {
        Self {
            enabled: false,
            packets: "tlshello".into(),
            length: "50-100".into(),
            interval: "10-20".into(),
            max_split: "100-200".into(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Multiplex {
    pub enabled: bool,
    pub concurrency: i16,
    pub xudp_concurrency: i16,
    pub udp_443: String,
}
impl Default for Multiplex {
    fn default() -> Self {
        Self {
            enabled: false,
            concurrency: 8,
            xudp_concurrency: 8,
            udp_443: "skip".into(),
        }
    }
}
fn invalid(message: &str) -> Error {
    Error::new("InvalidTransportOptions", "transport-options", message)
}
fn range(value: &str, min: u32, max: u32) -> bool {
    let parts: Vec<_> = value.split('-').collect();
    if parts.is_empty() || parts.len() > 2 {
        return false;
    }
    let nums: Option<Vec<u32>> = parts
        .iter()
        .map(|s| {
            if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
                None
            } else {
                s.parse().ok()
            }
        })
        .collect();
    nums.is_some_and(|v| v[0] >= min && *v.last().unwrap() <= max && v[0] <= *v.last().unwrap())
}
pub fn validate(p: &Preferences) -> Result<()> {
    if p.tls_defaults.as_ref().is_some_and(|v| {
        !["inherit", "auto", "custom", "disabled"].contains(&v.sni_mode.as_str())
            || (["custom", "disabled"].contains(&v.sni_mode.as_str())
                && !valid_name(&v.server_name, false))
            || v.http_host
                .as_ref()
                .is_some_and(|h| !h.is_empty() && !valid_name(h, true))
            || v.fingerprint.as_ref().is_some_and(|f| {
                ![
                    "disabled",
                    "chrome",
                    "firefox",
                    "edge",
                    "safari",
                    "ios",
                    "android",
                    "randomized",
                ]
                .contains(&f.as_str())
            })
    }) {
        return Err(invalid(
            "Проверьте общие SNI, HTTP Host и отпечаток TLS. Указывайте имя без схемы, пути и переноса строк.",
        ));
    }
    let f = &p.fragmentation;
    if !["tlshello", "1-3"].contains(&f.packets.as_str())
        || !range(&f.length, 1, 65535)
        || !range(&f.interval, 0, 1000)
        || !range(&f.max_split, 1, 65535)
    {
        return Err(invalid(
            "Фрагментация: длина и maxSplit 1–65535, интервал 0–1000 мс; используйте число или мин-макс",
        ));
    }
    let m = &p.multiplex;
    if !(-1..=128).contains(&m.concurrency)
        || !(-1..=1024).contains(&m.xudp_concurrency)
        || !["reject", "allow", "skip"].contains(&m.udp_443.as_str())
    {
        return Err(invalid(
            "Mux: TCP от -1 до 128, XUDP от -1 до 1024, QUIC: reject / allow / skip",
        ));
    }
    Ok(())
}
pub fn use_xray(profile: &Profile, prefs: &Preferences) -> Result<bool> {
    let advanced = prefs.fragmentation.enabled || prefs.multiplex.enabled;
    let supported = ["vless", "trojan", "shadowsocks", "socks", "http"]
        .contains(&profile.outbound["type"].as_str().unwrap_or(""));
    if prefs.transport_engine == "xray" && !supported {
        return Err(invalid("Xray не поддерживает этот протокол. Для Hysteria2 выберите sing-box или Автоматически."));
    }
    let required = profile.requires_xray() || advanced;
    if (required && prefs.transport_engine == "sing-box") || (advanced && !supported) {
        return Err(invalid(
            "Фрагментация и Mux требуют совместимый профиль Xray. Для Hysteria2 выключите эти функции; для остальных выберите Xray или Автоматически.",
        ));
    }
    let use_xray = required
        || (prefs.transport_engine == "auto" && profile.outbound["transport"]["type"] == "grpc")
        || (prefs.transport_engine == "xray" && supported);
    if use_xray && profile.outbound["tls"]["disable_sni"] == true {
        return Err(invalid(
            "Отключение SNI доступно только с sing-box без Reality. Xray требует имя сервера. Проверка сертификатов остаётся включённой.",
        ));
    }
    if prefs.fragmentation.enabled
        && prefs.fragmentation.packets == "tlshello"
        && profile.outbound["tls"]["enabled"] != true
    {
        return Err(invalid("Фрагментация tlshello требует TLS у VPN-сервера"));
    }
    if prefs.multiplex.enabled
        && (!["vless", "trojan"].contains(&profile.outbound["type"].as_str().unwrap_or(""))
            || !profile.outbound["flow"].as_str().unwrap_or("").is_empty()
            || ["grpc", "xhttp"]
                .contains(&profile.outbound["transport"]["type"].as_str().unwrap_or("")))
    {
        return Err(invalid(
            "Mux доступен для VLESS / Trojan TCP и WebSocket без Vision. gRPC, XHTTP и Hysteria2 используют собственный транспорт; выключите Mux.",
        ));
    }
    Ok(use_xray)
}
pub fn apply_xray(config: &mut Value, prefs: &Preferences) {
    if prefs.multiplex.enabled {
        let m = &prefs.multiplex;
        config["outbounds"][0]["mux"] = json!({"enabled":true,"concurrency":m.concurrency,"xudpConcurrency":m.xudp_concurrency,"xudpProxyUDP443":m.udp_443});
    }
    if prefs.fragmentation.enabled {
        let f = &prefs.fragmentation;
        config["outbounds"][0]["streamSettings"]["sockopt"] =
            json!({"dialerProxy":"netpulse-fragment"});
        config["outbounds"].as_array_mut().unwrap().push(json!({"tag":"netpulse-fragment","protocol":"freedom","settings":{"fragment":{"packets":f.packets,"length":f.length,"interval":f.interval,"maxSplit":f.max_split}}}));
    }
}

/// These overrides are sealed with credentials, while the subscription fingerprint
/// continues to refer to the unmodified upstream outbound.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProfileTransport {
    pub sni_mode: String,
    pub server_name: String,
    pub http_host: Option<String>,
    #[serde(default)]
    pub fingerprint: Option<String>,
}
#[derive(Serialize)]
pub struct ProfileTransportView {
    pub settings: ProfileTransport,
    pub original_sni: String,
    pub original_host: String,
    pub tls: bool,
    pub host_supported: bool,
    pub reality: bool,
}
fn valid_name(name: &str, allow_port: bool) -> bool {
    !name.is_empty()
        && name.len() <= 253
        && name.is_ascii()
        && name.bytes().all(|b| {
            b.is_ascii_alphanumeric() || b".-_".contains(&b) || (allow_port && b":[]".contains(&b))
        })
}
fn http_host(p: &Profile) -> String {
    let t = &p.outbound["transport"];
    t["headers"]["Host"]
        .as_str()
        .or_else(|| t["headers"]["host"].as_str())
        .or_else(|| t["host"].as_str())
        .unwrap_or("")
        .into()
}
impl Profile {
    pub fn resolved_with(&self, prefs: &Preferences) -> Result<Self> {
        let mut p = self.resolved()?;
        if let Some(defaults) = &prefs.tls_defaults {
            let mut v = defaults.clone();
            let view = p.transport_view();
            if !view.tls {
                v.sni_mode = "inherit".into();
                v.fingerprint = None;
            }
            if p.outbound["type"] == "hysteria2" {
                v.fingerprint = None;
            }
            if !view.host_supported {
                v.http_host = None;
            }
            p.transport_override = Some(v);
            p = p.resolved()?;
        }
        Ok(p)
    }
    pub fn transport_view(&self) -> ProfileTransportView {
        ProfileTransportView {
            settings: self.transport_override.clone().unwrap_or(ProfileTransport {
                sni_mode: "inherit".into(),
                ..Default::default()
            }),
            original_sni: self.outbound["tls"]["server_name"]
                .as_str()
                .unwrap_or("")
                .into(),
            original_host: http_host(self),
            tls: self.outbound["tls"]["enabled"] == true,
            host_supported: ["ws", "xhttp", "httpupgrade"]
                .contains(&self.outbound["transport"]["type"].as_str().unwrap_or("")),
            reality: self.outbound["tls"]["reality"]["enabled"] == true,
        }
    }
    pub fn resolved(&self) -> Result<Self> {
        let mut p = self.clone();
        if let Some(outbound) = p.outbound_override.take() {
            p.outbound = outbound;
            p.validate()?;
        }
        let Some(v) = p.transport_override.take() else {
            return Ok(p);
        };
        if v.server_name.len() > 253 {
            return Err(invalid("Имя TLS-сервера длиннее 253 байт"));
        }
        if !["inherit", "auto", "custom", "disabled"].contains(&v.sni_mode.as_str()) {
            return Err(invalid("Неизвестный режим SNI"));
        }
        if v.sni_mode != "inherit" {
            if p.outbound["tls"]["enabled"] != true {
                return Err(invalid("У профиля нет TLS: SNI не используется"));
            }
            let tls = p.outbound["tls"].as_object_mut().unwrap();
            tls.remove("disable_sni");
            match v.sni_mode.as_str() {
                "auto" => {
                    tls.remove("server_name");
                }
                "custom" | "disabled" => {
                    if !valid_name(&v.server_name, false) {
                        return Err(invalid(
                            "Укажите имя сервера для проверки сертификата (без схемы, пути и порта)",
                        ));
                    }
                    if v.sni_mode == "disabled"
                        && tls.get("reality").is_some_and(|r| r["enabled"] == true)
                    {
                        return Err(invalid("Reality требует SNI"));
                    }
                    tls.insert("server_name".into(), json!(v.server_name));
                    if v.sni_mode == "disabled" {
                        tls.insert("disable_sni".into(), json!(true));
                    }
                }
                _ => (),
            }
        }
        if let Some(fp) = v.fingerprint {
            if p.outbound["tls"]["enabled"] != true || p.outbound["type"] == "hysteria2" {
                return Err(invalid(
                    "uTLS доступен для TCP TLS-профилей; Hysteria2 / QUIC не поддерживается",
                ));
            }
            if ![
                "disabled",
                "chrome",
                "firefox",
                "edge",
                "safari",
                "ios",
                "android",
                "randomized",
            ]
            .contains(&fp.as_str())
            {
                return Err(invalid("Неподдерживаемый отпечаток uTLS"));
            }
            if fp == "disabled" && p.outbound["tls"]["reality"]["enabled"] == true {
                return Err(invalid("Reality требует uTLS"));
            }
            p.outbound["tls"]["utls"] = if fp == "disabled" {
                json!({"enabled":false})
            } else {
                json!({"enabled":true,"fingerprint":fp})
            };
        }
        if let Some(host) = v.http_host {
            if !host.is_empty() && !valid_name(&host, true) {
                return Err(invalid(
                    "HTTP Host: укажите имя, при необходимости с портом, без пути и перевода строк",
                ));
            }
            match p.outbound["transport"]["type"].as_str().unwrap_or("") {
                "ws" => {
                    if !p.outbound["transport"]["headers"].is_object() {
                        p.outbound["transport"]["headers"] = json!({});
                    }
                    let headers = p.outbound["transport"]["headers"].as_object_mut().unwrap();
                    headers.retain(|key, _| !key.eq_ignore_ascii_case("host"));
                    if !host.is_empty() {
                        headers.insert("Host".into(), json!(host));
                    }
                }
                "xhttp" | "httpupgrade" => {
                    p.outbound["transport"]["host"] = json!(host);
                }
                _ => {
                    return Err(invalid(
                        "HTTP Host поддерживается только для WebSocket, XHTTP и HTTPUpgrade",
                    ));
                }
            }
        }
        p.validate()?;
        Ok(p)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn global_transport_defaults_apply_by_capability_and_keep_source_unchanged() {
        let prefs = Preferences {
            tls_defaults: Some(ProfileTransport {
                sni_mode: "custom".into(),
                server_name: "global.example.org".into(),
                http_host: Some("http.example.org".into()),
                fingerprint: Some("chrome".into()),
            }),
            ..Preferences::default()
        };
        validate(&prefs).unwrap();
        let tls = crate::imports::parse_uri("trojan://fixture@example.org:443?type=ws").unwrap();
        let resolved = tls.resolved_with(&prefs).unwrap();
        assert_eq!(
            resolved.outbound["tls"]["server_name"],
            "global.example.org"
        );
        assert_eq!(
            resolved.outbound["transport"]["headers"]["Host"],
            "http.example.org"
        );
        assert_ne!(tls.outbound["tls"]["server_name"], "global.example.org");
        let hy = crate::imports::parse_uri("hy2://fixture@example.org:443")
            .unwrap()
            .resolved_with(&prefs)
            .unwrap();
        assert!(hy.outbound["tls"].get("utls").is_none());
        let plain = crate::imports::parse_uri("socks5://example.org:1080").unwrap();
        assert_eq!(
            plain.resolved_with(&prefs).unwrap().outbound,
            plain.outbound
        );
    }

    use super::*;
    #[test]
    fn explicit_incompatible_engine_is_rejected_instead_of_silent_fallback() {
        let profile=crate::imports::parse_uri("hysteria2://fixture@example.org:443").unwrap();
        for transport_engine in ["auto","sing-box"] {
            let prefs=Preferences{transport_engine:transport_engine.into(),..Preferences::default()};
            assert!(!use_xray(&profile,&prefs).unwrap());
        }
        let prefs=Preferences{transport_engine:"xray".into(),..Preferences::default()};
        assert!(use_xray(&profile,&prefs).is_err());
    }
    #[test]
    fn overrides_preserve_credentials_and_original_and_reject_injection() {
        let mut p = crate::imports::parse_uri(
            "trojan://secret@example.org:443?type=ws&host=original.org#Fixture",
        )
        .unwrap();
        let original = p.outbound.clone();
        p.transport_override = Some(ProfileTransport {
            sni_mode: "custom".into(),
            server_name: "tls.example.org".into(),
            http_host: Some("host.example.org".into()),
            fingerprint: None,
        });
        let resolved = p.resolved().unwrap();
        assert_eq!(resolved.outbound["tls"]["server_name"], "tls.example.org");
        assert_eq!(
            resolved.outbound["transport"]["headers"]["Host"],
            "host.example.org"
        );
        assert_eq!(resolved.outbound["password"], "secret");
        assert_eq!(p.outbound, original);
        p.transport_override.as_mut().unwrap().http_host = Some("x\r\nInjected: yes".into());
        assert!(p.resolved().is_err());
    }
    #[test]
    fn advanced_defaults_and_capability_checks() {
        let p = crate::imports::parse_uri("trojan://fixture@example.org:443#Fixture").unwrap();
        let mut prefs = Preferences::default();
        assert!(!use_xray(&p, &prefs).unwrap());
        prefs.fragmentation.enabled = true;
        assert!(use_xray(&p, &prefs).unwrap());
        prefs.fragmentation.length = "200-10".into();
        assert!(validate(&prefs).is_err());
        prefs.fragmentation = Fragmentation::default();
        prefs.multiplex.enabled = true;
        prefs.transport_engine = "sing-box".into();
        assert!(use_xray(&p, &prefs).is_err());
        prefs.transport_engine = "auto".into();
        let hy = crate::imports::parse_uri("hysteria2://fixture@example.org:443#Fixture").unwrap();
        assert!(use_xray(&hy, &prefs).is_err());
    }
    #[cfg(windows)]
    #[test]
    #[ignore = "Requires bundled cores; validates configuration only, no OS network changes"]
    fn bundled_cores_accept_fragment_mux_sni_and_fingerprints() {
        let xray = std::path::PathBuf::from(std::env::var_os("NETPULSE_TEST_XRAY").unwrap());
        let sing = std::path::PathBuf::from(std::env::var_os("NETPULSE_TEST_ENGINE").unwrap());
        let mut p = crate::imports::parse_uri("trojan://fixture@example.org:443#Fixture").unwrap();
        for fingerprint in [
            "chrome",
            "firefox",
            "edge",
            "safari",
            "ios",
            "android",
            "randomized",
            "disabled",
        ] {
            p.transport_override = Some(ProfileTransport {
                sni_mode: "custom".into(),
                server_name: "tls.example.org".into(),
                http_host: None,
                fingerprint: Some(fingerprint.into()),
            });
            let mut prefs = Preferences::default();
            prefs.fragmentation.enabled = true;
            prefs.multiplex.enabled = true;
            let resolved = p.resolved().unwrap();
            assert!(use_xray(&resolved, &prefs).unwrap());
            let mut config =
                crate::runtime::xray_config(&p, crate::runtime::free_port().unwrap(), "fixture")
                    .unwrap();
            apply_xray(&mut config, &prefs);
            assert_eq!(
                config["outbounds"][0]["streamSettings"]["sockopt"]["dialerProxy"],
                "netpulse-fragment"
            );
            let mut e = crate::engine::Engine::spawn_arguments(
                &xray,
                &config,
                &["run", "-test", "-format", "json", "-c", "stdin:"],
            )
            .unwrap();
            e.wait_checked(&config).unwrap();
            let config =
                crate::engine::configuration(&p, &Preferences::default(), 21001, "api", "fixture")
                    .unwrap();
            crate::engine::Engine::validate(&sing, &config).unwrap();
        }
        p.transport_override = Some(ProfileTransport {
            sni_mode: "disabled".into(),
            server_name: "example.org".into(),
            http_host: None,
            fingerprint: None,
        });
        let resolved = p.resolved().unwrap();
        assert!(use_xray(&resolved, &Preferences::default()).is_ok_and(|v| !v));
        let config =
            crate::engine::configuration(&p, &Preferences::default(), 21001, "api", "fixture")
                .unwrap();
        assert_eq!(config["outbounds"][0]["tls"]["disable_sni"], true);
        assert_ne!(config["outbounds"][0]["tls"]["insecure"], true);
        crate::engine::Engine::validate(&sing, &config).unwrap();
    }
}
