//! Real bundled Xray gRPC/TLS transfer, entirely on owned loopback endpoints.
#![cfg(windows)]
use netpulse_core::{
    engine::Engine,
    model::Preferences,
    runtime::{RuntimePlan, free_port},
};
use serde_json::json;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
#[test]
#[ignore = "Requires bundled engines and locally extracted owned TLS fixtures"]
fn grpc_gun_tls_transfers_bulk_upload_and_download_without_throttling() {
    let xray = std::path::PathBuf::from(std::env::var("NETPULSE_TEST_XRAY").unwrap());
    let engine = std::path::PathBuf::from(std::env::var("NETPULSE_TEST_ENGINE").unwrap());
    let cert = std::env::var("NETPULSE_GRPC_CERT").unwrap();
    let key = std::env::var("NETPULSE_GRPC_KEY").unwrap();
    let ca = include_str!("fixtures/owned-ca.pem")
        .lines()
        .collect::<Vec<_>>();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = listener.local_addr().unwrap().port();
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let halt = stop.clone();
    let server = std::thread::spawn(move || {
        while !halt.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream.set_nonblocking(false).unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(15)))
                        .unwrap();
                    let mut input = Vec::new();
                    let mut buf = [0u8; 16384];
                    let boundary = loop {
                        let n = stream.read(&mut buf).unwrap();
                        if n == 0 {
                            break None;
                        }
                        input.extend_from_slice(&buf[..n]);
                        if let Some(i) = input.windows(4).position(|x| x == b"\r\n\r\n") {
                            break Some(i + 4);
                        }
                    };
                    let Some(boundary) = boundary else {
                        continue;
                    };
                    let headers = String::from_utf8_lossy(&input[..boundary]);
                    eprintln!("origin request: {}", headers.lines().next().unwrap_or(""));
                    let upload = headers.starts_with("POST ");
                    let length = headers
                        .lines()
                        .find_map(|s| {
                            s.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .and_then(|s| s.parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    let mut received = input.len() - boundary;
                    while received < length {
                        let n = stream.read(&mut buf).unwrap();
                        if n == 0 {
                            break;
                        }
                        received += n;
                    }
                    if upload {
                        assert_eq!(received, 8 * 1024 * 1024);
                        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK").unwrap();
                    } else {
                        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 8388608\r\nConnection: close\r\n\r\n").unwrap();
                        for _ in 0..512 {
                            stream.write_all(&[7u8; 16384]).unwrap();
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5))
                }
                Err(e) => panic!("{e}"),
            }
        }
    });
    let port = free_port().unwrap();
    let id = "15bc786a-b01b-4a8a-962e-cb52e089e500";
    let config = json!({"log":{"loglevel":"debug"},"inbounds":[{"listen":"127.0.0.1","port":port,"protocol":"vless","settings":{"clients":[{"id":id}],"decryption":"none"},"streamSettings":{"network":"grpc","security":"tls","grpcSettings":{"serviceName":"owned_fixture"},"tlsSettings":{"alpn":["h2"],"certificates":[{"certificateFile":cert,"keyFile":key}]}}}],"outbounds":[{"protocol":"freedom","settings":{"finalRules":[{"ip":["127.0.0.1/32"],"port":origin.to_string(),"action":"allow"}]}}]});
    let mut grpc =
        Engine::spawn_arguments(&xray, &config, &["run", "-format", "json", "-c", "stdin:"])
            .unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    while std::net::TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(grpc.alive().unwrap(), "{}", grpc.diagnostic(&config));
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(25));
    }
    let profile=netpulse_core::imports::parse_uri(&format!("vless://{id}@127.0.0.1:{port}?security=tls&sni=localhost&fp=firefox&type=grpc&serviceName=owned_fixture&mode=gun")).unwrap();
    let prefs = Preferences {
        proxy_port: free_port().unwrap(),
        ..Default::default()
    };
    let secret = "owned-test-proxy";
    let mut plan = RuntimePlan::new(
        &engine,
        &profile,
        &prefs,
        free_port().unwrap(),
        "owned-test-api",
        secret,
        [free_port().unwrap(), free_port().unwrap()],
    )
    .unwrap();
    assert!(
        plan.bridge.is_some(),
        "Automatic mode must prefer Xray for gRPC gun"
    );
    plan.xray_path = xray;
    let bridge = plan.bridge.as_mut().unwrap();
    bridge["log"]["loglevel"] = json!("debug");
    bridge["outbounds"][0]["streamSettings"]["tlsSettings"]["certificates"] =
        json!([{"certificate":ca,"usage":"verify"}]);
    assert_eq!(
        bridge["outbounds"][0]["streamSettings"]["tlsSettings"]["alpn"],
        json!(["h2"])
    );
    plan.validate(&engine).unwrap();
    let mut runtime = plan.start(&engine).unwrap();
    let mut results = Vec::new();
    for proxy in [false, true] {
        let mut builder = reqwest::blocking::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(20));
        if proxy {
            builder = builder.proxy(
                reqwest::Proxy::all(format!("http://127.0.0.1:{}", plan.probe_port))
                    .unwrap()
                    .basic_auth("netpulse", secret),
            );
        }
        let client = builder.build().unwrap();
        let url = format!("http://127.0.0.1:{origin}/owned-bulk");
        let t = Instant::now();
        let response = client.get(&url).send().unwrap_or_else(|e| {
            panic!(
                "route proxy={proxy}: {e}; client: {}; server: {}",
                runtime.diagnostic(&plan),
                grpc.diagnostic(&config)
            )
        });
        assert!(response.status().is_success());
        let body = response.bytes().unwrap();
        assert_eq!(body.len(), 8 * 1024 * 1024);
        assert!(body.iter().all(|b| *b == 7));
        let download = t.elapsed().as_secs_f64();
        let t = Instant::now();
        let response = client
            .post(&url)
            .body(vec![42u8; 8 * 1024 * 1024])
            .send()
            .unwrap();
        assert_eq!(response.text().unwrap(), "OK");
        let upload = t.elapsed().as_secs_f64();
        results.push(json!({"route":if proxy {"sing-box-to-xray-grpc-tls"}else{"direct-loopback"},"download_bytes":8388608,"upload_bytes":8388608,"download_mbps":8388608.*8./download/1e6,"upload_mbps":8388608.*8./upload/1e6}));
    }
    if let Ok(path) = std::env::var("NETPULSE_TRANSFER_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&results).unwrap()).unwrap();
    }
    runtime.stop().unwrap();
    grpc.stop().unwrap();
    stop.store(true, Ordering::SeqCst);
    server.join().unwrap();
}
