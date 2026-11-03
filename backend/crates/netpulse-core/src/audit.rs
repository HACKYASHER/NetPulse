//! Date-separated audit with bounded backward paging, independent of UI filters.
use crate::{Error, Result, store::LogEvent};
use std::{
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
    sync::Mutex,
};
static WRITER: Mutex<()> = Mutex::new(());
const PAGE_BYTES: u64 = 1024 * 1024;
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
    if path.symlink_metadata().is_ok() && !regular_metadata(&path)?.is_file() {
        return Err(Error::new("InvalidLogPath","audit-write","Журнал должен быть обычным файлом"));
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| Error::io("audit-open", e))?;
    file.write_all(bytes)
        .map_err(|e| Error::io("audit-write", e))
}
fn managed_name(name: &str) -> bool {
    CATEGORIES.iter().any(|c|name==format!("{c}.jsonl")||(1..=5).any(|n|name==format!("{c}.{n}.jsonl")))
}
fn prune_at(root: &Path, timestamp:u64) {
    // Keep today and four preceding local dates. Unknown files and links are never removed.
    if crate::layout::normal_directory(root).is_err(){return;}
    let oldest=date(timestamp.saturating_sub(4*86400));
    let Ok(entries)=std::fs::read_dir(root) else{return;};
    for day in entries.flatten() {
        let name=day.file_name().to_string_lossy().into_owned();let path=day.path();
        if !valid_date(&name)||name>=oldest||crate::layout::normal_directory(&path).is_err()||!regular_metadata(&path).is_ok_and(|m|m.is_dir()){continue;}
        if let Ok(files)=std::fs::read_dir(&path) {
            for file in files.flatten() {
                if managed_name(&file.file_name().to_string_lossy())&&regular_metadata(&file.path()).is_ok_and(|m|m.is_file()) {let _=std::fs::remove_file(file.path());}
            }
        }
        let _=std::fs::remove_dir(path); // empty managed folders only
    }
}
pub fn append(directory: &Path, event: &LogEvent) -> Result<()> {
    let _guard = WRITER
        .lock()
        .map_err(|_| Error::new("AuditUnavailable", "audit", "Audit writer unavailable"))?;
    let root = directory.join("logs");
    let folder = root.join(date(event.timestamp));
    crate::layout::normal_directory(&folder)?;
    std::fs::create_dir_all(&folder).map_err(|e| Error::io("audit-directory", e))?;
    let mut bytes = serde_json::to_vec(event)
        .map_err(|_| Error::new("AuditSerialize", "audit", "Could not encode audit event"))?;
    bytes.push(b'\n');
    write(&folder, "main", &bytes)?;
    let group = category(event);
    if group != "main" {
        write(&folder, group, &bytes)?;
    }
    // Retention housekeeping is independent of file size.
    static LAST: Mutex<Option<std::time::Instant>> = Mutex::new(None);
    if let Ok(mut last) = LAST.lock()
        && last.is_none_or(|t| t.elapsed().as_secs() >= 60)
    {
        prune_at(&root,crate::store::now());
        *last = Some(std::time::Instant::now());
    }
    Ok(())
}
pub fn days(directory: &Path) -> Result<Vec<String>> {
    let root = directory.join("logs");
    crate::layout::normal_directory(&root)?;
    if !root.exists() {
        return Ok(vec![]);
    }
    let _guard=WRITER.lock().map_err(|_|Error::new("AuditUnavailable","audit-days","Audit writer unavailable"))?;
    prune_at(&root,crate::store::now());
    let mut days = std::fs::read_dir(root)
        .map_err(|e| Error::io("audit-days", e))?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| crate::layout::normal_directory(&e.path()).is_ok())
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
    crate::layout::normal_directory(&root)?;
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
#[derive(serde::Serialize)]
pub struct Page {pub text:String,pub next:Option<String>}
#[derive(serde::Serialize,serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {day:String,category:String,rotation:u8,offset:u64,head_len:usize,head:String}
fn head(file:&mut std::fs::File,len:usize)->Result<String> {
    use sha2::{Digest,Sha256};
    file.seek(SeekFrom::Start(0)).map_err(|e|Error::io("audit-read",e))?;
    let mut bytes=vec![0;len];file.read_exact(&mut bytes).map_err(|e|Error::io("audit-read",e))?;
    Ok(Sha256::digest(&bytes).iter().map(|b|format!("{b:02x}")).collect())
}
pub fn page(directory:&Path,day:&str,category:&str,cursor:Option<&str>,level:&str)->Result<Page> {
    let invalid=||Error::new("InvalidLogPath","audit-read","Недопустимая дата, раздел, уровень или страница журнала");
    if !valid_date(day)||!CATEGORIES.contains(&category)||!["ALL","TRACE","DEBUG","INFO","WARNING","ERROR"].contains(&level){return Err(invalid());}
    let folder=directory.join("logs").join(day);crate::layout::normal_directory(&folder)?;
    let _guard=WRITER.lock().map_err(|_|Error::new("AuditUnavailable","audit-read","Audit writer unavailable"))?;
    let mut position=cursor.map(|c|if c.len()>1024 {Err(invalid())}else{serde_json::from_str::<Cursor>(c).map_err(|_|invalid())}).transpose()?;
    if position.as_ref().is_some_and(|c|c.day!=day||c.category!=category||c.rotation>5||c.head_len>256){return Err(invalid());}
    let first=position.as_ref().map_or(0,|c|c.rotation);
    let mut remaining=PAGE_BYTES;let mut rows=Vec::new();let mut next=None;
    for rotation in first..=5 {
        let name=if rotation==0{format!("{category}.jsonl")}else{format!("{category}.{rotation}.jsonl")};let path=folder.join(name);
        if !path.exists(){if position.is_some(){return Err(invalid());}continue;}
        if !regular_metadata(&path)?.is_file(){return Err(invalid());}
        let mut file=std::fs::File::open(&path).map_err(|e|Error::io("audit-read",e))?;
        let len=file.metadata().map_err(|e|Error::io("audit-read",e))?.len();
        let (end,head_len,identity)=if let Some(c)=position.take(){
            if c.offset>len||c.head_len as u64>len||head(&mut file,c.head_len)?!=c.head{return Err(Error::new("LogChanged","audit-read","Журнал изменился или очищен. Откройте последнюю страницу."));}
            (c.offset,c.head_len,c.head)
        }else{let count=len.min(256) as usize;(len,count,head(&mut file,count)?)};
        let start=end.saturating_sub(remaining);file.seek(SeekFrom::Start(start)).map_err(|e|Error::io("audit-read",e))?;
        let mut bytes=Vec::new();file.take(end-start).read_to_end(&mut bytes).map_err(|e|Error::io("audit-read",e))?;
        remaining=remaining.saturating_sub(bytes.len() as u64);
        let complete_start=if start==0{0}else{bytes.iter().position(|b|*b==b'\n').map_or(bytes.len(),|n|n+1)};
        let mut offset=end;let mut stopped=false;
        for line in bytes[complete_start..].split_inclusive(|b|*b==b'\n').rev() {
            offset=offset.saturating_sub(line.len() as u64);
            let text=String::from_utf8_lossy(line).trim_end_matches(['\n','\r']).to_owned();
            let matches=level=="ALL"||serde_json::from_str::<LogEvent>(&text).is_ok_and(|e|e.level==level);
            if !text.is_empty()&&matches {rows.push(text);}
            if rows.len()>=500 {stopped=true;break;}
        }
        if !stopped {offset=if complete_start>0{start+complete_start as u64}else{0};if offset==end&&start>0{offset=start;}}
        if offset>0 || remaining==0 || rows.len()>=500 {
            // Even offset zero is a valid continuation into legacy rotated files.
            if offset>0||rotation<5 {next=Some(serde_json::to_string(&Cursor{day:day.into(),category:category.into(),rotation,offset,head_len,head:identity}).map_err(|_|invalid())?);}
            break;
        }
    }
    rows.reverse();Ok(Page{text:rows.join("\n"),next})
}
pub fn read(directory:&Path,day:&str,category:&str)->Result<String>{Ok(page(directory,day,category,None,"ALL")?.text)}
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
    fn append_does_not_rotate_away_today_and_prunes_only_expired_managed_files() {
        let temp=tempfile::tempdir().unwrap();let now=crate::store::now();
        let folder=temp.path().join("logs").join(super::date(now));std::fs::create_dir_all(&folder).unwrap();
        let old=temp.path().join("logs").join(super::date(now-6*86400));std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("main.jsonl"),"expire").unwrap();std::fs::write(old.join("notes.txt"),"keep").unwrap();
        std::fs::write(folder.join("main.jsonl"),vec![b' ';8*1024*1024]).unwrap();
        let event=crate::store::LogEvent{id:"fixture".into(),timestamp:now,level:"INFO".into(),subsystem:"Test".into(),stage:"append".into(),code:"Appended".into(),message:"Newest event".into()};
        super::append(temp.path(),&event).unwrap();
        super::prune_at(&temp.path().join("logs"),now);
        assert!(!folder.join("main.1.jsonl").exists());assert!(folder.join("main.jsonl").metadata().unwrap().len()>8*1024*1024);
        assert!(!old.join("main.jsonl").exists());assert!(old.join("notes.txt").exists());
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

#[cfg(test)]
mod rotation_reader_tests {
    #[test]
    fn empty_current_core_keeps_rotated_entries_in_order() {
        let dir=tempfile::tempdir().unwrap();let folder=dir.path().join("logs/2026-10-25");std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("core.jsonl"),"").unwrap();
        std::fs::write(folder.join("core.1.jsonl"),"old1\nold2\n").unwrap();
        assert_eq!(super::read(dir.path(),"2026-10-25","core").unwrap(),"old1\nold2");
        std::fs::write(folder.join("core.jsonl"),"current\n").unwrap();
        assert_eq!(super::read(dir.path(),"2026-10-25","core").unwrap(),"old1\nold2\ncurrent");
        std::fs::write(folder.join("core.2.jsonl"),(0..700).map(|i|format!("line{i}\n")).collect::<String>()).unwrap();
        let text=super::read(dir.path(),"2026-10-25","core").unwrap();
        assert_eq!(text.lines().count(),500);assert!(text.ends_with("old1\nold2\ncurrent"));
    }
}

#[cfg(test)]
mod paging_tests {
 #[test]
 fn pages_visit_every_line_across_legacy_rotations_and_exact_filters() {
  let d=tempfile::tempdir().unwrap();let folder=d.path().join("logs/2026-11-04");std::fs::create_dir_all(&folder).unwrap();
  for (name,start,end) in [("core.1.jsonl",0,700),("core.jsonl",700,1900)] {std::fs::write(folder.join(name),(start..end).map(|i|format!("{{\"id\":\"{i}\",\"timestamp\":1,\"level\":\"{}\",\"subsystem\":\"Core\",\"stage\":\"test\",\"code\":\"Test\",\"message\":\"строка\"}}\n",if i%2==0{"ERROR"}else{"INFO"})).collect::<String>()).unwrap();}
  for (level,count) in [("ALL",1900),("ERROR",950)] {
   let mut cursor=None;let mut ids=std::collections::HashSet::new();let mut pages=0;
   loop {let page=super::page(d.path(),"2026-11-04","core",cursor.as_deref(),level).unwrap();for line in page.text.lines(){let v:serde_json::Value=serde_json::from_str(line).unwrap();assert!(ids.insert(v["id"].as_str().unwrap().to_owned()));}pages+=1;cursor=page.next;if cursor.is_none(){break;}assert!(pages<20);}
   assert_eq!(ids.len(),count);
  }
 }
 #[test]
 fn cursor_detects_replaced_file_and_rejects_cross_category() {
  let d=tempfile::tempdir().unwrap();let folder=d.path().join("logs/2026-11-04");std::fs::create_dir_all(&folder).unwrap();let path=folder.join("core.jsonl");std::fs::write(&path,"old\n".repeat(800)).unwrap();
  let page=super::page(d.path(),"2026-11-04","core",None,"ALL").unwrap();let cursor=page.next.unwrap();
  assert!(super::page(d.path(),"2026-11-04","main",Some(&cursor),"ALL").is_err());
  std::fs::write(&path,"new\n".repeat(800)).unwrap();assert_eq!(super::page(d.path(),"2026-11-04","core",Some(&cursor),"ALL").err().unwrap().code,"LogChanged");
 }
}
