//! Owned-loopback crash fixture; never included in the application installer.
use netpulse_core::engine::{self, Engine};
use serde_json::json;
use std::{
    io::{self, Write},
    path::PathBuf,
};
fn main() {
    let path =
        PathBuf::from(std::env::var_os("NETPULSE_TEST_ENGINE").expect("Pinned engine required"));
    engine::verify_engine(&path).unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let config = json!({"log":{"disabled":true},"inbounds":[{"type":"mixed","listen":"127.0.0.1","listen_port":port}],"outbounds":[{"type":"direct"}]});
    let engine = Engine::spawn(&path, &config, false).unwrap();
    println!("{}", json!({"pid":engine.pid(),"port":port}));
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    drop(engine);
}
