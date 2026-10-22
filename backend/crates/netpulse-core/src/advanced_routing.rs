use crate::{Error, Result, model::Preferences};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub enabled: bool,
    pub kind: String,
    pub value: String,
    pub action: String,
}
pub fn validate(p: &Preferences) -> Result<()> {
    let fail = || {
        Error::new(
            "InvalidRoutingRule",
            "routing-rules",
            "Правило: укажите домен, IP/CIDR, имя EXE, GeoIP (ru/de/…), либо имя набора Geosite; действие: proxy / direct / reject",
        )
    };
    if p.routing_rules.len() > 64 || p.sniff_excluded_apps.len() > 200 {
        return Err(fail());
    }
    let mut sets = BTreeSet::new();
    for r in &p.routing_rules {
        if !["proxy", "direct", "reject"].contains(&r.action.as_str()) {
            return Err(fail());
        }
        let v = r.value.trim();
        let good = match r.kind.as_str() {
            "domain" => crate::routing::valid_domain(v),
            "cidr" => {
                let (ip, bits) = v.split_once('/').map_or((v, None), |(a, b)| (a, Some(b)));
                ip.parse::<std::net::IpAddr>().is_ok_and(|ip| {
                    bits.is_none_or(|s| {
                        s.parse::<u8>()
                            .is_ok_and(|n| n <= if ip.is_ipv4() { 32 } else { 128 })
                    })
                })
            }
            "process" => crate::app_rules::valid_executable(v) && !v.chars().any(char::is_control),
            "geoip" => {
                sets.insert(format!("geoip-{}", v.to_ascii_lowercase()));
                v.len() == 2 && v.bytes().all(|c| c.is_ascii_alphabetic())
            }
            "geosite" => {
                sets.insert(format!("geosite-{}", v.to_ascii_lowercase()));
                !v.is_empty()
                    && v.len() <= 64
                    && v.bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"-!_".contains(&c))
            }
            _ => false,
        };
        if !good {
            return Err(fail());
        }
    }
    if sets.len() > 8 {
        return Err(Error::new(
            "RuleSetLimit",
            "routing-rules",
            "Не более 8 внешних наборов GeoIP / Geosite",
        ));
    }
    for path in &p.sniff_excluded_apps {
        if path.len() > 32768
            || !path.to_ascii_lowercase().ends_with(".exe")
            || path.chars().any(char::is_control)
        {
            return Err(fail());
        }
    }
    Ok(())
}
pub fn needs_names(p: &Preferences) -> bool {
    !p.bypass_domains.is_empty()
        || p.routing_rules
            .iter()
            .any(|r| r.enabled && ["domain", "geosite"].contains(&r.kind.as_str()))
}
pub fn append_rules(rules: &mut Vec<Value>, p: &Preferences) {
    if p.routing_rules
        .iter()
        .any(|r| r.enabled && ["cidr", "geoip"].contains(&r.kind.as_str()))
    {
        rules.push(json!({"action":"resolve","server":"resolver","strategy":"prefer_ipv4"}));
    }
    for r in p.routing_rules.iter().filter(|r| r.enabled) {
        let value = r.value.trim();
        let mut rule = match r.kind.as_str() {
            "domain" => json!({"domain_suffix":[crate::routing::normalize_domain(value)]}),
            "cidr" => json!({"ip_cidr":[value]}),
            "process" => {
                json!({"process_path_regex":crate::app_rules::process_patterns(&[value.into()])})
            }
            _ => json!({"rule_set":[format!("{}-{}",r.kind,value.to_ascii_lowercase())]}),
        };
        if r.action == "reject" {
            rule["action"] = json!("reject");
        } else {
            rule["action"] = json!("route");
            rule["outbound"] = json!(r.action);
        }
        rules.push(rule);
    }
}
pub fn apply(config: &mut Value, p: &Preferences) {
    let mut sets = BTreeSet::new();
    let mut remote = Vec::new();
    for r in p
        .routing_rules
        .iter()
        .filter(|r| r.enabled && ["geoip", "geosite"].contains(&r.kind.as_str()))
    {
        let tag = format!("{}-{}", r.kind, r.value.trim().to_ascii_lowercase());
        if sets.insert(tag.clone()) {
            remote.push(json!({"type":"remote","tag":tag,"format":"binary","url":format!("https://raw.githubusercontent.com/SagerNet/sing-{}/rule-set/{}.srs",r.kind,tag),"http_client":{"detour":"direct"},"update_interval":"24h"}));
        }
    }
    if !remote.is_empty() {
        config["route"]["rule_set"] = json!(remote);
    }
    if needs_names(p) {
        config["dns"]["reverse_mapping"] = json!(true);
    }
    if p.fake_ip && p.effective_tun() {
        config["dns"]["servers"].as_array_mut().unwrap().push(json!({"type":"fakeip","tag":"fakeip","inet4_range":"198.18.0.0/15","inet6_range":"fc00::/18"}));
        if !config["dns"]["rules"].is_array() {
            config["dns"]["rules"] = json!([]);
        }
        // Explicit bypasses retain real addresses; remaining A/AAAA queries map
        // only inside the owned TUN. Upstream resolutions still use resolver.
        config["dns"]["rules"]
            .as_array_mut()
            .unwrap()
            .push(json!({"query_type":["A","AAAA"],"action":"route","server":"fakeip"}));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_networks_and_prevents_rule_set_url_injection() {
        for (kind, value, valid) in [
            ("cidr", "192.0.2.0/24", true),
            ("cidr", "2001:db8::/32", true),
            ("cidr", "127.0.0.1/99", false),
            ("geoip", "de", true),
            ("geoip", "../../x", false),
            ("domain", "https://example.org", false),
            ("geosite", "category-games", true),
        ] {
            let p = Preferences {
                routing_rules: vec![Rule {
                    enabled: true,
                    kind: kind.into(),
                    value: value.into(),
                    action: "direct".into(),
                }],
                ..Default::default()
            };
            assert_eq!(validate(&p).is_ok(), valid, "{kind}: {value}");
        }
    }
    #[test]
    fn exceptions_and_dns_precede_ordered_rules_and_application_defaults() {
        let profile = crate::imports::parse_uri("socks5://127.0.0.1:1080").unwrap();
        let p = Preferences {
            tun: true,
            fake_ip: true,
            bypass_domains: vec!["bypass.test".into()],
            routing_rules: vec![Rule {
                enabled: true,
                kind: "cidr".into(),
                value: "192.0.2.0/24".into(),
                action: "reject".into(),
            }],
            ..Default::default()
        };
        let c = crate::engine::configuration(&profile, &p, 20001, "api", "proxy").unwrap();
        let rules = c["route"]["rules"].as_array().unwrap();
        assert_eq!(rules[1]["action"], "hijack-dns");
        let bypass = rules
            .iter()
            .position(|r| r["domain_suffix"].is_array())
            .unwrap();
        let reject = rules.iter().position(|r| r["action"] == "reject").unwrap();
        assert!(bypass < reject);
        assert_eq!(c["dns"]["rules"][1]["server"], "fakeip");
        let mut no_tun = p.clone();
        no_tun.tun = false;
        let c = crate::engine::configuration(&profile, &no_tun, 20001, "api", "proxy").unwrap();
        assert!(
            !c["dns"]["servers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s["type"] == "fakeip")
        );
    }
    #[cfg(windows)]
    #[test]
    #[ignore = "Bundled sing-box schema checks only; never starts TUN or downloads rule sets"]
    fn bundled_engine_accepts_fakeip_geoip_geosite_and_process_sniff_exclusions() {
        let engine = std::path::PathBuf::from(std::env::var_os("NETPULSE_TEST_ENGINE").unwrap());
        let profile = crate::imports::parse_uri("socks5://127.0.0.1:1080").unwrap();
        let p = Preferences {
            tun: true,
            fake_ip: true,
            sniff_excluded_apps: vec![r"C:\Game\RobloxPlayerBeta.exe".into()],
            routing_rules: vec![
                Rule {
                    enabled: true,
                    kind: "geoip".into(),
                    value: "de".into(),
                    action: "direct".into(),
                },
                Rule {
                    enabled: true,
                    kind: "geosite".into(),
                    value: "category-games".into(),
                    action: "proxy".into(),
                },
                Rule {
                    enabled: true,
                    kind: "cidr".into(),
                    value: "192.0.2.0/24".into(),
                    action: "reject".into(),
                },
            ],
            ..Default::default()
        };
        p.validate().unwrap();
        let c = crate::engine::configuration(
            &profile,
            &p,
            crate::runtime::free_port().unwrap(),
            "api",
            "fixture",
        )
        .unwrap();
        crate::engine::Engine::validate(&engine, &c).unwrap();
    }
    #[cfg(windows)]
    #[test]
    #[ignore = "Requires bundled sing-box; owned UDP DNS only, no TUN or system DNS changes"]
    fn fakeip_and_dns_hijack_keep_bypass_answers_real() {
        use std::{
            net::{TcpStream, UdpSocket},
            time::Duration,
        };
        let upstream = UdpSocket::bind("127.0.0.1:0").unwrap();
        upstream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let upstream_port = upstream.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let mut b = [0; 2048];
            let (n, peer) = upstream.recv_from(&mut b).unwrap();
            let mut answer = b[..n].to_vec();
            answer[2] = 0x81;
            answer[3] = 0x80;
            answer[6] = 0;
            answer[7] = 1;
            answer.extend_from_slice(&[0xc0, 0x0c, 0, 1, 0, 1, 0, 0, 0, 60, 0, 4, 192, 0, 2, 42]);
            upstream.send_to(&answer, peer).unwrap();
        });
        let engine = std::path::PathBuf::from(std::env::var_os("NETPULSE_TEST_ENGINE").unwrap());
        let profile = crate::imports::parse_uri("socks5://127.0.0.1:1080").unwrap();
        let prefs = Preferences {
            tun: true,
            fake_ip: true,
            proxy_port: crate::runtime::free_port().unwrap(),
            bypass_domains: vec!["bypass.invalid".into()],
            ..Default::default()
        };
        let api = crate::runtime::free_port().unwrap();
        let port = UdpSocket::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let mut config =
            crate::engine::configuration(&profile, &prefs, api, "fixture-api", "fixture-proxy")
                .unwrap();
        config["inbounds"]
            .as_array_mut()
            .unwrap()
            .retain(|i| i["type"] != "tun");
        config["inbounds"].as_array_mut().unwrap().push(json!({"type":"direct","tag":"owned-dns","listen":"127.0.0.1","listen_port":port,"network":"udp"}));
        config["route"]["rules"]
            .as_array_mut()
            .unwrap()
            .insert(0, json!({"inbound":"owned-dns","action":"hijack-dns"}));
        config["dns"]["servers"][0] =
            json!({"type":"udp","tag":"resolver","server":"127.0.0.1","server_port":upstream_port});
        config["dns"]["rules"][0]["server"] = json!("resolver");
        crate::engine::Engine::validate(&engine, &config).unwrap();
        let mut process = crate::engine::Engine::spawn(&engine, &config, false).unwrap();
        for _ in 0..100 {
            if TcpStream::connect(("127.0.0.1", api)).is_ok() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(4)))
            .unwrap();
        for (domain, fake) in [("mapped.invalid", true), ("bypass.invalid", false)] {
            let mut query = vec![0x12, 0x34, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0];
            for label in domain.split('.') {
                query.push(label.len() as u8);
                query.extend_from_slice(label.as_bytes());
            }
            query.extend_from_slice(&[0, 0, 1, 0, 1]);
            socket.send_to(&query, ("127.0.0.1", port)).unwrap();
            let mut b = [0; 2048];
            let (n, _) = socket.recv_from(&mut b).unwrap();
            assert_eq!(&b[..2], &[0x12, 0x34]);
            assert_eq!(b[3] & 15, 0);
            assert_eq!(u16::from_be_bytes([b[6], b[7]]), 1);
            let ip = &b[n - 4..n];
            if fake {
                assert!(
                    ip[0] == 198 && [18, 19].contains(&ip[1]),
                    "Fake IP missing: {ip:?}"
                );
            } else {
                assert_eq!(ip, &[192, 0, 2, 42]);
            }
        }
        process.stop().unwrap();
        server.join().unwrap();
    }
}
