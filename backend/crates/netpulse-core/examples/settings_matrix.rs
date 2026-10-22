//! Offline release verification: persistence plus real bundled-core config checks.
//! Never starts a VPN, edits Windows proxy/DNS, or writes outside the supplied NEW directory.
use netpulse_core::{imports, model::Preferences, runtime::RuntimePlan, store::Store};
use serde_json::{json, Value};
use std::path::PathBuf;

fn preferences(patch: Value) -> Preferences {
    let mut value = serde_json::to_value(Preferences::default()).unwrap();
    for (key, value_) in patch.as_object().unwrap() { value[ key ] = value_.clone(); }
    serde_json::from_value::<Preferences>(value).unwrap().normalized()
}
fn main() {
    let directory = PathBuf::from(std::env::args_os().nth(1).expect("NEW absolute directory required"));
    assert!(directory.is_absolute() && !directory.exists());
    std::fs::create_dir_all(&directory).unwrap();
    let engine = PathBuf::from(std::env::var_os("NETPULSE_TEST_ENGINE").unwrap());
    let mut cases: Vec<(String, Value)> = Vec::new();
    for (field, values) in [
        ("language", json!(["ru","en"])), ("theme", json!(["dark","light","system"])),
        ("transport_engine", json!(["auto","sing-box","xray"])),
        ("route_mode", json!(["global","whitelist","blacklist"])),
        ("logging_level", json!(["DEBUG","INFO","WARNING","ERROR"])),
        ("ping_method", json!(["proxy_https_rtt","proxy_https_head_rtt","tcp_connect_rtt","icmp_rtt"])),
        ("ping_proxy_mode", json!(["double","keepalive"])),
        ("mtu", json!([1280,1360,1400,1420,1500,9000])), ("proxy_port", json!([1024,2080,65535])),
    ] {
        for value in values.as_array().unwrap() {
            cases.push((format!("{field}={value}"), json!({field:value})));
        }
    }
    for field in ["autostart","close_to_tray","minimize_to_tray","auto_reconnect","auto_failover","fake_ip","tun","system_proxy"] {
        for value in [false,true] { cases.push((format!("{field}={value}"),json!({field:value}))); }
    }
    for (preset, mtu) in [("balanced",1500),("compatibility",1400),("games",1500),("custom",1460)] {
        cases.push((format!("MTU preset {preset}"),json!({"network_preset":preset,"mtu":mtu,"tun":true})));
    }
    for dns in [json!({"mode":"system"}),json!({"mode":"udp","server":"1.1.1.1"}),json!({"mode":"udp","server":"2606:4700:4700::1111"}),json!({"mode":"tls","server":"one.one.one.one"}),json!({"mode":"https","server":"cloudflare-dns.com","path":"/dns-query"})] {
        for route in ["direct","vpn"] {
            if dns["mode"]=="system" && route=="vpn" { continue; }
            cases.push((format!("DNS {} {route}",dns["mode"]),json!({"dns":dns,"dns_route":route,"tun":true})));
        }
    }
    for mode in ["inherit","auto","custom","disabled"] {
        cases.push((format!("SNI {mode}"),json!({"tls_defaults":{"sni_mode":mode,"server_name":"example.invalid","http_host":null,"fingerprint":null}})));
    }
    for fingerprint in ["disabled","chrome","firefox","edge","safari","ios","android","randomized"] {
        cases.push((format!("TLS {fingerprint}"),json!({"tls_defaults":{"sni_mode":"inherit","server_name":"","http_host":null,"fingerprint":fingerprint}})));
    }
    for (name, sniffing) in [
        ("Sniffing disabled",json!({"enabled":false,"timeout_ms":300,"http":true,"tls":true,"quic":true})),
        ("Sniff HTTP",json!({"enabled":true,"timeout_ms":10,"http":true,"tls":false,"quic":false})),
        ("Sniff TLS",json!({"enabled":true,"timeout_ms":1000,"http":false,"tls":true,"quic":false})),
        ("Sniff QUIC",json!({"enabled":true,"timeout_ms":125,"http":false,"tls":false,"quic":true})),
    ] {cases.push((name.into(),json!({"sniffing":sniffing,"tun":true})));}
    cases.push(("Gaming DNS and independent modes".into(),json!({"network_preset":"games","mtu":1467,"tun":true,"system_proxy":true,"dns":{"mode":"https","server":"1.1.1.1","path":"/dns-query"},"dns_route":"vpn"})));
    cases.extend([
        ("HTTP Host".into(), json!({"tls_defaults":{"sni_mode":"inherit","server_name":"","http_host":"example.invalid","fingerprint":null}})),
        ("EXE routes and domain exceptions".into(),json!({"applications":["Steam.EXE","C:\\Another\\steam.exe","browser.exe"],"bypass_domains":[".ru","*.su","example.com"],"route_mode":"blacklist","tun":true})),
        ("Custom domain routes".into(),json!({"routing_rules":[{"enabled":true,"kind":"domain","value":".ru","action":"direct"},{"enabled":true,"kind":"domain","value":"example.com","action":"proxy"},{"enabled":true,"kind":"domain","value":"blocked.example","action":"reject"}],"tun":true})),
        ("Application group".into(),json!({"application_groups":[{"name":"Browser","paths":["C:\\App\\browser.exe"]}]})),
        ("Sniff exclusions".into(),json!({"sniff_excluded_apps":["browser.exe"],"tun":true})),
        ("Fragmentation".into(),json!({"fragmentation":{"enabled":true,"packets":"tlshello","length":"50-100","interval":"10-20","max_split":"100-200"}})),
        ("Multiplex".into(),json!({"multiplex":{"enabled":true,"concurrency":8,"xudp_concurrency":8,"udp_443":"skip"}})),
        ("Fragmentation and multiplex".into(),json!({"fragmentation":{"enabled":true},"multiplex":{"enabled":true}})),
        ("Mux boundaries".into(),json!({"multiplex":{"enabled":true,"concurrency":-1,"xudp_concurrency":128,"udp_443":"reject"}})),
        ("Mux UDP allow".into(),json!({"multiplex":{"enabled":true,"udp_443":"allow"}})),
        ("TUN and System Proxy".into(),json!({"tun":true,"system_proxy":true})),
    ]);
    let profile = imports::parse_uri("trojan://synthetic-test@127.0.0.1:443?security=tls&sni=example.invalid#Offline").unwrap();
    let database = directory.join("netpulse.db");
    let mut passed = Vec::new();
    for (name, patch) in &cases {
        let prefs = preferences(patch.clone());
        prefs.validate().unwrap_or_else(|e|panic!("{name}: {e:?}"));
        { let store=Store::open(&database).unwrap();store.save_preferences(&prefs).unwrap(); }
        let saved=Store::open(&database).unwrap().preferences().unwrap();
        assert_eq!(saved,prefs,"Persistence after reopen: {name}");
        let plan=RuntimePlan::new(&engine,&profile,&saved,24560,"matrix-api","matrix-proxy",[24561,24562]).unwrap_or_else(|e|panic!("{name}: {e:?}"));
        plan.validate(&engine).unwrap_or_else(|e|panic!("Bundled core rejected {name}: {e:?}"));
        passed.push(name.clone());
    }
    let invalid=[json!({"dns":{"mode":"udp","server":"not-an-ip"}}),json!({"dns_route":"vpn"}),json!({"mtu":1279}),json!({"mtu":9001}),json!({"proxy_port":1023}),json!({"probe_url":"http://example.com"}),json!({"probe_url":"https://user:pass@example.com"}),json!({"theme":"invalid"}),json!({"language":"invalid"}),json!({"transport_engine":"invalid"}),json!({"applications":["not-an-exe"]}),json!({"bypass_domains":["https://example.com/path"]}),json!({"fragmentation":{"length":"200-10"}}),json!({"multiplex":{"concurrency":129}}),json!({"tls_defaults":{"sni_mode":"custom","server_name":"https://invalid","http_host":null,"fingerprint":null}}),json!({"routing_rules":[{"enabled":true,"kind":"domain","value":"example.com","action":"invalid"}]})];
    for patch in &invalid { assert!(preferences(patch.clone()).validate().is_err(),"Accepted invalid setting: {patch}"); }
    for patch in [json!({"transport_engine":"sing-box","fragmentation":{"enabled":true}}),json!({"transport_engine":"xray","tls_defaults":{"sni_mode":"disabled","server_name":"example.invalid","http_host":null,"fingerprint":null}})] {
        let prefs=preferences(patch);assert!(RuntimePlan::new(&engine,&profile,&prefs,24560,"api","proxy",[24561,24562]).is_err());
    }
    let report=json!({"result":"passed","persisted_and_validated_cases":passed,"invalid_settings_rejected":invalid.len(),"incompatible_combinations_rejected":2,"cores_actually_executed_in_validation_mode":true,"windows_network_changed":false,"autostart_registry_changed":false});
    std::fs::write(directory.join("result.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    println!("{report}");
}
