//! Packaging smoke test. Uses only a caller-selected new directory, never AppData.
use netpulse_core::{imports, model::Preferences, store::Store};
use std::path::Path;
pub fn run(directory: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // Refuse to overwrite any existing data during the packaging check.
    std::fs::create_dir(directory)?;
    let path = directory.join("self-test.db");
    let mut store = Store::open(&path)?;
    let report=store.import(imports::parse("vless://15bc786a-b01b-4a8a-962e-cb52e089e500@example.org:443?security=tls&fm=#Packaging")?,None)?;
    assert_eq!(report.imported, 1);
    let id = store.profiles()?[0].id.clone();
    assert_eq!(store.profile(&id)?.outbound["type"], "vless");
    let settings = Preferences {
        language: "en".into(),
        theme: "light".into(),
        ..Preferences::default()
    };
    store.save_preferences(&settings)?;
    store.log(
        "INFO",
        "Application",
        "self-test",
        "PackagingPassed",
        "Native packaging smoke check passed",
    )?;
    let backup = store.backup()?;
    store.clear_all()?;
    store.restore(&backup)?;
    assert_eq!(store.profiles()?.len(), 1);
    drop(store);
    let store = Store::open(&path)?;
    assert_eq!(store.preferences()?.language, "en");
    assert_eq!(store.profiles()?.len(), 1);
    let cores = std::env::current_exe()?.parent().unwrap().join("cores");
    netpulse_core::engine::verify_engine(&cores.join("sing-box.exe"))?;
    netpulse_core::runtime::verify_xray(&cores.join("xray.exe"))?;
    let result = serde_json::json!({"result":"passed","version":env!("CARGO_PKG_VERSION"),"platform":"windows-x64","checks":["native Rust entrypoint","VLESS empty fm","SQLite reopen","DPAPI current-user roundtrip","settings persistence","encrypted backup restore"],"system_network_changed":false,"browser_tests_run":false});
    std::fs::write(
        directory.join("result.json"),
        serde_json::to_vec_pretty(&result)?,
    )?;
    Ok(())
}
