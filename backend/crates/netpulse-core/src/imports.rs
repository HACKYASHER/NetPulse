use crate::{Error, Result, model::Profile};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub const MAX_IMPORT: usize = 8 * 1024 * 1024;
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ImportIssue {
    pub index: usize,
    pub code: String,
    pub message: String,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ImportReport {
    pub format: String,
    pub imported: usize,
    pub issues: Vec<ImportIssue>,
}
pub struct Parsed {
    pub profiles: Vec<Profile>,
    pub report: ImportReport,
    pub usage: Option<crate::store::SubscriptionUsage>,
}
fn error(s: impl Into<String>) -> Error {
    Error::new("ImportError", "profile-parse", s)
}
fn decode(s: &str) -> Result<Vec<u8>> {
    for engine in [&STANDARD, &STANDARD_NO_PAD, &URL_SAFE, &URL_SAFE_NO_PAD] {
        if let Ok(v) = engine.decode(s.trim()) {
            return Ok(v);
        }
    }
    Err(error("Invalid Base64 encoding"))
}
fn unescape(s: &str) -> Result<String> {
    percent_encoding::percent_decode_str(s)
        .decode_utf8()
        .map(|s| s.into_owned())
        .map_err(|_| error("Invalid UTF-8 in URI"))
}
fn ensure_keys(v: &Value, allowed: &[&str], where_: &str) -> Result<()> {
    let object = v
        .as_object()
        .ok_or_else(|| error(format!("{where_}: object required")))?;
    if object.keys().any(|k| !allowed.contains(&k.as_str())) {
        return Err(error(format!(
            "{where_}: unsupported fields; review format compatibility"
        )));
    }
    Ok(())
}
fn finalmask(q: &mut BTreeMap<String, String>, hy2: bool) -> Result<()> {
    let Some(raw) = q.remove("fm") else {
        return Ok(());
    };
    if raw.is_empty() {
        return Ok(());
    }
    if !hy2 {
        return Err(error(
            "fm: configured Finalmask is supported only for Hysteria2 Salamander",
        ));
    }
    if raw.len() > 16384 {
        return Err(error("fm: exceeds 16 KiB"));
    }
    let v: Value = crate::strict_json::parse(&raw)
        .map_err(|_| error("fm: expected a JSON Finalmask object without duplicate fields"))?;
    ensure_keys(&v, &["udp", "tcp", "quicParams"], "fm")?;
    if let Some(quic) = v.get("quicParams") {
        ensure_keys(quic, &["debug", "congestion"], "fm.quicParams")?;
        if quic.get("debug").is_some_and(|v| v != false) {
            return Err(error(
                "fm.quicParams.debug: only false is supported; network debugging may expose sensitive data",
            ));
        }
        if quic.get("congestion").is_some_and(|v| v != "bbr") {
            return Err(error(
                "fm.quicParams.congestion: sing-box 1.14.3 supports BBR for this import; other algorithms require a compatible runtime",
            ));
        }
        q.insert("netpulse-bbr".into(), "standard".into());
    }
    if let Some(tcp) = v.get("tcp")
        && !tcp.is_null()
        && tcp != &json!([])
    {
        return Err(error("fm: TCP masks are unsupported for Hysteria2"));
    }
    let Some(udp) = v.get("udp") else {
        return Ok(());
    };
    let masks = udp
        .as_array()
        .ok_or_else(|| error("fm: UDP masks must be an array"))?;
    if masks.len() > 1 {
        return Err(error("fm: only a single Salamander UDP mask is supported"));
    }
    if let Some(mask) = masks.first() {
        ensure_keys(mask, &["type", "settings"], "fm UDP")?;
        if mask["type"] != "salamander" {
            return Err(error(
                "fm: unsupported UDP mask; supported type is Salamander",
            ));
        }
        ensure_keys(&mask["settings"], &["password"], "fm Salamander")?;
        let password = mask["settings"]["password"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 1000)
            .ok_or_else(|| error("fm: Salamander password is required"))?;
        if q.get("obfs")
            .is_some_and(|v| !v.is_empty() && v != "salamander")
            || q.get("obfs-password")
                .is_some_and(|v| !v.is_empty() && v != password)
        {
            return Err(error("fm: conflicts with obfs parameters"));
        }
        q.insert("obfs".into(), "salamander".into());
        q.insert("obfs-password".into(), password.into());
    }
    Ok(())
}
pub fn parse_uri(raw: &str) -> Result<Profile> {
    if raw.len() > 65536 {
        return Err(error("URI exceeds 64 KiB"));
    }
    let u = url::Url::parse(raw).map_err(|_| error("Malformed connection URI"))?;
    let scheme = u.scheme();
    if ![
        "vless",
        "trojan",
        "hy2",
        "hysteria2",
        "ss",
        "socks",
        "socks5",
        "http",
        "https",
    ]
    .contains(&scheme)
    {
        return Err(error("Unsupported URI protocol"));
    }
    let mut q = BTreeMap::new();
    for (k, v) in u.query_pairs() {
        if q.insert(k.into_owned(), v.into_owned()).is_some() {
            return Err(error("Duplicate URI parameter"));
        }
    }
    finalmask(&mut q, ["hy2", "hysteria2"].contains(&scheme))?;
    let common = [
        "sni",
        "alpn",
        "fp",
        "type",
        "path",
        "serviceName",
        "host",
        "security",
        "mode",
    ];
    for key in q.keys() {
        let allowed = match scheme {
            "vless" => {
                common.contains(&key.as_str())
                    || ["pbk", "sid", "flow", "encryption"].contains(&key.as_str())
            }
            "trojan" => common.contains(&key.as_str()),
            "hy2" | "hysteria2" => [
                "sni",
                "alpn",
                "obfs",
                "obfs-password",
                "security",
                "netpulse-bbr",
            ]
            .contains(&key.as_str()),
            _ => false,
        };
        if !allowed {
            return Err(error("Unsupported URI parameter; its value was not logged"));
        }
    }
    let get = |k: &str, default: &str| {
        q.get(k)
            .filter(|v| !v.is_empty())
            .cloned()
            .unwrap_or_else(|| default.into())
    };
    let host = u
        .host_str()
        .ok_or_else(|| error("Server host is missing"))?
        .trim_matches(['[', ']']);
    let default_port = match scheme {
        "socks" | "socks5" => 1080,
        "http" => 8080,
        _ => 443,
    };
    let kind = match scheme {
        "hy2" => "hysteria2",
        "ss" => "shadowsocks",
        "socks5" => "socks",
        "https" => "http",
        _ => scheme,
    };
    let mut o = json!({"type":kind,"server":host,"server_port":u.port().unwrap_or(default_port)});
    let user = unescape(u.username())?;
    let password = unescape(u.password().unwrap_or(""))?;
    if !u.path().is_empty() && u.path() != "/" {
        return Err(error(
            "URI path is unsupported; use the transport path parameter",
        ));
    }
    match kind {
        "socks" | "http" => {
            if !user.is_empty() {
                o["username"] = json!(user);
                o["password"] = json!(password);
            }
        }
        "shadowsocks" => {
            let creds = if u.password().is_some() {
                format!("{user}:{password}")
            } else {
                String::from_utf8(decode(&user)?)
                    .map_err(|_| error("Shadowsocks credentials must be UTF-8"))?
            };
            let (method, pass) = creds
                .split_once(':')
                .ok_or_else(|| error("Shadowsocks cipher/password is missing"))?;
            o["method"] = json!(method);
            o["password"] = json!(pass);
        }
        "hysteria2" => {
            if q.contains_key("netpulse-bbr") {
                o["bbr_profile"] = json!("standard");
                o["brutal_debug"] = json!(false);
            }
            o["password"] = json!(if u.password().is_some() {
                format!("{user}:{password}")
            } else {
                user
            });
            let obfs = get("obfs", "");
            let pass = get("obfs-password", "");
            if !obfs.is_empty() || !pass.is_empty() {
                if obfs != "salamander" || pass.is_empty() {
                    return Err(error(
                        "Hysteria2 requires a complete Salamander obfuscation configuration",
                    ));
                }
                o["obfs"] = json!({"type":"salamander","password":pass});
            }
        }
        "vless" => {
            if u.password().is_some() || get("encryption", "none") != "none" {
                return Err(error("Unsupported VLESS encryption or userinfo"));
            }
            o["uuid"] = json!(user);
            let flow = get("flow", "");
            if !flow.is_empty() {
                o["flow"] = json!(flow);
            }
        }
        "trojan" => {
            if u.password().is_some() {
                return Err(error("Trojan password belongs in URI userinfo"));
            }
            o["password"] = json!(user);
        }
        _ => unreachable!(),
    }
    if ["vless", "trojan", "hysteria2"].contains(&kind) || scheme == "https" {
        let security = get("security", "tls");
        if !["tls", "reality"].contains(&security.as_str())
            || (security == "reality" && kind != "vless")
        {
            return Err(error("Unsupported TLS security mode"));
        }
        let mut tls = json!({"enabled":true});
        let sni = get("sni", "");
        if !sni.is_empty() {
            tls["server_name"] = json!(sni);
        }
        let alpn = get("alpn", "");
        if !alpn.is_empty() {
            tls["alpn"] = json!(alpn.split(',').collect::<Vec<_>>());
        }
        let fingerprint = get("fp", "");
        if !fingerprint.is_empty() || security == "reality" {
            tls["utls"] = json!({"enabled":true,"fingerprint":if fingerprint.is_empty(){"chrome"}else{&fingerprint}});
        }
        let pbk = get("pbk", "");
        if security == "reality" {
            tls["reality"] = json!({"enabled":true,"public_key":pbk,"short_id":get("sid","")});
        } else if !pbk.is_empty() || !get("sid", "").is_empty() {
            return Err(error("Reality keys require security=reality"));
        }
        o["tls"] = tls;
    }
    let transport = get("type", "tcp");
    match transport.as_str() {
        "tcp" => {
            if q.contains_key("path") || q.contains_key("serviceName") || q.contains_key("host") {
                return Err(error("TCP does not use transport path/host/serviceName"));
            }
        }
        "ws" => {
            if q.contains_key("serviceName") {
                return Err(error("WebSocket does not use serviceName"));
            }
            let mut t = json!({"type":"ws","path":get("path","/")});
            let host = get("host", "");
            if !host.is_empty() {
                t["headers"] = json!({"Host":host});
            }
            o["transport"] = t;
        }
        "grpc" => {
            if q.contains_key("path") || q.contains_key("host") {
                return Err(error("gRPC requires serviceName, not path/host"));
            }
            let mode = get("mode", "gun");
            if !["gun", "multi"].contains(&mode.as_str()) {
                return Err(error("gRPC mode must be gun or multi"));
            }
            o["transport"] = json!({"type":"grpc","service_name":get("serviceName","")});
            if mode == "multi" {
                o["transport"]["mode"] = json!(mode);
            }
        }
        "xhttp" => {
            if kind != "vless" {
                return Err(error("XHTTP currently requires VLESS and Xray-core"));
            }
            let mode = get("mode", "auto");
            if !["auto", "packet-up", "stream-up", "stream-one"].contains(&mode.as_str()) {
                return Err(error("Unsupported XHTTP mode"));
            }
            o["transport"] =
                json!({"type":"xhttp","path":get("path","/"),"host":get("host",""),"mode":mode});
        }
        _ => {
            return Err(error(
                "Unsupported transport; supported: TCP, WebSocket, gRPC, XHTTP (Xray-core)",
            ));
        }
    }
    let name = unescape(u.fragment().filter(|v| !v.is_empty()).unwrap_or(kind))?;
    if q.contains_key("mode") && !["grpc", "xhttp"].contains(&transport.as_str()) {
        return Err(error("mode is supported only for gRPC/XHTTP"));
    }
    let profile = Profile {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        outbound: o,
        subscription_id: None,
        transport_override: None,
        outbound_override: None,
    };
    profile.validate()?;
    Ok(profile)
}

pub(crate) fn validate_outbound_schema(v: &Value) -> Result<()> {
    ensure_keys(
        v,
        &[
            "type",
            "tag",
            "server",
            "server_port",
            "username",
            "password",
            "uuid",
            "flow",
            "method",
            "version",
            "tls",
            "transport",
            "obfs",
            "bbr_profile",
            "brutal_debug",
        ],
        "sing-box outbound",
    )?;
    if let Some(tls) = v.get("tls") {
        ensure_keys(
            tls,
            &[
                "enabled",
                "server_name",
                "disable_sni",
                "alpn",
                "insecure",
                "utls",
                "reality",
            ],
            "TLS",
        )?;
        if let Some(utls) = tls.get("utls") {
            ensure_keys(utls, &["enabled", "fingerprint"], "uTLS")?;
        }
        if let Some(r) = tls.get("reality") {
            ensure_keys(r, &["enabled", "public_key", "short_id"], "Reality")?;
        }
    }
    if let Some(t) = v.get("transport") {
        ensure_keys(
            t,
            &["type", "path", "headers", "service_name", "mode", "host"],
            "Transport",
        )?;
        if !["ws", "grpc", "xhttp"].contains(&t["type"].as_str().unwrap_or("")) {
            return Err(error("Unsupported transport"));
        }
        if let Some(h) = t.get("headers") {
            ensure_keys(h, &["Host"], "WebSocket headers")?;
        }
    }
    if let Some(o) = v.get("obfs") {
        ensure_keys(o, &["type", "password"], "Obfuscation")?;
        if o["type"] != "salamander" || v["type"] != "hysteria2" {
            return Err(error("Unsupported obfuscation"));
        }
    }
    if v.get("version").is_some_and(|x| x != "5") {
        return Err(error("Only SOCKS5 is supported"));
    }
    Ok(())
}
pub(crate) fn parse_outbound(v: &Value) -> Result<Profile> {
    validate_outbound_schema(v)?;
    let mut outbound = v.clone();
    outbound.as_object_mut().unwrap().remove("tag");
    let profile = Profile {
        id: uuid::Uuid::new_v4().to_string(),
        name: v["tag"].as_str().unwrap_or("Imported server").into(),
        outbound,
        subscription_id: None,
        transport_override: None,
        outbound_override: None,
    };
    profile.validate()?;
    Ok(profile)
}
fn parse_clash(v: &Value) -> Result<Profile> {
    ensure_keys(
        v,
        &[
            "name",
            "type",
            "server",
            "port",
            "username",
            "password",
            "uuid",
            "cipher",
            "tls",
            "servername",
            "sni",
            "alpn",
            "client-fingerprint",
            "flow",
            "network",
            "ws-opts",
            "grpc-opts",
            "reality-opts",
            "obfs",
            "obfs-password",
            "udp",
            "skip-cert-verify",
        ],
        "Clash profile",
    )?;
    if v["skip-cert-verify"] == true {
        return Err(error("TLS certificate verification must remain enabled"));
    }
    let kind = match v["type"].as_str().unwrap_or("") {
        "ss" => "shadowsocks",
        "socks5" => "socks",
        "hy2" => "hysteria2",
        v => v,
    };
    let mut o = json!({"type":kind,"server":v["server"],"server_port":v["port"]});
    for k in ["username", "password", "uuid", "flow"] {
        if let Some(value) = v.get(k) {
            o[k] = value.clone();
        }
    }
    if let Some(cipher) = v.get("cipher") {
        o["method"] = cipher.clone();
    }
    if v["tls"] == true || ["vless", "trojan", "hysteria2"].contains(&kind) {
        let mut tls = json!({"enabled":true});
        if let Some(sni) = v.get("sni").or_else(|| v.get("servername")) {
            tls["server_name"] = sni.clone();
        }
        if let Some(alpn) = v.get("alpn") {
            tls["alpn"] = alpn.clone();
        }
        if let Some(fp) = v.get("client-fingerprint") {
            tls["utls"] = json!({"enabled":true,"fingerprint":fp});
        }
        if let Some(r) = v.get("reality-opts") {
            ensure_keys(r, &["public-key", "short-id"], "Clash Reality")?;
            tls["reality"] = json!({"enabled":true,"public_key":r["public-key"],"short_id":r.get("short-id").unwrap_or(&json!(""))});
            if tls.get("utls").is_none() {
                tls["utls"] = json!({"enabled":true,"fingerprint":"chrome"});
            }
        }
        o["tls"] = tls;
    }
    match v["network"].as_str().unwrap_or("tcp") {
        "tcp" => {
            if v.get("ws-opts").is_some() || v.get("grpc-opts").is_some() {
                return Err(error("Transport options do not match TCP"));
            }
        }
        "ws" => {
            let opts = v.get("ws-opts").cloned().unwrap_or(json!({}));
            ensure_keys(&opts, &["path", "headers"], "Clash WebSocket")?;
            o["transport"] =
                json!({"type":"ws","path":opts.get("path").cloned().unwrap_or(json!("/"))});
            if let Some(h) = opts.get("headers") {
                ensure_keys(h, &["Host"], "WebSocket headers")?;
                o["transport"]["headers"] = h.clone();
            }
        }
        "grpc" => {
            let opts = v.get("grpc-opts").cloned().unwrap_or(json!({}));
            ensure_keys(&opts, &["grpc-service-name"], "Clash gRPC")?;
            o["transport"] = json!({"type":"grpc","service_name":opts.get("grpc-service-name").cloned().unwrap_or(json!(""))});
        }
        _ => return Err(error("Unsupported Clash transport")),
    }
    if v.get("obfs").is_some() || v.get("obfs-password").is_some() {
        if v["obfs"] != "salamander"
            || kind != "hysteria2"
            || v["obfs-password"].as_str().unwrap_or("").is_empty()
        {
            return Err(error(
                "Only complete Hysteria2 Salamander obfuscation is supported",
            ));
        }
        o["obfs"] = json!({"type":"salamander","password":v["obfs-password"]});
    }
    let p = Profile {
        id: uuid::Uuid::new_v4().to_string(),
        name: v["name"].as_str().unwrap_or("Imported server").into(),
        outbound: o,
        subscription_id: None,
        transport_override: None,
        outbound_override: None,
    };
    p.validate()?;
    Ok(p)
}
pub fn parse(raw: &str) -> Result<Parsed> {
    if raw.len() > MAX_IMPORT {
        return Err(error("Import exceeds 8 MiB"));
    }
    let mut text = raw.trim().trim_start_matches('\u{feff}').to_string();
    if text.is_empty() {
        return Err(error("Import is empty"));
    }
    let mut format = "uri";
    let yaml = text.starts_with("proxies:") || text.starts_with("---");
    if !yaml && !text.contains("://") && !text.starts_with(['{', '[']) {
        text = String::from_utf8(decode(&text.split_whitespace().collect::<String>())?)
            .map_err(|_| error("Decoded subscription must be UTF-8"))?;
        format = "base64";
    }
    let mut profiles = vec![];
    let mut issues = vec![];
    if yaml || text.trim_start().starts_with(['{', '[']) {
        let v: Value = if yaml {
            if text.lines().any(|s| {
                s.contains(": &")
                    || s.contains(": *")
                    || s.contains("- &")
                    || s.contains("- *")
                    || s.contains("<<:")
                    || s.contains("!!")
                    || s.starts_with("%")
            }) {
                return Err(error(
                    "YAML aliases, anchors, merge keys and tags are unsupported",
                ));
            }
            let options=serde_saphyr::options!{budget:serde_saphyr::budget!{
                max_depth:32,max_documents:1,max_events:200_000,max_nodes:100_000,
                max_total_scalar_bytes:8*1024*1024,max_aliases:0,max_anchors:0,max_merge_keys:0,
            }};
            serde_saphyr::from_str_with_options(&text,options).map_err(|_| error("Malformed YAML subscription or parsing limit exceeded"))?
        } else {
            crate::strict_json::parse(&text)
                .map_err(|_| error("Malformed JSON subscription or duplicate fields"))?
        };
        let clash = v.get("proxies").is_some();
        let nodes = if let Some(a) = v.as_array() {
            a.clone()
        } else if v.get("outbounds").is_some() {
            ensure_keys(&v, &["outbounds"], "Subscription")?;
            v["outbounds"]
                .as_array()
                .ok_or_else(|| error("outbounds must be an array"))?
                .clone()
        } else if clash {
            ensure_keys(&v, &["proxies"], "Clash subscription")?;
            v["proxies"]
                .as_array()
                .ok_or_else(|| error("proxies must be an array"))?
                .clone()
        } else {
            vec![v]
        };
        if nodes.len() > 2000 {
            return Err(error("Maximum 2000 profiles per import"));
        }
        for (i, v) in nodes.iter().enumerate() {
            match if clash {
                parse_clash(v)
            } else {
                parse_outbound(v)
            } {
                Ok(p) => profiles.push(p),
                Err(e) => issues.push(ImportIssue {
                    index: i + 1,
                    code: e.code,
                    message: e.message,
                }),
            }
        }
        format = if clash {
            if yaml { "clash_yaml" } else { "clash_json" }
        } else {
            "sing_box_json"
        };
    } else {
        let lines: Vec<_> = text
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        if lines.len() > 2000 {
            return Err(error("Maximum 2000 profiles per import"));
        }
        for (i, line) in lines.iter().enumerate() {
            match parse_uri(line) {
                Ok(p) => profiles.push(p),
                Err(e) => issues.push(ImportIssue {
                    index: i + 1,
                    code: e.code,
                    message: e.message,
                }),
            }
        }
    }
    if profiles.is_empty() {
        return Err(error(
            issues
                .first()
                .map(|i| format!("Profile {}: {}", i.index, i.message))
                .unwrap_or_else(|| "No compatible profiles found".into()),
        ));
    }
    let imported = profiles.len();
    Ok(Parsed {
        usage: None,
        profiles,
        report: ImportReport {
            format: format.into(),
            imported,
            issues,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const VLESS: &str =
        "vless://15bc786a-b01b-4a8a-962e-cb52e089e500@example.org:443?security=tls&fm=#Test";
    #[test]
    fn empty_fm_across_protocols() {
        for uri in [
            VLESS,
            "hy2://pass@example.org:443?fm=",
            "trojan://pass@example.org:443?fm=",
            "ss://YWVzLTEyOC1nY206cGFzcw@example.org:443?fm=",
            "socks5://user:pass@example.org:1080?fm=",
            "https://user:pass@example.org:443?fm=",
            "http://user:pass@example.org:8080?fm=",
        ] {
            assert!(parse_uri(uri).is_ok(), "{uri}");
        }
    }
    #[test]
    fn provider_quic_bbr_and_reality_transports() {
        let fm = percent_encoding::utf8_percent_encode(
            r#"{"quicParams":{"debug":false,"congestion":"bbr"}}"#,
            percent_encoding::NON_ALPHANUMERIC,
        )
        .to_string();
        let p = parse_uri(&format!(
            "hysteria2://fixture@example.org:443/?sni=example.org&fm={fm}#HY2"
        ))
        .unwrap();
        assert_eq!(p.outbound["bbr_profile"], "standard");
        assert_eq!(p.outbound["brutal_debug"], false);
        assert!(p.outbound.get("up_mbps").is_none());
        let reality = "encryption=none&security=reality&sni=example.org&fp=firefox&pbk=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA&sid=0123456789abcdef";
        let base = "vless://15bc786a-b01b-4a8a-962e-cb52e089e500@example.org:443";
        let grpc = parse_uri(&format!(
            "{base}?{reality}&type=grpc&serviceName=fixture_grpc&mode=gun"
        ))
        .unwrap();
        assert_eq!(grpc.outbound["transport"]["service_name"], "fixture_grpc");
        assert!(!grpc.requires_xray());
        let xhttp = parse_uri(&format!("{base}?{reality}&type=xhttp&path=%2F&mode=auto")).unwrap();
        assert_eq!(xhttp.outbound["transport"]["path"], "/");
        assert_eq!(xhttp.outbound["transport"]["mode"], "auto");
        assert!(xhttp.requires_xray());
        assert!(parse_uri(&format!("{base}?{reality}&type=xhttp&mode=unknown")).is_err());
        let bad_fm = fm.replace("bbr", "unknown");
        assert!(parse_uri(&format!("hy2://fixture@example.org:443?fm={bad_fm}")).is_err());
    }
    #[test]
    fn partial_and_no_secret_errors() {
        let p = parse(&format!(
            "{VLESS}\nhy2://SENSITIVE@example.org:443?fm=OPAQUE_SECRET"
        ))
        .unwrap();
        assert_eq!(p.report.imported, 1);
        assert_eq!(p.report.issues[0].index, 2);
        let s = serde_json::to_string(&p.report).unwrap();
        assert!(!s.contains("SENSITIVE"));
        assert!(!s.contains("OPAQUE_SECRET"));
    }
    #[test]
    fn duplicate_rejected() {
        assert!(parse_uri(&format!("{VLESS}&sni=a&sni=b")).is_ok());
        assert!(parse_uri("hy2://pass@example.org:443?sni=a&sni=b").is_err());
    }
    #[test]
    fn salamander_mapping() {
        let fm = percent_encoding::utf8_percent_encode(
            r#"{"udp":[{"type":"salamander","settings":{"password":"secret"}}]}"#,
            percent_encoding::NON_ALPHANUMERIC,
        )
        .to_string();
        let p = parse_uri(&format!("hy2://pass@example.org:443?fm={fm}")).unwrap();
        assert_eq!(p.outbound["obfs"]["password"], "secret");
    }
    #[test]
    fn rejects_insecure_and_script() {
        assert!(parse_uri(&VLESS.replace("security=tls", "allowInsecure=1")).is_err());
        assert!(parse(r#"{"outbounds":[],"route":{"final":"direct"}}"#).is_err());
    }
    #[test]
    fn base64_and_json() {
        let p = parse(&STANDARD.encode(VLESS)).unwrap();
        assert_eq!(p.report.format, "base64");
        assert_eq!(
            parse(r#"[{"type":"socks","server":"127.0.0.1","server_port":1080}]"#)
                .unwrap()
                .profiles
                .len(),
            1
        );
    }
    #[test]
    fn clash_yaml_and_duplicate_json() {
        let p=parse("proxies:\n  - name: Owned\n    type: trojan\n    server: example.org\n    port: 443\n    password: fixture\n").unwrap();
        assert_eq!(p.report.format, "clash_yaml");
        assert_eq!(p.profiles[0].outbound["type"], "trojan");
        assert!(
            parse(
                r#"{"type":"socks","server":"example.org","server":"evil.org","server_port":1080}"#
            )
            .is_err()
        );
        assert!(parse("proxies:\n  - &a {name: test, type: socks5}\n  - *a").is_err());
    }
    #[test]
    fn yaml_resource_limits_reject_deep_structures_and_alias_expansion() {
        let deep=format!("proxies: {}0{}","[".repeat(1000),"]".repeat(1000));
        assert!(parse(&deep).is_err());
        for yaml in ["---\nproxies: [&a {type: socks5, name: x, server: localhost, port: 1080}, *a]", "---\nproxies: !include file.yaml", "---\nproxies: []\n---\nproxies: []"]{assert!(parse(yaml).is_err(),"unsupported YAML was accepted");}
        let valid="proxies:\n  - name: Saved server\n    type: socks5\n    server: 127.0.0.1\n    port: 1080\n";
        assert_eq!(parse(valid).unwrap().profiles.len(),1);
    }
}
