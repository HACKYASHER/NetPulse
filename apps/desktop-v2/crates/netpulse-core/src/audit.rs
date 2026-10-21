//! Date-separated, bounded JSON-lines audit, independent of UI filters.
use crate::{Error, Result, store::LogEvent};
use std::{
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
    sync::Mutex,
};
static WRITER: Mutex<()> = Mutex::new(());
const LIMIT: u64 = 8 * 1024 * 1024;
pub const CATEGORIES: &[&str] = &["main", "core", "tunnel", "subscriptions", "service"];
pub fn date(timestamp: u64) -> String {
    #[cfg(windows)]
    let timestamp = {
        use windows_sys::Win32::{
            Foundation::FILETIME, Storage::FileSystem::FileTimeToLocalFileTime,
        };
        let ticks = timestamp
            .saturating_add(11_644_473_600)
            .saturating_mul(10_000_000);
        let input = FILETIME {
            dwLowDateTime: ticks as u32,
            dwHighDateTime: (ticks >> 32) as u32,
        };
        let mut local = FILETIME::default();
        if unsafe { FileTimeToLocalFileTime(&input, &mut local) } != 0 {
            (((local.dwHighDateTime as u64) << 32) | local.dwLowDateTime as u64)
                .saturating_div(10_000_000)
                .saturating_sub(11_644_473_600)
        } else {
            timestamp
        }
    };
    // Gregorian civil date from Unix days; no date formatting dependency.
    let z = (timestamp / 86400) as i64 + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }
    format!("{year:04}-{month:02}-{day:02}")
}
fn valid_date(s: &str) -> bool {
    s.len() == 10
        && s.bytes().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                c == b'-'
            } else {
                c.is_ascii_digit()
            }
        })
}
pub fn category(e: &LogEvent) -> &'static str {
    if ["Subscription", "Subscriptions"].contains(&e.subsystem.as_str())
        || e.stage.starts_with("subscription")
    {
        "subscriptions"
    } else if e.subsystem == "Service" || e.stage.contains("service") {
        "service"
    } else if ["Core", "Engine"].contains(&e.subsystem.as_str())
        || e.stage.starts_with("core-")
        || e.stage.starts_with("xray-")
    {
        "core"
    } else if ["Connection", "Tunnel", "TUN"].contains(&e.subsystem.as_str())
        || e.stage.contains("tun-")
    {
        "tunnel"
    } else {
        "main"
    }
}
fn write(folder: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    let path = folder.join(format!("{name}.jsonl"));
    if path.metadata().is_ok_and(|m| m.len() >= LIMIT) {
        let oldest = folder.join(format!("{name}.5.jsonl"));
        if oldest.exists() {
            std::fs::remove_file(oldest).map_err(|e| Error::io("audit-rotate", e))?;
        }
        for n in (1..5).rev() {
            let source = folder.join(format!("{name}.{n}.jsonl"));
            if source.exists() {
                std::fs::rename(source, folder.join(format!("{name}.{}.jsonl", n + 1)))
                    .map_err(|e| Error::io("audit-rotate", e))?;
            }
        }
        std::fs::rename(&path, folder.join(format!("{name}.1.jsonl")))
            .map_err(|e| Error::io("audit-rotate", e))?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| Error::io("audit-open", e))?;
    file.write_all(bytes)
        .map_err(|e| Error::io("audit-write", e))
}
fn prune(root: &Path) {
    // Only managed files under strict date folders. Never follow a symlink or
    // touch legacy files or arbitrary user files placed in the logs directory.
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    let mut files = Vec::new();
    for day in entries.flatten() {
        if !day.file_type().is_ok_and(|t| t.is_dir())
            || !valid_date(&day.file_name().to_string_lossy())
        {
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(day.path()) {
            for file in entries.flatten() {
                if !file.file_type().is_ok_and(|t| t.is_file()) {
                    continue;
                }
                let name = file.file_name().to_string_lossy().into_owned();
                let managed = CATEGORIES.iter().any(|c| {
                    name == format!("{c}.jsonl")
                        || (1..=5).any(|n| name == format!("{c}.{n}.jsonl"))
                });
                if managed && let Ok(meta) = file.metadata() {
                    files.push((meta.modified().ok(), file.path(), meta.len()));
                }
            }
        }
    }
    let mut size: u64 = files.iter().map(|v| v.2).sum();
    files.sort_by_key(|v| v.0);
    for (_, path, len) in files {
        if size <= 128 * 1024 * 1024 {
            break;
        }
        if std::fs::remove_file(path).is_ok() {
            size = size.saturating_sub(len);
        }
    }
}
pub fn append(directory: &Path, event: &LogEvent) -> Result<()> {
    let _guard = WRITER
        .lock()
        .map_err(|_| Error::new("AuditUnavailable", "audit", "Audit writer unavailable"))?;
    let root = directory.join("logs");
    let folder = root.join(date(event.timestamp));
    std::fs::create_dir_all(&folder).map_err(|e| Error::io("audit-directory", e))?;
    let mut bytes = serde_json::to_vec(event)
        .map_err(|_| Error::new("AuditSerialize", "audit", "Could not encode audit event"))?;
    bytes.push(b'\n');
    write(&folder, "main", &bytes)?;
    let group = category(event);
    if group != "main" {
        write(&folder, group, &bytes)?;
    }
    // Check on rotation-sized boundaries instead of scanning on each command.
    static LAST: Mutex<Option<std::time::Instant>> = Mutex::new(None);
    if let Ok(mut last) = LAST.lock()
        && last.is_none_or(|t| t.elapsed().as_secs() >= 60)
    {
        prune(&root);
        *last = Some(std::time::Instant::now());
    }
    Ok(())
}
pub fn days(directory: &Path) -> Result<Vec<String>> {
    let root = directory.join("logs");
    if !root.exists() {
        return Ok(vec![]);
    }
    let mut days = std::fs::read_dir(root)
        .map_err(|e| Error::io("audit-days", e))?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|s| valid_date(s))
        .collect::<Vec<_>>();
    days.sort();
    days.reverse();
    days.truncate(366);
    Ok(days)
}
fn regular_metadata(path: &Path) -> Result<std::fs::Metadata> {
    let meta = path
        .symlink_metadata()
        .map_err(|e| Error::io("audit-clear", e))?;
    #[cfg(windows)]
    let linked = {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0 // includes NTFS junctions
    };
    #[cfg(not(windows))]
    let linked = meta.file_type().is_symlink();
    if linked {
        return Err(Error::new(
            "InvalidLogPath",
            "audit-clear",
            "Папка или файл журнала заменены ссылкой",
        ));
    }
    Ok(meta)
}
/// Clear all managed categories/dates and known legacy logs, never arbitrary
/// user files, subdirectories, symlinks or NTFS junctions. Serialize with writers.
pub fn clear(directory: &Path) -> Result<()> {
    let _guard = WRITER.lock().map_err(|_| {
        Error::new(
            "AuditUnavailable",
            "audit-clear",
            "Audit writer unavailable",
        )
    })?;
    let root = directory.join("logs");
    if !root.exists() {
        return Ok(());
    }
    regular_metadata(&root)?;
    let mut files = Vec::new();
    let mut folders = Vec::new();
    for entry in std::fs::read_dir(&root).map_err(|e| Error::io("audit-clear", e))? {
        let entry = entry.map_err(|e| Error::io("audit-clear", e))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "netpulse.jsonl" || (1..=5).any(|n| name == format!("netpulse.{n}.jsonl")) {
            if regular_metadata(&entry.path())?.is_file() {
                files.push(entry.path());
            }
        } else if valid_date(&name) {
            let path = entry.path();
            if !regular_metadata(&path)?.is_dir() {
                continue;
            }
            for file in std::fs::read_dir(&path).map_err(|e| Error::io("audit-clear", e))? {
                let file = file.map_err(|e| Error::io("audit-clear", e))?;
                let name = file.file_name().to_string_lossy().into_owned();
                if CATEGORIES.iter().any(|c| {
                    name == format!("{c}.jsonl")
                        || (1..=5).any(|n| name == format!("{c}.{n}.jsonl"))
                }) && regular_metadata(&file.path())?.is_file()
                {
                    files.push(file.path());
                }
            }
            folders.push(path);
        }
    }
    // Validate the full selection before mutating any of it.
    for file in files {
        std::fs::remove_file(file).map_err(|e| Error::io("audit-clear", e))?;
    }
    for folder in folders {
        let _ = std::fs::remove_dir(folder);
    } // only empty folders
    Ok(())
}
pub fn read(directory: &Path, day: &str, category: &str) -> Result<String> {
    if !valid_date(day) || !CATEGORIES.contains(&category) {
        return Err(Error::new(
            "InvalidLogPath",
            "audit-read",
            "Недопустимая дата или раздел журнала",
        ));
    }
    let folder = directory.join("logs").join(day);
    if folder
        .symlink_metadata()
        .is_ok_and(|m| m.file_type().is_symlink())
    {
        return Err(Error::new(
            "InvalidLogPath",
            "audit-read",
            "Ссылка вместо папки журнала",
        ));
    }
    let path = folder.join(format!("{category}.jsonl"));
    if !path.exists() {
        return Ok(String::new());
    }
    if path
        .symlink_metadata()
        .is_ok_and(|m| m.file_type().is_symlink())
    {
        return Err(Error::new(
            "InvalidLogPath",
            "audit-read",
            "Ссылка вместо файла журнала",
        ));
    }
    let mut file = std::fs::File::open(path).map_err(|e| Error::io("audit-read", e))?;
    let length = file
        .metadata()
        .map_err(|e| Error::io("audit-read", e))?
        .len();
    let offset = length.saturating_sub(256 * 1024);
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| Error::io("audit-read", e))?;
    let mut bytes = Vec::new();
    file.take(256 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|e| Error::io("audit-read", e))?;
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if offset > 0 {
        text = text
            .split_once('\n')
            .map(|(_, tail)| tail.to_owned())
            .unwrap_or_default();
    }
    Ok(text
        .lines()
        .rev()
        .take(500)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n"))
}
#[cfg(test)]
mod tests {
    #[test]
    fn clear_removes_all_categories_rotations_and_dates_but_preserves_other_files() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("logs");
        for date in ["2026-10-20", "2026-10-21"] {
            let folder = root.join(date);
            std::fs::create_dir_all(&folder).unwrap();
            for category in super::CATEGORIES {
                std::fs::write(folder.join(format!("{category}.jsonl")), "fixture").unwrap();
                std::fs::write(folder.join(format!("{category}.5.jsonl")), "fixture").unwrap();
            }
        }
        std::fs::write(root.join("netpulse.jsonl"), "legacy").unwrap();
        std::fs::write(root.join("notes.txt"), "keep").unwrap();
        std::fs::write(root.join("2026-10-21").join("notes.txt"), "keep").unwrap();
        super::clear(temp.path()).unwrap();
        assert_eq!(super::days(temp.path()).unwrap(), vec!["2026-10-21"]);
        assert!(
            super::read(temp.path(), "2026-10-21", "core")
                .unwrap()
                .is_empty()
        );
        assert!(!root.join("2026-10-20").exists());
        assert!(!root.join("netpulse.jsonl").exists());
        assert!(root.join("notes.txt").exists());
        assert!(root.join("2026-10-21").join("notes.txt").exists());
        super::clear(temp.path()).unwrap();
    }
    #[test]
    fn categories_share_a_local_date_and_tail_reader_rejects_traversal() {
        let temp = tempfile::tempdir().unwrap();
        let event = crate::store::LogEvent {
            id: "fixture".into(),
            timestamp: crate::store::now(),
            level: "ERROR".into(),
            subsystem: "Core".into(),
            stage: "runtime-output".into(),
            code: "Fixture".into(),
            message: "Проверка без секретов".into(),
        };
        super::append(temp.path(), &event).unwrap();
        let days = super::days(temp.path()).unwrap();
        assert_eq!(days.len(), 1);
        assert!(
            super::read(temp.path(), &days[0], "main")
                .unwrap()
                .contains("Проверка")
        );
        assert!(
            super::read(temp.path(), &days[0], "core")
                .unwrap()
                .contains("Проверка")
        );
        assert!(
            super::read(temp.path(), &days[0], "service")
                .unwrap()
                .is_empty()
        );
        assert!(super::read(temp.path(), "../../elsewhere", "main").is_err());
        assert!(super::read(temp.path(), &days[0], "../../elsewhere").is_err());
        let path = temp.path().join("logs").join(&days[0]).join("main.jsonl");
        std::fs::write(path, "Проверка\n".repeat(40000)).unwrap();
        let tail = super::read(temp.path(), &days[0], "main").unwrap();
        assert_eq!(tail.lines().count(), 500);
        assert!(tail.lines().all(|s| s == "Проверка"));
    }
    #[test]
    fn audit_rotates_without_losing_the_new_event() {
        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path().join("logs").join(super::date(0));
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("main.jsonl"), vec![b' '; super::LIMIT as usize]).unwrap();
        let event = crate::store::LogEvent {
            id: "fixture".into(),
            timestamp: 0,
            level: "INFO".into(),
            subsystem: "Test".into(),
            stage: "rotation".into(),
            code: "Rotated".into(),
            message: "Newest event".into(),
        };
        super::append(temp.path(), &event).unwrap();
        assert_eq!(
            folder.join("main.1.jsonl").metadata().unwrap().len(),
            super::LIMIT
        );
        let saved: crate::store::LogEvent =
            serde_json::from_str(&std::fs::read_to_string(folder.join("main.jsonl")).unwrap())
                .unwrap();
        assert_eq!(saved.id, "fixture");
    }
    #[test]
    fn file_audit_survives_activity_clear_and_ignores_legacy_threshold() {
        let temp = tempfile::tempdir().unwrap();
        let store = crate::store::Store::open(&temp.path().join("test.db")).unwrap();
        let mut preferences = store.preferences().unwrap();
        preferences.logging_level = "ERROR".into();
        store.save_preferences(&preferences).unwrap();
        store
            .log("DEBUG", "Interface", "click", "Action", "button")
            .unwrap();
        store
            .log(
                "ERROR",
                "Interface",
                "runtime",
                "UiRuntimeFailure",
                "Runtime failure",
            )
            .unwrap();
        store.clear_logs().unwrap();
        let text = std::fs::read_to_string(
            temp.path()
                .join("logs")
                .join(super::date(crate::store::now()))
                .join("main.jsonl"),
        )
        .unwrap();
        assert!(!text.contains("DEBUG"));
        assert!(text.contains("UiRuntimeFailure"));
        assert!(store.logs().unwrap().is_empty());
        assert!(
            store
                .log("INFO", "Interface", "click", "Action", "vless://secret")
                .is_err()
        );
        assert!(
            !std::fs::read_to_string(
                temp.path()
                    .join("logs")
                    .join(super::date(crate::store::now()))
                    .join("main.jsonl")
            )
            .unwrap()
            .contains("secret")
        );
    }
    #[test]
    fn poll_commands_stay_in_file_and_do_not_bury_errors_in_events() {
        let d = tempfile::tempdir().unwrap();
        let store = crate::store::Store::open(&d.path().join("test.db")).unwrap();
        store
            .log(
                "ERROR",
                "Connection",
                "probe",
                "ProbeFailed",
                "Owned fixture failure",
            )
            .unwrap();
        for _ in 0..1100 {
            store
                .log(
                    "DEBUG",
                    "Command",
                    "snapshot",
                    "Started",
                    "Native command started",
                )
                .unwrap();
        }
        let events = store.logs().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].code, "ProbeFailed");
        assert_eq!(
            store.event(&events[0].id).unwrap().message,
            "Owned fixture failure"
        );
        assert!(
            std::fs::read_to_string(
                d.path()
                    .join("logs")
                    .join(super::date(crate::store::now()))
                    .join("main.jsonl")
            )
            .unwrap()
            .contains("Native command started")
        );
    }
}
