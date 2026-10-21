//! One routing/TUN engine; an authenticated loopback Xray bridge only when required.
use crate::{
    Error, Result,
    engine::{self, Engine},
    model::{Preferences, Profile},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
pub const XRAY_VERSION: &str = "26.10.17";
pub const XRAY_SHA256: &str = "a743b8c2ce49cb33d6c1cf764c780bf1a475d746ad7485e7c3fcfe6458511c92";
pub const XRAY_ARCHIVE_SHA256: &str =
    "c10a532a9271964f2aa2f4b4c0757605ff529222cb1d5d7ab88f8ecc52e67fb0";
pub fn free_port() -> Result<u16> {
    Ok(std::net::TcpListener::bind("127.0.0.1:0")
        .map_err(|e| Error::io("runtime-port", e))?
        .local_addr()
        .map_err(|e| Error::io("runtime-port", e))?
        .port())
}
pub fn verify_xray(path: &Path) -> Result<()> {
    if crate::core_releases::verify_custom(path, "xray")? {
        return Ok(());
    }
    // Previously bundled official release remains eligible for explicit rollback.
    if crate::core_releases::hash(path)
        .is_ok_and(|h| h == "15c2d007954ac53ba69b80ec91242786b3c0b71d52649165b4ca1d5cc96ef8f1")
    {
        return Ok(());
    }

    let mut file =
        std::fs::File::open(path).map_err(|e| crate::engine::component_access_error("Xray", e))?;
    let mut digest = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| Error::io("xray-verify", e))?;
        if n == 0 {
            break;
        }
        digest.update(&buf[..n]);
    }
    if digest
        .finalize()
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect::<String>()
        != XRAY_SHA256
    {
        return Err(Error::new(
            "EngineHashMismatch",
            "xray-verify",
            "Xray executable differs from the pinned official release",
        ));
    }
    Ok(())
}
pub struct RuntimePlan {
    pub config: Value,
    pub bridge: Option<Value>,
    pub probe_port: u16,
    pub direct_probe_port: u16,
    bridge_port: u16,
    pub xray_path: PathBuf,
}
impl Drop for RuntimePlan {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        fn wipe(value: &mut Value) {
            match value {
                Value::String(s) => s.zeroize(),
                Value::Object(m) => m.values_mut().for_each(wipe),
                Value::Array(a) => a.iter_mut().for_each(wipe),
                _ => (),
            }
        }
        wipe(&mut self.config);
        if let Some(bridge) = &mut self.bridge {
            wipe(bridge);
        }
    }
}
impl RuntimePlan {
    pub fn new(
        engine_path: &Path,
        profile: &Profile,
        prefs: &Preferences,
        api_port: u16,
        api_secret: &str,
        proxy_secret: &str,
        probe_ports: [u16; 2],
    ) -> Result<Self> {
        let [probe_port, direct_probe_port] = probe_ports;
        profile.validate()?;
        prefs.validate()?;
        let resolved = profile.resolved_with(prefs)?;
        let profile = &resolved;
        let mut routed = profile.clone();
        let mut bridge = None;
        let mut bridge_port = 0;
        let use_xray = crate::transport_options::use_xray(profile, prefs)?;
        if use_xray {
            bridge_port = free_port()?;
            let password = uuid::Uuid::new_v4().to_string();
            let mut xray = xray_config(profile, bridge_port, &password)?;
            crate::transport_options::apply_xray(&mut xray, prefs);
            bridge = Some(xray);
            routed.outbound = json!({"type":"socks","server":"127.0.0.1","server_port":bridge_port,"username":"netpulse","password":password,"version":"5"});
        }
        let mut config = engine::configuration(&routed, prefs, api_port, api_secret, proxy_secret)?;
        #[cfg(windows)]
        {
            let host = profile.outbound["server"].as_str().unwrap_or("");
            let loopback = host.eq_ignore_ascii_case("localhost")
                || host
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback());
            if !loopback && let Some(interface) = crate::routes::outbound_interface() {
                if let Some(xray) = &mut bridge {
                    let index = if prefs.fragmentation.enabled { 1 } else { 0 };
                    let outbound = &mut xray["outbounds"][index];
                    if outbound["streamSettings"]["sockopt"].is_null() {
                        outbound["streamSettings"]["sockopt"] = json!({});
                    }
                    outbound["streamSettings"]["sockopt"]["interface"] = json!(interface);
                } else {
                    config["outbounds"][0]["bind_interface"] = json!(interface);
                }
                // The local SOCKS bridge is deliberately left on loopback.
                config["outbounds"][1]["bind_interface"] = json!(interface);
                if let Ok(view) = crate::routes::inspect()
                    && !crate::routes::interface_has_ipv6_default(&view, &interface)
                {
                    // Prefer IPv4 for direct domains on this physical interface.
                    // Keep IPv6-only answers available; literal IPv6 needs a usable route.
                    config["outbounds"][1]["domain_resolver"] =
                        json!({"server":"bootstrap","strategy":"prefer_ipv4"});
                }
            }
        }
        // Diagnostics always traverse the selected outbound, even if this GUI is
        // excluded by a process whitelist. Normal traffic retains its actual rules.
        config["inbounds"].as_array_mut().unwrap().push(json!({"type":"mixed","tag":"diagnostic-proxy","listen":"127.0.0.1","listen_port":probe_port,"users":[{"username":"netpulse","password":proxy_secret}]}));
        config["route"]["rules"].as_array_mut().unwrap().insert(
            0,
            json!({"inbound":"diagnostic-proxy","action":"route","outbound":"proxy"}),
        );
        config["inbounds"].as_array_mut().unwrap().push(json!({"type":"mixed","tag":"diagnostic-direct","listen":"127.0.0.1","listen_port":direct_probe_port,"users":[{"username":"netpulse","password":proxy_secret}]}));
        config["route"]["rules"].as_array_mut().unwrap().insert(
            0,
            json!({"inbound":"diagnostic-direct","action":"route","outbound":"direct"}),
        );
        let xray_path = engine_path.with_file_name("xray.exe");
        if use_xray && prefs.effective_tun() {
            // Xray's transport sockets must leave through the physical NIC. If
            // captured by our TUN, route only this pinned bridge process direct.
            config["route"]["rules"].as_array_mut().unwrap().insert(0,json!({"process_path_regex":crate::app_rules::process_patterns(&[xray_path.to_string_lossy().into_owned()]),"action":"route","outbound":"direct"}));
        }
        Ok(Self {
            config,
            bridge,
            probe_port,
            direct_probe_port,
            bridge_port,
            xray_path,
        })
    }
    pub fn validate(&self, engine_path: &Path) -> Result<()> {
        engine::verify_engine(engine_path)?;
        if let Some(c) = &self.bridge {
            verify_xray(&self.xray_path)?;
            let mut e = Engine::spawn_arguments(
                &self.xray_path,
                c,
                &["run", "-test", "-format", "json", "-c", "stdin:"],
            )?;
            e.wait_checked(c)?;
        }
        Engine::validate(engine_path, &self.config)
    }
    pub fn start(&self, engine_path: &Path) -> Result<EngineGroup> {
        let bridge = if let Some(c) = &self.bridge {
            let mut p = Engine::spawn_arguments(
                &self.xray_path,
                c,
                &["run", "-format", "json", "-c", "stdin:"],
            )?;
            wait_endpoint(&mut p, self.bridge_port, c)?;
            Some(p)
        } else {
            None
        };
        let mut primary = Engine::spawn(engine_path, &self.config, false)?;
        wait_endpoint(&mut primary, self.probe_port, &self.config)?;
        Ok(EngineGroup { primary, bridge })
    }
}
fn wait_endpoint(engine: &mut Engine, port: u16, config: &Value) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if !engine.alive()? {
            let _ = engine.stop();
            let detail = engine.diagnostic(config);
            return Err(Error::new(
                "EngineExited",
                "runtime-ready",
                if detail.is_empty() {
                    "Runtime exited before opening its authenticated loopback endpoint".into()
                } else {
                    format!(
                        "Runtime exited before opening its authenticated loopback endpoint: {detail}"
                    )
                },
            ));
        }
        if std::net::TcpStream::connect_timeout(
            &format!("127.0.0.1:{port}").parse().unwrap(),
            Duration::from_millis(100),
        )
        .is_ok()
        {
            return Ok(());
        }
        if Instant::now() > deadline {
            return Err(Error::new(
                "EngineTimeout",
                "runtime-ready",
                "Runtime endpoint did not become available within 8 seconds",
            ));
        }
        std::thread::sleep(Duration::from_millis(30));
    }
}
pub struct EngineGroup {
    primary: Engine,
    bridge: Option<Engine>,
}
impl EngineGroup {
    pub fn recent_diagnostic(&self) -> String {
        let mut output = self.primary.diagnostic(&Value::Null);
        if let Some(b) = &self.bridge {
            output.push_str("\nXray: ");
            output.push_str(&b.diagnostic(&Value::Null));
        }
        output
    }
    pub fn pid(&self) -> u32 {
        self.primary.pid()
    }
    pub fn alive(&mut self) -> Result<bool> {
        Ok(self.primary.alive()?
            && match &mut self.bridge {
                Some(b) => b.alive()?,
                None => true,
            })
    }
    pub fn stop(&mut self) -> Result<()> {
        let result = self.primary.stop();
        if let Some(b) = &mut self.bridge {
            b.stop()?;
        }
        result
    }
    pub fn diagnostic(&self, plan: &RuntimePlan) -> String {
        let primary = self.primary.diagnostic(&plan.config);
        let bridge = self
            .bridge
            .as_ref()
            .zip(plan.bridge.as_ref())
            .map(|(engine, config)| engine.diagnostic(config))
            .unwrap_or_default();
        if bridge.is_empty() {
            primary
        } else {
            format!(
                "sing-box:\n{}Xray:\n{}",
                crate::runtime_log::complete_tail(&primary, 2000),
                crate::runtime_log::complete_tail(&bridge, 2000)
            )
        }
    }
}
impl Drop for EngineGroup {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
pub fn xray_config(profile: &Profile, port: u16, password: &str) -> Result<Value> {
    profile.validate()?;
    let profile = profile.resolved()?;
    let o = &profile.outbound;
    let t = &o["transport"];
    let tls = &o["tls"];
    let transport = t["type"].as_str().unwrap_or("tcp");
    let network = match transport {
        "httpupgrade" => "httpupgrade",
        "ws" => "ws",
        "grpc" => "grpc",
        "xhttp" => "xhttp",
        _ => "tcp",
    };
    let security = if tls["reality"]["enabled"] == true {
        "reality"
    } else if tls["enabled"] == true {
        "tls"
    } else {
        "none"
    };
    let mut stream = json!({"network":network,"security":security});
    match network {
        "xhttp" => {
            stream["xhttpSettings"] =
                json!({"path":t["path"],"mode":t["mode"],"host":t["host"].as_str().unwrap_or("")})
        }
        "grpc" => {
            stream["grpcSettings"] = json!({"serviceName":t["service_name"].as_str().unwrap_or(""),"multiMode":t["mode"]=="multi"})
        }
        "ws" => {
            stream["wsSettings"] = json!({"path":t["path"].as_str().unwrap_or("/"),"headers":t["headers"].as_object().cloned().unwrap_or_default()})
        }
        "httpupgrade" => {
            stream["httpupgradeSettings"] = json!({"path":t["path"].as_str().unwrap_or("/"),"host":t["host"].as_str().unwrap_or("")})
        }
        _ => (),
    }
    let sni = tls["server_name"].as_str().unwrap_or("");
    let fp = if tls["utls"]["enabled"] == false {
        ""
    } else {
        tls["utls"]["fingerprint"].as_str().unwrap_or("chrome")
    };
    if stream["security"] == "reality" {
        stream["realitySettings"] = json!({"serverName":sni,"fingerprint":fp,"publicKey":tls["reality"]["public_key"],"shortId":tls["reality"]["short_id"]});
    } else if security == "tls" {
        let mut settings = json!({"serverName":sni,"allowInsecure":false,"fingerprint":fp});
        if let Some(alpn) = tls.get("alpn") {
            settings["alpn"] = alpn.clone();
        }
        if network == "grpc" {
            settings["alpn"] = json!(["h2"]);
        }
        stream["tlsSettings"] = settings;
    }
    let (protocol, settings) = match o["type"].as_str().unwrap_or("") {
        "vless" => {
            let mut user = json!({"id":o["uuid"],"encryption":"none"});
            if let Some(flow) = o.get("flow") {
                user["flow"] = flow.clone();
            }
            (
                "vless",
                json!({"vnext":[{"address":o["server"],"port":o["server_port"],"users":[user]}]}),
            )
        }
        "trojan" => (
            "trojan",
            json!({"servers":[{"address":o["server"],"port":o["server_port"],"password":o["password"]}]}),
        ),
        "shadowsocks" => (
            "shadowsocks",
            json!({"servers":[{"address":o["server"],"port":o["server_port"],"method":o["method"],"password":o["password"]}]}),
        ),
        "socks" | "http" => {
            let mut server = json!({"address":o["server"],"port":o["server_port"]});
            if o["username"].as_str().is_some_and(|v| !v.is_empty()) {
                server["users"] = json!([{"user":o["username"],"pass":o["password"]}]);
            }
            (
                if o["type"] == "socks" {
                    "socks"
                } else {
                    "http"
                },
                json!({"servers":[server]}),
            )
        }
        _ => {
            return Err(Error::new(
                "UnsupportedRuntime",
                "xray-config",
                "Xray does not support this protocol",
            ));
        }
    };
    Ok(
        json!({"log":{"loglevel":"warning"},"inbounds":[{"listen":"127.0.0.1","port":port,"protocol":"socks","settings":{"auth":"password","accounts":[{"user":"netpulse","pass":password}],"udp":true}}],"outbounds":[{"protocol":protocol,"settings":settings,"streamSettings":stream}]}),
    )
}
pub fn install_xray(data_dir: &Path) -> Result<()> {
    let response = crate::network::client(Duration::from_secs(120))?
        .get("https://github.com/XTLS/Xray-core/releases/download/v26.10.17/Xray-windows-64.zip")
        .send()
        .map_err(|_| {
            Error::new(
                "EngineDownloadFailed",
                "xray-download",
                "Official Xray download failed",
            )
        })?;
    if !response.status().is_success() {
        return Err(Error::new(
            "EngineDownloadFailed",
            "xray-download",
            format!(
                "Official download returned HTTP {}",
                response.status().as_u16()
            ),
        ));
    }
    let mut bytes = Vec::new();
    response
        .take(100 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| Error::io("xray-download", e))?;
    if bytes.len() > 100 * 1024 * 1024
        || Sha256::digest(&bytes)
            .iter()
            .map(|v| format!("{v:02x}"))
            .collect::<String>()
            != XRAY_ARCHIVE_SHA256
    {
        return Err(Error::new(
            "EngineHashMismatch",
            "xray-download",
            "Xray archive differs from the pinned official release",
        ));
    }
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| {
        Error::new(
            "EngineArchiveInvalid",
            "xray-extract",
            "Invalid Xray archive",
        )
    })?;
    let dest = data_dir.join("engine");
    std::fs::create_dir_all(&dest).map_err(|e| Error::io("xray-directory", e))?;
    let temporary = dest.join(format!("xray-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let file = archive.by_name("xray.exe").map_err(|_| {
            Error::new(
                "EngineArchiveInvalid",
                "xray-extract",
                "xray.exe missing from archive",
            )
        })?;
        let mut out =
            std::fs::File::create(&temporary).map_err(|e| Error::io("xray-extract", e))?;
        std::io::copy(&mut file.take(100 * 1024 * 1024), &mut out)
            .map_err(|e| Error::io("xray-extract", e))?;
        drop(out);
        verify_xray(&temporary)?;
        std::fs::rename(&temporary, dest.join("xray.exe")).map_err(|e| Error::io("xray-install", e))
    })(); // Only this UUID file belongs to this operation.
    let _ = std::fs::remove_file(temporary);
    result
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    #[ignore = "Pinned engine; owned IPv6 loopback and synthetic hosts DNS only"]
    fn prefer_ipv4_preserves_ipv6_only_names_without_using_external_dns() {
        use std::{
            io::{Read, Write},
            net::{TcpListener, TcpStream},
        };
        let origin = TcpListener::bind("[::1]:0").unwrap();
        let target = origin.local_addr().unwrap().port();
        origin.set_nonblocking(true).unwrap();
        let responder = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                match origin.accept() {
                    Ok((mut socket, _)) => {
                        socket
                            .set_read_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        let mut byte = [0];
                        socket.read_exact(&mut byte).unwrap();
                        socket.write_all(&byte).unwrap();
                        return;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() < deadline,
                            "owned IPv6 origin was never reached"
                        );
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) => panic!("owned IPv6 origin: {e}"),
                }
            }
        });
        let path = std::path::PathBuf::from(std::env::var_os("NETPULSE_TEST_ENGINE").unwrap());
        for strategy in ["prefer_ipv4", "ipv4_only"] {
            let port = free_port().unwrap();
            let config = json!({
                "log":{"level":"error"},
                "dns":{"servers":[{"type":"hosts","tag":"fixture","predefined":{"v6.fixture.invalid":["::1"]}}]},
                "inbounds":[{"type":"mixed","listen":"127.0.0.1","listen_port":port}],
                "outbounds":[{"type":"direct","tag":"direct","domain_resolver":{"server":"fixture","strategy":strategy}}],
                "route":{"final":"direct"}
            });
            Engine::validate(&path, &config).unwrap();
            let mut engine = Engine::spawn(&path, &config, false).unwrap();
            wait_endpoint(&mut engine, port, &config).unwrap();
            let mut socket = TcpStream::connect(("127.0.0.1", port)).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            socket.write_all(&[5, 1, 0]).unwrap();
            let mut greeting = [0; 2];
            socket.read_exact(&mut greeting).unwrap();
            assert_eq!(greeting, [5, 0]);
            let name = b"v6.fixture.invalid";
            let mut request = vec![5, 1, 0, 3, name.len() as u8];
            request.extend(name);
            request.extend(target.to_be_bytes());
            socket.write_all(&request).unwrap();
            let mut reply = [0; 4];
            socket.read_exact(&mut reply).unwrap();
            if strategy == "prefer_ipv4" {
                assert_eq!(reply[1], 0, "{}", engine.diagnostic(&config));
                let mut bound = vec![0; if reply[3] == 4 { 18 } else { 6 }];
                socket.read_exact(&mut bound).unwrap();
                socket.write_all(b"v").unwrap();
                let mut echoed = [0];
                socket.read_exact(&mut echoed).unwrap();
                assert_eq!(&echoed, b"v");
            } else {
                assert_ne!(
                    reply[1], 0,
                    "IPv4-only resolver must demonstrate the original empty-result regression"
                );
            }
            engine.stop().unwrap();
        }
        responder.join().unwrap();
    }
    #[test]
    #[ignore = "Requires the pinned official Xray; validates only synthetic configuration"]
    fn actual_xray_accepts_reality_xhttp_and_multi_grpc() {
        let path = std::path::PathBuf::from(
            std::env::var_os("NETPULSE_TEST_XRAY").expect("Set NETPULSE_TEST_XRAY"),
        );
        verify_xray(&path).unwrap();
        for transport in [
            "type=xhttp&path=%2F&mode=auto",
            "type=grpc&serviceName=fixture&mode=multi",
        ] {
            let p=crate::imports::parse_uri(&format!("vless://15bc786a-b01b-4a8a-962e-cb52e089e500@example.org:443?encryption=none&security=reality&sni=example.org&fp=firefox&pbk=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA&sid=0123456789abcdef&{transport}")).unwrap();
            let c = xray_config(&p, free_port().unwrap(), "synthetic-fixture").unwrap();
            let mut e = Engine::spawn_arguments(
                &path,
                &c,
                &["run", "-test", "-format", "json", "-c", "stdin:"],
            )
            .unwrap();
            e.wait_checked(&c).unwrap();
        }
    }
    #[test]
    #[ignore = "Requires the bundled Xray runtime; configuration checks only"]
    fn preferred_xray_accepts_supported_transports_without_applying_tun() {
        let xray = std::path::PathBuf::from(std::env::var("NETPULSE_TEST_XRAY").unwrap());
        for uri in [
            "vless://15bc786a-b01b-4a8a-962e-cb52e089e500@example.org:443?security=tls#TCP",
            "vless://15bc786a-b01b-4a8a-962e-cb52e089e500@example.org:443?security=tls&type=ws&path=%2F#WS",
            "trojan://fixture@example.org:443#Trojan",
            "ss://aes-128-gcm:fixture@example.org:443#SS",
            "socks5://127.0.0.1:1080#Socks",
            "http://127.0.0.1:1080#HTTP",
        ] {
            let p = crate::imports::parse_uri(uri).unwrap();
            let config = xray_config(&p, free_port().unwrap(), "fixture").unwrap();
            let mut child = Engine::spawn_arguments(
                &xray,
                &config,
                &["run", "-test", "-format", "json", "-c", "stdin:"],
            )
            .unwrap();
            child.wait_checked(&config).unwrap();
        }
    }
}
