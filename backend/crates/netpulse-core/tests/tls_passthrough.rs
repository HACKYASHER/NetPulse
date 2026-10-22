//! Real end-to-end TLS through bundled cores. No system routes or trust-store changes.
#![cfg(windows)]
use netpulse_core::{
    engine::Engine,
    model::Preferences,
    runtime::{RuntimePlan, free_port},
};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

#[derive(Debug)]
struct Recorded {
    socket: TcpStream,
    bytes: Arc<Mutex<Vec<u8>>>,
    writes: bool,
    fragment: bool,
}
impl Read for Recorded {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        let n = self.socket.read(b)?;
        if !self.writes {
            self.bytes.lock().unwrap().extend_from_slice(&b[..n]);
        }
        Ok(n)
    }
}
impl Write for Recorded {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        let n = if self.fragment && !b.is_empty() {
            self.fragment = false;
            let first = b.len().min(7);
            self.socket.write_all(&b[..first])?;
            // Exceed the optional TLS sniff timeout with an incomplete record.
            std::thread::sleep(Duration::from_millis(350));
            for chunk in b[first..].chunks(11) {
                self.socket.write_all(chunk)?;
            }
            b.len()
        } else {
            self.socket.write(b)?
        };
        if self.writes {
            self.bytes.lock().unwrap().extend_from_slice(&b[..n]);
        }
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.socket.flush()
    }
}
fn first_record(bytes: &[u8]) -> &[u8] {
    assert!(
        bytes.len() >= 5 && bytes[0] == 22,
        "Missing TLS handshake record"
    );
    let length = 5 + u16::from_be_bytes([bytes[3], bytes[4]]) as usize;
    assert!(bytes.len() >= length);
    &bytes[..length]
}
fn tunnel(port: u16, origin: u16, socks: bool) -> TcpStream {
    let mut socket = TcpStream::connect(("127.0.0.1", port)).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(8)))
        .unwrap();
    socket
        .set_write_timeout(Some(Duration::from_secs(8)))
        .unwrap();
    if socks {
        socket.write_all(&[5, 1, 0]).unwrap();
        let mut reply = [0; 2];
        socket.read_exact(&mut reply).unwrap();
        assert_eq!(reply, [5, 0]);
        let mut request = vec![5, 1, 0, 1, 127, 0, 0, 1];
        request.extend(origin.to_be_bytes());
        socket.write_all(&request).unwrap();
        let mut reply = [0; 10];
        socket.read_exact(&mut reply).unwrap();
        assert_eq!(&reply[..2], &[5, 0]);
    } else {
        socket
            .write_all(
                format!("CONNECT 127.0.0.1:{origin} HTTP/1.1\r\nHost: 127.0.0.1:{origin}\r\n\r\n")
                    .as_bytes(),
            )
            .unwrap();
        let mut reply = Vec::new();
        while !reply.ends_with(b"\r\n\r\n") {
            let mut b = [0];
            socket.read_exact(&mut b).unwrap();
            reply.push(b[0]);
            assert!(reply.len() < 8192);
        }
        assert!(String::from_utf8_lossy(&reply).starts_with("HTTP/1.1 200"));
    }
    socket
}

#[test]
#[ignore = "Requires bundled cores; owned loopback TLS, never changes Windows proxy, TUN or trusted certificates"]
fn connect_and_socks_preserve_client_hello_sni_and_peer_certificate() {
    let identity = native_tls::Identity::from_pkcs12(
        include_bytes!("fixtures/owned-localhost.pfx"),
        "fixture-only",
    )
    .unwrap();
    let acceptor = native_tls::TlsAcceptor::new(identity).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = listener.local_addr().unwrap().port();
    listener.set_nonblocking(true).unwrap();
    let (records, received) = mpsc::channel();
    let halt = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop = halt.clone();
    let server = std::thread::spawn(move || {
        while !stop.load(std::sync::atomic::Ordering::SeqCst) {
            let Ok((socket, _)) = listener.accept() else {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            };
            socket.set_nonblocking(false).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(8)))
                .unwrap();
            let bytes = Arc::new(Mutex::new(Vec::new()));
            let stream = Recorded {
                socket,
                bytes: bytes.clone(),
                writes: false,
                fragment: false,
            };
            if let Ok(mut tls) = acceptor.accept(stream) {
                let mut request = Vec::new();
                let mut block = [0; 1024];
                while !request.ends_with(b"\r\n\r\n") && request.len() < 8192 {
                    match tls.read(&mut block) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => request.extend_from_slice(&block[..n]),
                    }
                }
                if request.ends_with(b"\r\n\r\n") {
                    let _=tls.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    let _ = tls.shutdown();
                }
            }
            let _ = records.send(bytes.lock().unwrap().clone());
        }
    });
    let root = native_tls::Certificate::from_pem(include_bytes!("fixtures/owned-ca.pem")).unwrap();
    let connector = native_tls::TlsConnector::builder()
        .add_root_certificate(root)
        .min_protocol_version(Some(native_tls::Protocol::Tlsv12))
        .build()
        .unwrap();
    let engine = std::path::PathBuf::from(std::env::var_os("NETPULSE_TEST_ENGINE").unwrap());
    let xray = std::path::PathBuf::from(std::env::var_os("NETPULSE_TEST_XRAY").unwrap());
    let remote_port = free_port().unwrap();
    let tls_proxy_port = free_port().unwrap();
    let mut remote_config = serde_json::json!({"log":{"loglevel":"warning"},"inbounds":[{"listen":"127.0.0.1","port":remote_port,"protocol":"socks","settings":{"auth":"noauth","udp":true}}],"outbounds":[{"protocol":"freedom","settings":{"finalRules":[{"ip":["127.0.0.1/32"],"port":origin.to_string(),"action":"allow"}]}}]});
    remote_config["inbounds"].as_array_mut().unwrap().push(serde_json::json!({"listen":"127.0.0.1","port":tls_proxy_port,"protocol":"vless","settings":{"decryption":"none","clients":[{"id":"15bc786a-b01b-4a8a-962e-cb52e089e500"}]},"streamSettings":{"network":"tcp","security":"tls","tlsSettings":{"certificates":[{"certificate":include_str!("fixtures/owned-localhost-cert.pem").lines().collect::<Vec<_>>(),"key":include_str!("fixtures/owned-localhost-key.pem").lines().collect::<Vec<_>>()}]}}}));
    let mut remote = Engine::spawn_arguments(
        &xray,
        &remote_config,
        &["run", "-format", "json", "-c", "stdin:"],
    )
    .unwrap();
    for _ in 0..100 {
        if TcpStream::connect(("127.0.0.1", remote_port)).is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let profile =
        netpulse_core::imports::parse_uri(&format!("socks5://127.0.0.1:{remote_port}")).unwrap();
    let mut pinned = None;
    for (transport, socks, fragment, wrong_name) in [
        ("direct", false, false, false),
        ("auto", false, false, false),
        ("auto", true, true, false),
        ("xray", false, true, false),
        ("xray", true, false, false),
        ("vless-tls", false, false, false),
        ("vless-fragment", false, false, false),
        ("vless-mux", false, false, false),
        ("vless-fragment-mux", true, false, false),
        ("auto", false, false, true),
    ] {
        let mut runtime = None;
        let socket = if transport == "direct" {
            TcpStream::connect(("127.0.0.1", origin)).unwrap()
        } else {
            let mut prefs = Preferences {
                proxy_port: free_port().unwrap(),
                system_proxy: true,
                tun: true,
                transport_engine: if transport.starts_with("vless-") {
                    "xray".into()
                } else {
                    transport.into()
                },
                bypass_domains: vec!["unrelated.invalid".into()],
                ..Preferences::default()
            };
            prefs.fragmentation.enabled = transport.contains("fragment");
            prefs.multiplex.enabled = transport.contains("mux");
            let outer_profile=netpulse_core::imports::parse_uri(&format!("vless://15bc786a-b01b-4a8a-962e-cb52e089e500@127.0.0.1:{tls_proxy_port}?security=tls&sni=localhost#OwnedTLS")).unwrap();
            let mut plan = RuntimePlan::new(
                &engine,
                if transport.starts_with("vless-") {
                    &outer_profile
                } else {
                    &profile
                },
                &prefs,
                free_port().unwrap(),
                "fixture-api",
                "fixture-proxy",
                [free_port().unwrap(), free_port().unwrap()],
            )
            .unwrap();
            // Test the mixed-listener side of a combined-mode plan without applying TUN.
            plan.config["inbounds"]
                .as_array_mut()
                .unwrap()
                .retain(|v| v["type"] != "tun");
            if transport.starts_with("vless-") {
                plan.bridge.as_mut().unwrap()["outbounds"][0]["streamSettings"]["tlsSettings"]["certificates"] = serde_json::json!([{"usage":"verify","certificate":include_str!("fixtures/owned-ca.pem").lines().collect::<Vec<_>>()}]);
            }
            plan.xray_path = xray.clone();
            plan.validate(&engine).unwrap();
            runtime = Some(plan.start(&engine).unwrap());
            tunnel(prefs.proxy_port, origin, socks)
        };
        socket
            .set_read_timeout(Some(Duration::from_secs(8)))
            .unwrap();
        let written = Arc::new(Mutex::new(Vec::new()));
        let wrapped = Recorded {
            socket,
            bytes: written.clone(),
            writes: true,
            fragment,
        };
        let result = connector.connect(
            if wrong_name {
                "wrong-name.invalid"
            } else {
                "localhost"
            },
            wrapped,
        );
        if wrong_name {
            assert!(result.is_err(), "Hostname verification must remain enabled");
        } else {
            let mut tls = result.unwrap();
            let peer = tls.peer_certificate().unwrap().unwrap().to_der().unwrap();
            if let Some(expected) = &pinned {
                assert_eq!(&peer, expected, "Proxy changed the original certificate");
            } else {
                pinned = Some(peer);
            }
            tls.write_all(b"GET /fixture HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .unwrap();
            let mut reply = Vec::new();
            let _ = tls.read_to_end(&mut reply);
            assert!(
                reply.starts_with(b"HTTP/1.1 204"),
                "{transport}, socks={socks}, fragmented={fragment}"
            );
        }
        let read = received.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(
            first_record(&written.lock().unwrap()),
            first_record(&read),
            "ClientHello/SNI changed in transit"
        );
        drop(runtime);
    }
    halt.store(true, std::sync::atomic::Ordering::SeqCst);
    server.join().unwrap();
    remote.stop().unwrap();
}
