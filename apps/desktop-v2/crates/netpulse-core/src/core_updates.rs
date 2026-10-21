//! Only runtimes verified for this client may be activated. Unknown releases
//! remain informational until a client compatibility catalog approves them.
use crate::{Error, Result, engine, runtime};
use serde::Serialize;
use std::{path::Path, time::Duration};
#[derive(Serialize)]
pub struct CoreVersion {
    pub name: String,
    pub installed: String,
    pub latest: Option<String>,
    pub compatible: bool,
    pub error: Option<Error>,
}
pub fn check() -> Vec<CoreVersion> {
    [
        ("sing-box", engine::ENGINE_VERSION, "SagerNet/sing-box"),
        ("Xray", runtime::XRAY_VERSION, "XTLS/Xray-core"),
    ]
    .into_iter()
    .map(|(name, installed, repo)| {
        let result = (|| {
            let response = crate::network::client(Duration::from_secs(10))?
                .get(format!(
                    "https://api.github.com/repos/{repo}/releases/latest"
                ))
                .header("User-Agent", "NetPulse/2.0")
                .send()
                .map_err(|_| {
                    Error::new(
                        "CoreUpdateUnavailable",
                        "core-update-check",
                        "Could not check official releases",
                    )
                })?;
            if !response.status().is_success() {
                return Err(Error::new(
                    "CoreUpdateUnavailable",
                    "core-update-check",
                    "Official release server rejected the request",
                ));
            }
            use std::io::Read;
            let mut bytes = vec![];
            response
                .take(1024 * 1024)
                .read_to_end(&mut bytes)
                .map_err(|e| Error::io("core-update-check", e))?;
            let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| {
                Error::new(
                    "CoreUpdateInvalid",
                    "core-update-check",
                    "Invalid official release metadata",
                )
            })?;
            value["tag_name"]
                .as_str()
                .map(|s| s.trim_start_matches('v').to_string())
                .ok_or_else(|| {
                    Error::new(
                        "CoreUpdateInvalid",
                        "core-update-check",
                        "Missing release version",
                    )
                })
        })();
        match result {
            Ok(latest) => CoreVersion {
                name: name.into(),
                installed: installed.into(),
                compatible: latest == installed,
                latest: Some(latest),
                error: None,
            },
            Err(error) => CoreVersion {
                name: name.into(),
                installed: installed.into(),
                latest: None,
                compatible: false,
                error: Some(error),
            },
        }
    })
    .collect()
}
pub fn ensure_bundled(resources: &Path, data: &Path) -> Result<()> {
    let target = data.join("engine");
    if engine::verify_engine(&target.join("sing-box.exe")).is_ok()
        && runtime::verify_xray(&target.join("xray.exe")).is_ok()
        && (crate::core_releases::approval(&target, "xray")?.is_some()
            || crate::core_releases::hash(&target.join("xray.exe"))? == runtime::XRAY_SHA256)
    {
        return Ok(());
    }
    activate_bundled(resources, data)
}
pub fn activate_bundled(resources: &Path, data: &Path) -> Result<()> {
    let source = resources.join("cores");
    engine::verify_engine(&source.join("sing-box.exe"))?;
    runtime::verify_xray(&source.join("xray.exe"))?;
    let staged = data.join(format!("core-stage-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&staged).map_err(|e| Error::io("core-stage", e))?;
    let result = (|| {
        for name in ["sing-box.exe", "libcronet.dll", "xray.exe"] {
            std::fs::copy(source.join(name), staged.join(name))
                .map_err(|e| Error::io("core-stage", e))?;
        }
        engine::verify_engine(&staged.join("sing-box.exe"))?;
        runtime::verify_xray(&staged.join("xray.exe"))?;
        let profile = crate::imports::parse_uri("socks5://127.0.0.1:1080#RuntimeCheck")?;
        let config = engine::configuration(
            &profile,
            &crate::model::Preferences::default(),
            crate::runtime::free_port()?,
            "catalog-check",
            "catalog-check",
        )?;
        engine::Engine::validate(&staged.join("sing-box.exe"), &config)?;
        activate_staged(&staged, data)?;
        Ok(())
    })();
    if staged.exists() {
        for name in ["sing-box.exe", "libcronet.dll", "xray.exe"] {
            let _ = std::fs::remove_file(staged.join(name));
        }
        let _ = std::fs::remove_dir(staged);
    }
    result
}
pub fn activate_staged(staged: &Path, data: &Path) -> Result<()> {
    let target = data.join("engine");
    let backup = data.join(format!("core-previous-{}", uuid::Uuid::new_v4()));
    let had_previous = target.exists();
    if had_previous {
        move_directory(&target, &backup).map_err(|e| Error::io("core-backup", e))?;
    }
    if let Err(e) = move_directory(staged, &target) {
        if had_previous {
            let _ = move_directory(&backup, &target);
        }
        return Err(Error::io("core-activate", e));
    }
    if had_previous && let Err(error) = save_pointer(data, &backup) {
        move_directory(&target, staged).map_err(|e| Error::io("core-activate-rollback", e))?;
        move_directory(&backup, &target).map_err(|e| Error::io("core-activate-rollback", e))?;
        return Err(error);
    }
    Ok(())
}
fn move_directory(from: &Path, to: &Path) -> std::io::Result<()> {
    // Windows scanners can briefly retain executable handles after `check` exits.
    // Retry sharing/access races; never remove a destination or terminate a process.
    let until = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        match std::fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(error)
                if matches!(error.raw_os_error(), Some(5 | 32 | 33))
                    && std::time::Instant::now() < until =>
            {
                std::thread::sleep(Duration::from_millis(100))
            }
            Err(error) => return Err(error),
        }
    }
}
pub fn rollback(data: &Path) -> Result<()> {
    let raw = std::fs::read(data.join("core-rollback.json"))
        .map_err(|e| Error::io("core-rollback", e))?;
    let name: String = serde_json::from_slice(&raw).map_err(|_| {
        Error::new(
            "CoreRollbackInvalid",
            "core-rollback",
            "Invalid previous runtime record",
        )
    })?;
    if !name.starts_with("core-previous-") || name.contains(['/', '\\']) {
        return Err(Error::new(
            "CoreRollbackInvalid",
            "core-rollback",
            "Invalid previous runtime directory",
        ));
    }
    let previous = data.join(name);
    engine::verify_engine(&previous.join("sing-box.exe"))?;
    runtime::verify_xray(&previous.join("xray.exe"))?;
    let target = data.join("engine");
    let current = data.join(format!("core-previous-{}", uuid::Uuid::new_v4()));
    move_directory(&target, &current).map_err(|e| Error::io("core-rollback", e))?;
    if let Err(e) = move_directory(&previous, &target) {
        let _ = move_directory(&current, &target);
        return Err(Error::io("core-rollback", e));
    }
    if let Err(error) = save_pointer(data, &current) {
        move_directory(&target, &previous).map_err(|e| Error::io("core-rollback-recover", e))?;
        move_directory(&current, &target).map_err(|e| Error::io("core-rollback-recover", e))?;
        return Err(error);
    }
    Ok(())
}
fn save_pointer(data: &Path, directory: &Path) -> Result<()> {
    use std::io::Write;
    let staged = data.join(format!("core-pointer-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&staged)
            .map_err(|e| Error::io("core-rollback-save", e))?;
        file.write_all(
            &serde_json::to_vec(&directory.file_name().unwrap().to_string_lossy()).unwrap(),
        )
        .map_err(|e| Error::io("core-rollback-save", e))?;
        file.sync_all()
            .map_err(|e| Error::io("core-rollback-save", e))?;
        drop(file);
        #[cfg(windows)]
        {
            use windows_sys::Win32::Storage::FileSystem::{
                MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
            };
            let from = crate::service::wide(&staged.to_string_lossy());
            let to = crate::service::wide(&data.join("core-rollback.json").to_string_lossy());
            if unsafe {
                MoveFileExW(
                    from.as_ptr(),
                    to.as_ptr(),
                    MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
                )
            } == 0
            {
                return Err(Error::io(
                    "core-rollback-save",
                    std::io::Error::last_os_error(),
                ));
            }
        }
        #[cfg(not(windows))]
        std::fs::rename(&staged, data.join("core-rollback.json"))
            .map_err(|e| Error::io("core-rollback-save", e))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(staged);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_runtime_does_not_prevent_local_core_settings_or_subscription_access() {
        let data = tempfile::tempdir().unwrap();
        let resources = tempfile::tempdir().unwrap();
        let store = crate::store::Store::open(&data.path().join("state.db")).unwrap();
        store
            .add_subscription("Fixture", "https://example.org/fixture")
            .unwrap();
        let error = ensure_bundled(resources.path(), data.path()).unwrap_err();
        assert_eq!(error.code, "RuntimeComponentUnavailable");
        let mut core =
            crate::network::NetworkCore::new(store, engine::installed_engine(data.path())).unwrap();
        assert_eq!(core.store.subscriptions().unwrap().len(), 1);
        assert!(core.snapshot().engine_pid.is_none());
        assert!(
            core.store
                .save_preferences(&crate::model::Preferences::default())
                .is_ok()
        );
        let profile = crate::imports::parse("socks5://127.0.0.1:1080#Fixture").unwrap();
        core.store.import(profile, None).unwrap();
        assert!(core.connect(&core.store.profiles().unwrap()[0].id).is_err());
        assert!(core.disconnect().is_ok());
        assert_eq!(core.store.subscriptions().unwrap().len(), 1);
    }
    #[test]
    fn failed_activation_preserves_existing_files_and_rejects_traversal() {
        let data = tempfile::tempdir().unwrap();
        let resources = tempfile::tempdir().unwrap();
        std::fs::create_dir(data.path().join("engine")).unwrap();
        std::fs::write(data.path().join("engine/marker"), b"previous").unwrap();
        assert!(activate_bundled(resources.path(), data.path()).is_err());
        assert_eq!(
            std::fs::read(data.path().join("engine/marker")).unwrap(),
            b"previous"
        );
        std::fs::write(
            data.path().join("core-rollback.json"),
            br#""core-previous-../escape""#,
        )
        .unwrap();
        assert_eq!(
            rollback(data.path()).unwrap_err().code,
            "CoreRollbackInvalid"
        );
    }
    #[test]
    #[ignore = "Requires pinned runtime fixtures; offline installation only"]
    fn offline_bundle_repair_rollback_and_tamper_detection() {
        let resources = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let cores = resources.path().join("cores");
        std::fs::create_dir(&cores).unwrap();
        let sbox = std::path::PathBuf::from(std::env::var("NETPULSE_TEST_ENGINE").unwrap());
        let xray = std::path::PathBuf::from(std::env::var("NETPULSE_TEST_XRAY").unwrap());
        for (from, name) in [
            (sbox.clone(), "sing-box.exe"),
            (sbox.with_file_name("libcronet.dll"), "libcronet.dll"),
            (xray, "xray.exe"),
        ] {
            std::fs::copy(from, cores.join(name)).unwrap();
        }
        ensure_bundled(resources.path(), data.path()).unwrap();
        std::fs::write(data.path().join("engine/marker"), b"old").unwrap();
        activate_bundled(resources.path(), data.path()).unwrap();
        assert!(!data.path().join("engine/marker").exists());
        rollback(data.path()).unwrap();
        assert!(data.path().join("engine/marker").exists());
        std::fs::write(cores.join("xray.exe"), b"tampered").unwrap();
        assert!(activate_bundled(resources.path(), data.path()).is_err());
        assert!(data.path().join("engine/marker").exists());
    }
}
