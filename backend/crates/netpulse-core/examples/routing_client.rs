//! Owned routing fixture. Never shipped with the application.
use std::{
    io::{Read, Write},
    net::TcpStream,
    time::Duration,
};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let mut socket = TcpStream::connect(format!("127.0.0.1:{}", args[1])).unwrap();
    println!(
        "source={} pid={}",
        socket.local_addr().unwrap(),
        std::process::id()
    );
    socket
        .set_read_timeout(Some(Duration::from_secs(4)))
        .unwrap();
    write!(socket,"GET http://127.0.0.1:{}/fixture HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",args[2],args[2]).unwrap();
    let mut response = String::new();
    let _ = socket.read_to_string(&mut response);
    println!("{}", response.lines().next().unwrap_or("NO RESPONSE"));
}
