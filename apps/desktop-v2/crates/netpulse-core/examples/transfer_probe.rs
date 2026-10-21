//! Small, manual transfer comparison using the selected stored VLESS profile.
//! Does not change TUN, proxy settings or the selected server. No profile secrets are printed.
use netpulse_core::{
    runtime::{RuntimePlan, free_port},
    store::Store,
};
use std::{
    io::Read,
    time::{Duration, Instant},
};
fn main() -> netpulse_core::Result<()> {
    let directory = std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap())
        .join("local.netpulse.desktop.v2");
    let store = Store::open(&directory.join("netpulse.db"))?;
    let preferences = store.preferences()?;
    let Some(id) = preferences.selected_profile.as_deref() else {
        println!("No selected profile");
        return Ok(());
    };
    let profile = store.profile(id)?;
    if profile.outbound["type"] != "vless" || !profile.outbound["transport"].is_null() {
        println!("Select a TCP VLESS profile for this comparison");
        return Ok(());
    }
    let engine = netpulse_core::engine::installed_engine(&directory);
    let mut results = Vec::new();
    for name in ["sing-box", "xray", "xray", "sing-box"] {
        let mut prefs = preferences.clone();
        prefs.tun = false;
        prefs.system_proxy = false;
        prefs.route_mode = netpulse_core::model::RouteMode::Global;
        prefs.proxy_port = free_port()?;
        prefs.transport_engine = name.into();
        let secret = uuid::Uuid::new_v4().to_string();
        let plan = RuntimePlan::new(
            &engine,
            &profile,
            &prefs,
            free_port()?,
            "test-api",
            &secret,
            [free_port()?, free_port()?],
        )?;
        plan.validate(&engine)?;
        let mut runtime = plan.start(&engine)?;
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .proxy(
                reqwest::Proxy::all(format!("http://127.0.0.1:{}", plan.probe_port))
                    .unwrap()
                    .basic_auth("netpulse", &secret),
            )
            .timeout(Duration::from_secs(25))
            .build()
            .unwrap();
        let start = Instant::now();
        let download = (|| -> std::result::Result<(u64, f64), String> {
            let response = client
                .get("https://speed.cloudflare.com/__down?bytes=2097152")
                .send()
                .map_err(|_| "request failed")?;
            if !response.status().is_success() {
                return Err(format!("HTTP {}", response.status().as_u16()));
            }
            let first_byte = start.elapsed().as_secs_f64();
            let mut read = response.take(2097152);
            let bytes =
                std::io::copy(&mut read, &mut std::io::sink()).map_err(|_| "body failed")?;
            Ok((bytes, first_byte))
        })();
        let elapsed = start.elapsed().as_secs_f64();
        let start = Instant::now();
        let upload = client
            .post("https://speed.cloudflare.com/__up")
            .body(vec![0u8; 1048576])
            .send();
        let upload_elapsed = start.elapsed().as_secs_f64();
        let value = serde_json::json!({"engine":name,"download":download.as_ref().ok().map(|(bytes,first)|serde_json::json!({"bytes":bytes,"mbps":*bytes as f64*8./elapsed/1e6,"first_byte_seconds":first})),"download_error":download.err(),"upload_http":upload.as_ref().ok().map(|r|r.status().as_u16()),"upload_mbps":if upload.as_ref().is_ok_and(|r|r.status().is_success()){Some(1048576.*8./upload_elapsed/1e6)}else{None}});
        println!("{value}");
        results.push(value);
        runtime.stop()?;
    }
    if let Some(path) = std::env::args().nth(1) {
        std::fs::write(path, serde_json::to_vec_pretty(&results).unwrap())
            .map_err(|e| netpulse_core::Error::io("report", e))?;
    }
    Ok(())
}
