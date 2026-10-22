use crate::{Error, Result};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Application {
    pub name: String,
    pub path: String,
    pub source: String,
}
pub fn normalized_catalog(apps: Vec<Application>) -> Result<Vec<Application>> {
    if apps.len() > 5000 {
        return Err(Error::new(
            "CatalogLimit",
            "applications",
            "Catalog exceeds 5000 entries",
        ));
    }
    let mut found = std::collections::BTreeMap::new();
    for mut app in apps {
        app.path = app.path.trim().trim_matches('"').replace('/', "\\");
        if let Some(path) = app.path.strip_prefix(r"\\?\UNC\") {
            app.path = format!(r"\\{path}");
        } else if let Some(path) = app.path.strip_prefix(r"\\?\") {
            app.path = path.into();
        }
        let absolute = app.path.starts_with(r"\\")
            || (app.path.as_bytes().get(1) == Some(&b':')
                && app.path.as_bytes().get(2) == Some(&b'\\'));
        if !absolute
            || !app.path.to_ascii_lowercase().ends_with(".exe")
            || app.path.len() > 4096
            || app.path.chars().any(char::is_control)
            || app.path.split('\\').any(|s| s == "." || s == "..")
            || app.name.len() > 512
            || app.source.len() > 128
        {
            return Err(Error::new(
                "CatalogInvalid",
                "applications",
                "Invalid application catalog entry",
            ));
        }
        found.insert(app.path.to_lowercase(), app);
    }
    Ok(found.into_values().collect())
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    version: u8,
    applications: Vec<Application>,
}
pub fn read_catalog(directory: &std::path::Path) -> Result<Option<Vec<Application>>> {
    let path = directory.join("applications.json");
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Error::io("applications-read", e)),
    };
    if bytes.len() > 2_000_000 {
        return Err(Error::new(
            "CatalogLimit",
            "applications",
            "Catalog exceeds 2 MB",
        ));
    }
    let catalog: Catalog = serde_json::from_slice(&bytes).map_err(|_| {
        Error::new(
            "CatalogInvalid",
            "applications",
            "Application catalog is unreadable; file preserved",
        )
    })?;
    if catalog.version != 1 {
        return Err(Error::new(
            "CatalogVersion",
            "applications",
            "Unknown catalog version",
        ));
    }
    normalized_catalog(catalog.applications).map(Some)
}
pub fn write_catalog(directory: &std::path::Path, apps: Vec<Application>) -> Result<()> {
    use std::io::Write;
    let catalog = Catalog {
        version: 1,
        applications: normalized_catalog(apps)?,
    };
    let bytes = serde_json::to_vec_pretty(&catalog).map_err(|_| {
        Error::new(
            "CatalogInvalid",
            "applications",
            "Catalog serialization failed",
        )
    })?;
    if bytes.len() > 2_000_000 {
        return Err(Error::new(
            "CatalogLimit",
            "applications",
            "Catalog exceeds 2 MB",
        ));
    }
    let pending = directory.join(format!("applications-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&pending)
            .map_err(|e| Error::io("applications-write", e))?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|e| Error::io("applications-write", e))?;
        drop(file);
        std::fs::rename(&pending, directory.join("applications.json"))
            .map_err(|e| Error::io("applications-replace", e))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&pending);
    }
    result
}

#[cfg(windows)]
pub fn scan() -> Result<Vec<Application>> {
    use windows_sys::Win32::{
        Foundation::*,
        System::{Diagnostics::ToolHelp::*, Threading::*},
    };
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(Error::io(
            "applications-scan",
            std::io::Error::last_os_error(),
        ));
    }
    let mut found = std::collections::BTreeMap::new();
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut next = unsafe { Process32FirstW(snapshot, &mut entry) };
    while next != 0 {
        let handle =
            unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, entry.th32ProcessID) };
        if !handle.is_null() {
            let mut buffer = [0u16; 32768];
            let mut len = buffer.len() as u32;
            if unsafe { QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut len) } != 0
            {
                let path = String::from_utf16_lossy(&buffer[..len as usize]);
                let name = std::path::Path::new(&path)
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                found.insert(
                    path.to_lowercase(),
                    Application {
                        name,
                        path,
                        source: "running_process".into(),
                    },
                );
            }
            unsafe { CloseHandle(handle) };
        }
        next = unsafe { Process32NextW(snapshot, &mut entry) };
    }
    unsafe { CloseHandle(snapshot) };
    // Registry discovery is read-only; an entry is accepted only for an existing
    // EXE file. Installer/uninstaller command strings are never executed.
    use winreg::{
        RegKey,
        enums::{
            HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
        },
    };
    fn add(
        found: &mut std::collections::BTreeMap<String, Application>,
        path: &str,
        name: &str,
        source: &str,
    ) {
        let path = path.trim().trim_matches('"');
        let candidate = std::path::Path::new(path);
        if candidate.is_absolute()
            && candidate.is_file()
            && candidate
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
            && let Ok(canonical) = std::fs::canonicalize(candidate)
        {
            let path = canonical
                .to_string_lossy()
                .trim_start_matches(r"\\?\")
                .to_owned();
            found
                .entry(path.to_lowercase())
                .or_insert_with(|| Application {
                    name: if name.is_empty() {
                        candidate
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned()
                    } else {
                        name.to_owned()
                    },
                    path,
                    source: source.into(),
                });
        }
    }
    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        let root = RegKey::predef(hive);
        for view in [KEY_WOW64_32KEY, KEY_WOW64_64KEY] {
            if let Ok(key) = root.open_subkey_with_flags(
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths",
                KEY_READ | view,
            ) {
                for name in key.enum_keys().flatten() {
                    if let Ok(entry) = key.open_subkey(name)
                        && let Ok(path) = entry.get_value::<String, _>("")
                    {
                        add(&mut found, &path, "", "registry_app_paths");
                    }
                }
            }
            if let Ok(key) = root.open_subkey_with_flags(
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
                KEY_READ | view,
            ) {
                for name in key.enum_keys().flatten() {
                    if let Ok(entry) = key.open_subkey(name) {
                        let name = entry
                            .get_value::<String, _>("DisplayName")
                            .unwrap_or_default();
                        if let Ok(icon) = entry.get_value::<String, _>("DisplayIcon") {
                            let path = icon
                                .rsplit_once(',')
                                .filter(|(_, n)| n.parse::<i32>().is_ok())
                                .map_or(icon.as_str(), |(p, _)| p);
                            add(&mut found, path, &name, "registry_installed");
                        }
                    }
                }
            }
        }
    }
    // Read Start Menu shortcut targets through Windows' own shortcut resolver.
    // Fixed code only: no provider/profile text is interpolated into a shell.
    let mut system = [0u16; 32768];
    let n = unsafe {
        windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW(
            system.as_mut_ptr(),
            system.len() as u32,
        )
    };
    if n > 0 && (n as usize) < system.len() {
        let exe = std::path::PathBuf::from(String::from_utf16_lossy(&system[..n as usize]))
            .join(r"WindowsPowerShell\v1.0\powershell.exe");
        let script = r#"$ErrorActionPreference='SilentlyContinue'; $w=New-Object -ComObject WScript.Shell; $roots=@([Environment]::GetFolderPath('StartMenu'),[Environment]::GetFolderPath('CommonStartMenu')); $items=@(foreach($root in $roots){Get-ChildItem -LiteralPath $root -Filter '*.lnk' -Recurse | Select-Object -First 2000 | ForEach-Object {$link=$w.CreateShortcut($_.FullName); if($link.TargetPath.EndsWith('.exe',[StringComparison]::OrdinalIgnoreCase)){[PSCustomObject]@{name=$_.BaseName;path=$link.TargetPath}}}}); ConvertTo-Json -InputObject $items -Compress"#;
        use std::os::windows::process::CommandExt;
        if let Ok(mut child) = std::process::Command::new(exe)
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .creation_flags(0x08000000)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            let stdout = child.stdout.take();
            let reader = std::thread::spawn(move || {
                use std::io::Read;
                let mut bytes = vec![];
                if let Some(out) = stdout {
                    let _ = out.take(8 * 1024 * 1024 + 1).read_to_end(&mut bytes);
                }
                bytes
            });
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
            loop {
                if child.try_wait().ok().flatten().is_some() {
                    break;
                }
                if std::time::Instant::now() > deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            if let Ok(bytes) = reader.join()
                && bytes.len() <= 8 * 1024 * 1024
                && let Ok(values) = serde_json::from_slice::<Vec<serde_json::Value>>(&bytes)
            {
                for v in values {
                    if let Some(path) = v["path"].as_str() {
                        add(
                            &mut found,
                            path,
                            v["name"].as_str().unwrap_or(""),
                            "start_menu",
                        );
                    }
                }
            }
        }
    }
    // Epic manifests give an exact launch executable, avoiding filesystem-wide scans.
    if let Some(program_data) = std::env::var_os("ProgramData") {
        let directory =
            std::path::PathBuf::from(program_data).join(r"Epic\EpicGamesLauncher\Data\Manifests");
        if let Ok(entries) = std::fs::read_dir(directory) {
            for entry in entries.flatten().take(2000) {
                let p = entry.path();
                if p.extension().is_some_and(|v| v == "item")
                    && entry.metadata().is_ok_and(|m| m.len() < 1024 * 1024)
                    && let Ok(bytes) = std::fs::read(p)
                    && let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes)
                    && let (Some(root), Some(exe)) = (
                        v["InstallLocation"].as_str(),
                        v["LaunchExecutable"].as_str(),
                    )
                {
                    add(
                        &mut found,
                        &std::path::Path::new(root).join(exe).to_string_lossy(),
                        v["DisplayName"].as_str().unwrap_or(""),
                        "epic_manifest",
                    );
                }
            }
        }
    }
    let mut libraries = vec![];
    if let Ok(steam) = RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"SOFTWARE\Valve\Steam")
        && let Ok(path) = steam.get_value::<String, _>("SteamPath")
    {
        let root = std::path::PathBuf::from(path);
        libraries.push(root.clone());
        if let Ok(vdf) = std::fs::read_to_string(root.join(r"steamapps\libraryfolders.vdf")) {
            for line in vdf.lines() {
                let parts: Vec<_> = line.split('"').collect();
                if parts.get(1) == Some(&"path")
                    && let Some(path) = parts.get(3)
                {
                    libraries.push(std::path::PathBuf::from(path.replace(r"\\", r"\")));
                }
            }
        }
    }
    for library in libraries {
        if let Ok(entries) = std::fs::read_dir(library.join(r"steamapps\common")) {
            for game in entries.flatten().take(500) {
                if !game.path().is_dir() {
                    continue;
                }
                let name = game.file_name().to_string_lossy().into_owned();
                let mut dirs = vec![(game.path(), 0)];
                let mut scanned = 0;
                while let Some((dir, depth)) = dirs.pop() {
                    if scanned > 4000 {
                        break;
                    }
                    if let Ok(entries) = std::fs::read_dir(dir) {
                        for entry in entries.flatten() {
                            scanned += 1;
                            let path = entry.path();
                            if entry.file_type().is_ok_and(|t| t.is_dir()) && depth < 3 {
                                dirs.push((path, depth + 1));
                            } else {
                                add(&mut found, &path.to_string_lossy(), &name, "steam_library");
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(found.into_values().collect())
}
#[cfg(not(windows))]
pub fn scan() -> Result<Vec<Application>> {
    Err(Error::new(
        "PlatformUnsupported",
        "applications-scan",
        "Windows process inventory is required",
    ))
}

#[cfg(test)]
mod catalog_tests {
    use super::*;
    #[test]
    fn separate_catalog_normalizes_paths_and_replaces_atomically() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_catalog(dir.path()).unwrap(), None);
        let app = |path: &str| Application {
            name: "tool".into(),
            path: path.into(),
            source: "manual".into(),
        };
        write_catalog(
            dir.path(),
            vec![app(r"C:/Apps/tool.exe"), app(r"\\?\C:\Apps\TOOL.exe")],
        )
        .unwrap();
        assert_eq!(read_catalog(dir.path()).unwrap().unwrap().len(), 1);
        write_catalog(dir.path(), vec![app(r"C:\Other\tool.exe")]).unwrap();
        assert_eq!(
            read_catalog(dir.path()).unwrap().unwrap()[0].path,
            r"C:\Other\tool.exe"
        );
        assert!(write_catalog(dir.path(), vec![app(r"C:\Apps\..\bad.exe")]).is_err());
        assert_eq!(read_catalog(dir.path()).unwrap().unwrap().len(), 1);
        std::fs::write(dir.path().join("applications.json"), "broken").unwrap();
        assert!(read_catalog(dir.path()).is_err());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("applications.json")).unwrap(),
            "broken"
        );
    }
}
