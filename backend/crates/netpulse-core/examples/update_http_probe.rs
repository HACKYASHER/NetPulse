//! Read-only connection probe for the fixed official update endpoint.
use std::{net::{TcpStream,ToSocketAddrs},time::{Duration,Instant}};
fn main(){
    let started=Instant::now();
    match ("api.github.com",443).to_socket_addrs(){
        Ok(addresses)=>for address in addresses.filter(|a|a.is_ipv4()){
            let before=Instant::now();let result=TcpStream::connect_timeout(&address,Duration::from_secs(3));
            println!("TCP {address}: {:?} in {:?}",result.as_ref().map(|_|()).map_err(|e|e.to_string()),before.elapsed());
        },Err(e)=>println!("DNS: {e:?}")
    }
    println!("DNS/TCP total {:?}",started.elapsed());
    let before=Instant::now();
    let client=netpulse_core::network::client(Duration::from_secs(8)).unwrap();
    let result=client.get("https://api.github.com/repos/XTLS/Xray-core/releases?per_page=1").header("User-Agent","NetPulse-update-audit").send();
    println!("HTTPS: {:?} in {:?}",result.as_ref().map(|r|r.status()),before.elapsed());
    if let Err(e)=result{println!("Error: {e:?}");}
    for engine in ["xray","sing-box"]{
        let before=Instant::now();let result=netpulse_core::core_releases::list(engine);
        println!("Release metadata {engine}: {:?} in {:?}",result.as_ref().map(|r|r.len()),before.elapsed());
        if let Err(e)=result{println!("Metadata error: {e:?}");}
    }
}
