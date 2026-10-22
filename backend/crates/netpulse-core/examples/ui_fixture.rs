//! Creates synthetic, loopback-only UI data in a NEW directory. Never connects.
use netpulse_core::{store::Store,imports,model::Preferences,inventory};
fn main() {
 let dir=std::path::PathBuf::from(std::env::args_os().nth(1).expect("New directory required"));
 assert!(dir.is_absolute()&&!dir.exists(),"Fixture directory must be new");
 std::fs::create_dir_all(&dir).unwrap();
 let mut store=Store::open(&dir.join("netpulse.db")).unwrap();
 let sub=store.add_subscription("Example subscription","https://example.invalid/subscription").unwrap();
 let names=["🇩🇪 Germany GRPC","🇩🇪 Germany VLESS","🇫🇮 Finland VLESS","🇫🇮 Finland GRPC","🇺🇸 United States","🇳🇱 Netherlands"];
 let mut uris=Vec::new();
 for(i,name)in names.iter().enumerate(){
  let uri=format!("vless://85d01a47-8957-4f0a-afb2-c7896d30792b@127.0.0.1:{}?security=tls&sni=example.invalid#{}",44000+i,url::form_urlencoded::byte_serialize(name.as_bytes()).collect::<String>().replace('+',"%20"));
  uris.push(uri);
 }
 store.import(imports::parse(&uris.join("\n")).unwrap(),Some(&sub)).unwrap();
 let profiles=store.profiles().unwrap();
 let prefs=Preferences{selected_profile:Some(profiles[2].id.clone()),favorites:vec![profiles[0].id.clone()],applications:vec!["netpulse.exe".into(),"steam.exe".into()],bypass_domains:vec!["ru".into(),"su".into()],..Preferences::default()};
 store.save_preferences(&prefs).unwrap();
 inventory::write_catalog(&dir,vec![inventory::Application{name:"NetPulse".into(),path:std::env::current_dir().unwrap().parent().unwrap().join("release/NetPulse/NetPulse.exe").to_string_lossy().into_owned(),source:"fixture".into()},inventory::Application{name:"Steam".into(),path:r"C:\Program Files (x86)\Steam\steam.exe".into(),source:"fixture".into()}]).unwrap();
 store.add_measurement("latency",&serde_json::json!({"profile_id":profiles[2].id,"method":"proxy_https_rtt","avg_ms":42.5,"timestamp":netpulse_core::store::now(),"successes":2,"failures":0,"timeouts":0,"note":"Synthetic UI fixture"})).unwrap();
 store.add_measurement("mtu",&serde_json::json!({"suggested_mtu":1400,"estimate":true,"target":"1.1.1.1","timestamp":netpulse_core::store::now()})).unwrap();
 store.log("ERROR","Application","fixture","ExampleError","Пример ошибки для проверки кодировки").unwrap();
 println!("{}",dir.display());
}
