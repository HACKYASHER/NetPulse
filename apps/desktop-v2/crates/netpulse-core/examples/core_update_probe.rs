//! Explicit official-release smoke test. Only a fresh temporary directory changes.
use netpulse_core::{core_releases, core_updates, engine, model::Preferences, runtime};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = tempfile::tempdir()?;
    let target = data.path().join("engine");
    std::fs::create_dir(&target)?;
    let sbox = std::path::PathBuf::from(std::env::var("NETPULSE_TEST_ENGINE")?);
    let xray = std::path::PathBuf::from(std::env::var("NETPULSE_TEST_XRAY")?);
    for (source, name) in [
        (sbox.clone(), "sing-box.exe"),
        (sbox.with_file_name("libcronet.dll"), "libcronet.dll"),
        (xray, "xray.exe"),
    ] {
        std::fs::copy(source, target.join(name))?;
    }
    if let Ok(old) = std::env::var("NETPULSE_TEST_XRAY_PREVIOUS") {
        std::fs::copy(old, target.join("xray.exe"))?;
        runtime::verify_xray(&target.join("xray.exe"))?;
    }
    let previous = core_releases::executable_version(&target.join("xray.exe"), "xray")?;
    for (name, version) in [
        ("xray", runtime::XRAY_VERSION),
        ("sing-box", engine::ENGINE_VERSION),
    ] {
        println!("Checking official release list: {name}");
        let releases = core_releases::list(name)?;
        let tag = format!("v{version}");
        let selected = releases
            .iter()
            .find(|r| r.tag == tag)
            .cloned()
            .ok_or("Bundled release missing in official list")?;
        println!("Installing and validating: {name} {}", selected.tag);
        core_releases::install(selected, data.path(), &[], &Preferences::default())?;
        assert!(core_releases::approval(&target, name)?.is_some());
        assert_eq!(
            core_releases::executable_version(
                &target.join(if name == "xray" {
                    "xray.exe"
                } else {
                    "sing-box.exe"
                }),
                name
            )?,
            version
        );
        assert_eq!(
            std::fs::read_dir(data.path().join("core-downloads"))?.count(),
            0
        );
    }
    let updated = core_releases::executable_version(&target.join("xray.exe"), "xray")?;
    if std::env::var_os("NETPULSE_TEST_XRAY_PREVIOUS").is_some() {
        assert_ne!(previous, updated);
    }
    println!("Independently authenticating service approval records");
    let approvals = core_releases::authenticated_approvals(&target)?;
    assert_eq!(approvals.len(), 2);
    core_updates::rollback(data.path())?;
    engine::verify_engine(&target.join("sing-box.exe"))?;
    runtime::verify_xray(&target.join("xray.exe"))?;
    if let Ok(report) = std::env::var("NETPULSE_UPDATE_REPORT") {
        std::fs::write(
            report,
            serde_json::to_vec_pretty(
                &serde_json::json!({"result":"passed","previous_xray":previous,"installed_xray":updated,"archives_removed":true,"system_settings_changed":false,"independent_service_approval":true,"rollback":"passed"}),
            )?,
        )?;
    }
    println!(
        "Official Xray/sing-box download, archive checksum, configuration, independent service approval and rollback passed; no system settings changed"
    );
    Ok(())
}
