//! Read stored profiles and test isolated outbound connections; never applies TUN or WinINET.
use std::sync::atomic::AtomicBool;
fn main() -> netpulse_core::Result<()> {
    let dir = std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA").expect("LOCALAPPDATA"))
        .join("local.netpulse.desktop.v2");
    if !dir.join("netpulse.db").exists() {
        println!("No stored profiles");
        return Ok(());
    }
    let store = netpulse_core::store::Store::open(&dir.join("netpulse.db"))?;
    let engine = netpulse_core::engine::installed_engine(&dir);
    let preferences = store.preferences()?;
    for p in store
        .profiles()?
        .iter()
        .filter(|p| p.protocol == "vless")
        .take(12)
    {
        let profile = store.profile(&p.id)?;
        if profile.outbound["transport"]["type"] != "grpc" {
            continue;
        }
        for transport in ["sing-box", "auto"] {
            let mut prefs = preferences.clone();
            prefs.transport_engine = transport.into();
            let result = netpulse_core::diagnostics::latency(
                profile.clone(),
                prefs,
                engine.clone(),
                "proxy_https_rtt",
                &AtomicBool::new(false),
            );
            println!(
                "{}",
                serde_json::json!({"transport":"grpc","engine":transport,"successes":result.successes,"timeouts":result.timeouts,"avg_ms":result.avg_ms,"error_code":result.error.map(|e|e.code)})
            );
        }
    }
    Ok(())
}
