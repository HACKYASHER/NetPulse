//! Verify a public installer download in a temporary folder; never installs or changes VPN.
fn main() -> netpulse_core::Result<()> {
    let release = netpulse_core::client_updates::check("2.0.0-alpha.4")?
        .expect("Expected published NetPulse installer");
    let dir = tempfile::tempdir().expect("Temporary folder");
    netpulse_core::client_updates::download(dir.path(), &release)?;
    netpulse_core::client_updates::verify_download(dir.path(), &release)?;
    println!(
        "{}",
        serde_json::json!({"result":"passed","tag":release.tag,"size":release.size,"sha256":release.sha256,"installation":"not_run"})
    );
    Ok(())
}
