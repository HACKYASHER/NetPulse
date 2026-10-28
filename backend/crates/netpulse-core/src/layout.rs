//! Installed files and per-user profiles share one root; only profiles are writable.
use crate::{Error, Result};
use std::path::{Path, PathBuf};
use sha2::{Digest, Sha256};

pub fn normal_directory(path: &Path) -> Result<()> {
    use std::os::windows::fs::MetadataExt;
    // Inspect ancestors from the volume down: querying a descendant first can
    // already follow a parent's junction (including one pointing to a share).
    let mut parts=path.ancestors().collect::<Vec<_>>();parts.reverse();
    for part in parts {
        match std::fs::symlink_metadata(part) {
            Ok(m) if !m.is_dir() || m.file_attributes() & 0x400 != 0 =>
                return Err(Error::new("UnsafeDataDirectory", "layout", "Папка NetPulse не должна содержать ссылки или точки перенаправления.")),
            Ok(_) => {},
            Err(e) if e.kind()==std::io::ErrorKind::NotFound => {},
            Err(e) => return Err(Error::io("layout-directory",e)),
        }
    }
    Ok(())
}
pub fn local_directory(path:&Path)->Result<()> {
    let text=path.to_string_lossy();let bytes=text.as_bytes();
    if bytes.len()<3 || !bytes[0].is_ascii_alphabetic() || bytes[1]!=b':' || !matches!(bytes[2],b'\\'|b'/') || unsafe{windows_sys::Win32::Storage::FileSystem::GetDriveTypeW(crate::service::wide(&format!("{}:\\",bytes[0] as char)).as_ptr())}!=3 {
        return Err(Error::new("UnsafeDataDirectory","layout","Используйте папку на локальном диске без ссылок и сетевых путей."));
    }
    normal_directory(path)
}

fn permissions(path: &Path, sddl: &str) -> Result<()> {
    use windows_sys::Win32::{Foundation::*, Security::{*, Authorization::*}};
    let sd=crate::service::security_descriptor(sddl)?;
    let (mut dacl,mut present,mut defaulted)=(std::ptr::null_mut(),0,0);
    if unsafe {GetSecurityDescriptorDacl(sd.pointer(),&mut present,&mut dacl,&mut defaulted)}==0 || present==0 {
        return Err(Error::io("profile-security",std::io::Error::last_os_error()));
    }
    let mut name=crate::service::wide(&path.to_string_lossy());
    let code=unsafe {SetNamedSecurityInfoW(name.as_mut_ptr(),SE_FILE_OBJECT,DACL_SECURITY_INFORMATION|PROTECTED_DACL_SECURITY_INFORMATION,std::ptr::null_mut(),std::ptr::null_mut(),dacl,std::ptr::null_mut())};
    if code!=ERROR_SUCCESS {return Err(Error::io("profile-security",std::io::Error::from_raw_os_error(code as i32)));}
    Ok(())
}

/// Never execute a legacy per-user binary with administrator rights simply to
/// ask whether it is running. The trusted installer helper checks processes.
pub fn client_running() -> Result<bool> {
    use windows_sys::Win32::{Foundation::*,System::Diagnostics::ToolHelp::*};
    let snapshot=crate::service::Handle(unsafe{CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS,0)});
    if snapshot.0==INVALID_HANDLE_VALUE {return Err(Error::io("installer-process-check",std::io::Error::last_os_error()));}
    let mut row:PROCESSENTRY32W=unsafe{std::mem::zeroed()};row.dwSize=std::mem::size_of::<PROCESSENTRY32W>() as u32;
    let mut valid=unsafe{Process32FirstW(snapshot.0,&mut row)};
    while valid!=0 {
        let length=row.szExeFile.iter().position(|c|*c==0).unwrap_or(row.szExeFile.len());
        if String::from_utf16_lossy(&row.szExeFile[..length]).eq_ignore_ascii_case("NetPulse.exe") {return Ok(true);}
        valid=unsafe{Process32NextW(snapshot.0,&mut row)};
    }
    Ok(false)
}

/// Run by the elevated installer, before extracting files. Users can create their
/// own profile directory, but cannot write the parent or another user's profile.
pub fn prepare_install_root(root: &Path) -> Result<()> {
    crate::install_paths::validate_root(root)?;
    // Removing an empty, pre-existing directory also revokes previously opened
    // handles. Recreate it atomically with the final protected descriptor.
    if root.exists() && std::fs::read_dir(root).map_err(|e|Error::io("install-directory",e))?.next().is_none(){std::fs::remove_dir(root).map_err(|e|Error::io("install-directory",e))?;}
    let mut missing=root.ancestors().take_while(|p|!p.exists()).map(PathBuf::from).collect::<Vec<_>>();missing.reverse();
    for directory in missing {crate::install_paths::create_protected_directory(&directory)?;}
    let data=root.join("data");normal_directory(&data)?;
    if !data.exists(){crate::install_paths::create_protected_directory(&data)?;}
    crate::service::protect_install_path(&data)?;
    permissions(&data,"D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;;FRFX;;;BU)(A;;0x00000004;;;BU)(A;OICIIO;FA;;;CO)")?;
    // An interrupted first installation can resume only the machine-recorded,
    // already protected root; a file marker in an arbitrary folder is insufficient.
    crate::install_paths::record_pending_root(root)
}

/// Explicitly protect each installed file's owner as well as its inherited ACL.
/// Owning a file would otherwise let an unelevated user change that file's DACL.
pub fn finalize_install_root(root: &Path) -> Result<()> {
    use std::os::windows::fs::MetadataExt;
    normal_directory(root)?;
    let manifest=std::fs::read_to_string(root.join("installation-files.txt")).map_err(|e|Error::io("install-manifest",e))?;
    let mut parents=Vec::new();
    for name in manifest.lines() {
        let relative=Path::new(name);
        if relative.is_absolute() || relative.components().any(|c|!matches!(c,std::path::Component::Normal(_))) || relative.components().next().is_some_and(|p|p.as_os_str().to_string_lossy().eq_ignore_ascii_case("data")) {
            return Err(Error::new("InstallManifestInvalid","install-security","Invalid installation file manifest"));
        }
        let path=root.join(relative);normal_directory(path.parent().unwrap())?;
        let metadata=path.symlink_metadata().map_err(|e|Error::io("install-security",e))?;
        if !metadata.is_file() || metadata.file_attributes() & 0x400!=0 {return Err(Error::new("UnsafeInstallFile","install-security","Installation file must not be a link"));}
        crate::service::protect_install_path(&path)?;
        parents.extend(relative.parent().unwrap().ancestors().map(|p|root.join(p)));
    }
    parents.sort();parents.dedup();for directory in parents {crate::service::protect_install_path(&directory)?;}
    Ok(())
}

fn private_profile(path: &Path, sid: &str) -> Result<()> {
    use windows_sys::Win32::{Foundation::*,Security::{*,Authorization::*},Storage::FileSystem::*};
    crate::service::validate_sid(sid)?;normal_directory(path)?;
    std::fs::create_dir_all(path).map_err(|e|Error::io("profile-directory",e))?;
    // No delete sharing: the checked directory cannot be replaced while its
    // ownership and private ACL are inspected/applied through this same handle.
    let handle=crate::service::Handle(unsafe{CreateFileW(crate::service::wide(&path.to_string_lossy()).as_ptr(),READ_CONTROL|WRITE_DAC,FILE_SHARE_READ|FILE_SHARE_WRITE,std::ptr::null(),OPEN_EXISTING,FILE_FLAG_BACKUP_SEMANTICS|FILE_FLAG_OPEN_REPARSE_POINT,std::ptr::null_mut())});
    if handle.0==INVALID_HANDLE_VALUE{return Err(Error::io("profile-security",std::io::Error::last_os_error()));}
    let mut info:BY_HANDLE_FILE_INFORMATION=unsafe{std::mem::zeroed()};
    if unsafe{GetFileInformationByHandle(handle.0,&mut info)}==0{return Err(Error::io("profile-security",std::io::Error::last_os_error()));}
    if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT!=0{return Err(Error::new("UnsafeDataDirectory","profile-security","Папка профиля не должна быть ссылкой."));}
    let (mut owner,mut previous)=(std::ptr::null_mut(),std::ptr::null_mut());
    let code=unsafe{GetSecurityInfo(handle.0,SE_FILE_OBJECT,OWNER_SECURITY_INFORMATION,&mut owner,std::ptr::null_mut(),std::ptr::null_mut(),std::ptr::null_mut(),&mut previous)};
    if code!=ERROR_SUCCESS{return Err(Error::io("profile-security",std::io::Error::from_raw_os_error(code as i32)));}
    let allowed=crate::install_paths::sid_text(owner).is_some_and(|value|profile_owner_allowed(&value,sid));unsafe{LocalFree(previous as _);}
    if !allowed{return Err(Error::new("ProfileOwnerMismatch","profile-security","Владелец папки профиля отличается от текущего пользователя. Используйте свою папку профиля; чужие данные не изменены."));}
    let sd=crate::service::security_descriptor(&format!("D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FA;;;{sid})"))?;
    let (mut dacl,mut present,mut defaulted)=(std::ptr::null_mut(),0,0);
    if unsafe{GetSecurityDescriptorDacl(sd.pointer(),&mut present,&mut dacl,&mut defaulted)}==0 || present==0{return Err(Error::io("profile-security",std::io::Error::last_os_error()));}
    let code=unsafe{SetSecurityInfo(handle.0,SE_FILE_OBJECT,DACL_SECURITY_INFORMATION|PROTECTED_DACL_SECURITY_INFORMATION,std::ptr::null_mut(),std::ptr::null_mut(),dacl,std::ptr::null_mut())};
    if code!=ERROR_SUCCESS{return Err(Error::io("profile-security",std::io::Error::from_raw_os_error(code as i32)));}Ok(())
}
fn profile_owner_allowed(owner:&str,sid:&str)->bool {owner==sid || matches!(owner,"S-1-5-18"|"S-1-5-32-544")}
/// Icons must never contact SMB shares or follow reparse points from imported
/// application paths. Routing can still store a process name without reading it.
pub fn local_executable(path:&Path)->bool {
    use std::os::windows::fs::MetadataExt;
    let text=path.to_string_lossy();let bytes=text.as_bytes();
    bytes.len()>3 && bytes[0].is_ascii_alphabetic() && bytes[1]==b':' && matches!(bytes[2],b'\\'|b'/')
        && !text[2..].contains(':') && path.extension().is_some_and(|v|v.eq_ignore_ascii_case("exe"))
        && path.parent().is_some_and(|p|normal_directory(p).is_ok())
        && path.symlink_metadata().is_ok_and(|m|m.is_file()&&m.file_attributes() & 0x400==0)
}

fn hash(path: &Path) -> Result<String> {
    use std::io::Read;
    let mut file=std::fs::File::open(path).map_err(|e|Error::io("profile-migrate",e))?;
    let mut digest=Sha256::new();let mut block=[0;65536];
    loop {let n=file.read(&mut block).map_err(|e|Error::io("profile-migrate",e))?;if n==0 {break;}digest.update(&block[..n]);}
    Ok(digest.finalize().iter().map(|b|format!("{b:02x}")).collect())
}

fn copy_profile(source: &Path, destination: &Path, files: &mut Vec<(PathBuf,String)>, relative: &Path) -> Result<()> {
    use std::os::windows::fs::MetadataExt;
    normal_directory(source)?;
    for entry in std::fs::read_dir(source).map_err(|e|Error::io("profile-migrate",e))? {
        let entry=entry.map_err(|e|Error::io("profile-migrate",e))?;
        if entry.file_name()=="client.lock" || entry.file_name()=="migrated-to.txt" {continue;}
        let metadata=entry.path().symlink_metadata().map_err(|e|Error::io("profile-migrate",e))?;
        if metadata.file_attributes() & 0x400!=0 {return Err(Error::new("UnsafeDataDirectory","profile-migrate","Перенос профиля остановлен: найдена ссылка. Исходные данные сохранены."));}
        let child=relative.join(entry.file_name());let target=destination.join(entry.file_name());
        if metadata.is_dir() {
            std::fs::create_dir(&target).map_err(|e|Error::io("profile-migrate",e))?;
            copy_profile(&entry.path(),&target,files,&child)?;
        } else if metadata.is_file() {
            let before=hash(&entry.path())?;
            std::fs::copy(entry.path(),&target).map_err(|e|Error::io("profile-migrate",e))?;
            if before!=hash(&target)? || before!=hash(&entry.path())? {return Err(Error::new("ProfileChanged","profile-migrate","Профиль изменился во время переноса. Закройте прежний NetPulse и повторите запуск."));}
            files.push((child,before));
        }
    }
    Ok(())
}

/// The GUI holds the legacy QLockFile throughout migration and this session.
/// Commit the verified copy before removing any source file. A failed copy leaves
/// the old profile intact; retry never overlays an existing destination profile.
pub fn resolve_profile(root: &Path, legacy: Option<&Path>) -> Result<PathBuf> {
    use std::os::windows::fs::MetadataExt;
    let sid=crate::service::current_sid()?;
    let parent=root.join("data");normal_directory(&parent)?;
    std::fs::create_dir_all(&parent).map_err(|e|Error::io("profile-directory",e))?;
    let target=parent.join(&sid);normal_directory(&target)?;
    if target.exists() {private_profile(&target,&sid)?;return Ok(target);}
    let Some(legacy)=legacy.filter(|p|p.join("netpulse.db").is_file()) else {private_profile(&target,&sid)?;return Ok(target);};
    normal_directory(legacy)?;
    let stage=parent.join(format!("migration-{}",uuid::Uuid::new_v4()));private_profile(&stage,&sid)?;
    let mut files=Vec::new();
    // Incomplete migration directories are private and retained if verification
    // fails; no source files have been removed and no profile is activated.
    copy_profile(legacy,&stage,&mut files,Path::new(""))?;
    let db=rusqlite::Connection::open_with_flags(stage.join("netpulse.db"),rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e|Error::new("ProfileDatabaseInvalid","profile-migrate",e.to_string()))?;
    let valid:String=db.query_row("PRAGMA quick_check",[],|r|r.get(0)).map_err(|e|Error::new("ProfileDatabaseInvalid","profile-migrate",e.to_string()))?;
    if valid!="ok" {return Err(Error::new("ProfileDatabaseInvalid","profile-migrate","Проверка копии базы не пройдена. Исходный профиль сохранён."));}drop(db);
    let receipt=serde_json::json!({"source":legacy,"files":files,"verified":true});
    std::fs::write(stage.join("migration.json"),serde_json::to_vec_pretty(&receipt).unwrap()).map_err(|e|Error::io("profile-migrate",e))?;
    std::fs::rename(&stage,&target).map_err(|e|Error::io("profile-migrate-commit",e))?;
    // Unelevated, exact copied files only. Changed or inaccessible source files
    // are preserved, as are unknown files and the legacy client lock.
    for (relative,digest) in &files {
        let source=legacy.join(relative);
        if normal_directory(source.parent().unwrap()).is_ok() && std::fs::symlink_metadata(&source).is_ok_and(|m|m.file_attributes() & 0x400==0) && hash(&source).is_ok_and(|v|v==*digest) {let _=std::fs::remove_file(source);}
    }
    let _=std::fs::write(legacy.join("migrated-to.txt"),target.to_string_lossy().as_bytes());
    Ok(target)
}

/// Runs in the normal user's first launch, never in the elevated installer.
/// Only payload names from the new installation manifest are removed; unrelated
/// files and directories in the former per-user application are preserved.
pub fn cleanup_legacy_application(root: &Path, programs: &Path) -> Result<usize> {
    use std::os::windows::fs::MetadataExt;
    let manifest=std::fs::read_to_string(root.join("installation-files.txt")).map_err(|e|Error::io("legacy-files",e))?;
    let names=manifest.lines().map(PathBuf::from).collect::<Vec<_>>();
    if names.iter().any(|p|p.is_absolute() || p.components().any(|c|!matches!(c,std::path::Component::Normal(_)))) {return Err(Error::new("InstallManifestInvalid","legacy-files","Invalid installation file manifest"));}
    let mut removed=0;
    for name in ["NetPulse","NetPulse Qt"] {
        let old=programs.join(name);
        if old==root || !old.join("NetPulse.exe").is_file() || normal_directory(&old).is_err() {continue;}
        for relative in &names {
            let path=old.join(relative);
            if normal_directory(path.parent().unwrap()).is_err() {continue;}
            if path.symlink_metadata().is_ok_and(|m|m.is_file() && m.file_attributes() & 0x400==0) && std::fs::remove_file(&path).is_ok() {removed+=1;}
        }
        let mut parents=names.iter().filter_map(|p|p.parent()).flat_map(|p|p.ancestors()).map(|p|old.join(p)).collect::<Vec<_>>();
        parents.sort_by_key(|p|std::cmp::Reverse(p.components().count()));parents.dedup();
        for directory in parents {if directory.starts_with(&old) && normal_directory(&directory).is_ok(){let _=std::fs::remove_dir(directory);}}
        let _=std::fs::remove_dir(&old);
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn foreign_profile_owners_and_remote_icon_paths_are_rejected() {
        let sid="S-1-5-21-1-2-3-1001";
        assert!(profile_owner_allowed(sid,sid));assert!(profile_owner_allowed("S-1-5-18",sid));assert!(profile_owner_allowed("S-1-5-32-544",sid));
        assert!(!profile_owner_allowed("S-1-5-21-1-2-3-1002",sid));assert!(!profile_owner_allowed("S-1-1-0",sid));
        for path in [r"\\server\share\app.exe",r"\\?\UNC\server\share\app.exe",r"C:\app.exe:stream.exe",r"relative.exe"]{assert!(!local_executable(Path::new(path)));}
        for path in [r"\\server\share",r"\\?\UNC\server\share",r"relative"]{assert!(local_directory(Path::new(path)).is_err());}
    }
    #[test] fn installer_manifest_cannot_protect_or_touch_files_outside_payload() {
        let d=tempfile::tempdir().unwrap();let app=d.path().join("app");std::fs::create_dir(&app).unwrap();let outside=d.path().join("outside.txt");std::fs::write(&outside,b"preserve").unwrap();
        for name in ["../outside.txt","data/settings.json",r"C:\Windows\system.ini"] {
            std::fs::write(app.join("installation-files.txt"),name).unwrap();assert_eq!(finalize_install_root(&app).unwrap_err().code,"InstallManifestInvalid");
        }
        assert_eq!(std::fs::read(outside).unwrap(),b"preserve");
    }
    #[test] fn encrypted_profiles_remain_readable_after_migration() {
        let d=tempfile::tempdir().unwrap();let old=d.path().join("old");std::fs::create_dir(&old).unwrap();
        let mut store=crate::store::Store::open(&old.join("netpulse.db")).unwrap();let parsed=crate::imports::parse("socks5://alice:fixture-secret@127.0.0.1:1080#Saved").unwrap();store.import(parsed,None).unwrap();let id=store.profiles().unwrap()[0].id.clone();drop(store);
        let new=resolve_profile(&d.path().join("app"),Some(&old)).unwrap();let store=crate::store::Store::open(&new.join("netpulse.db")).unwrap();assert!(serde_json::to_string(&store.profile(&id).unwrap()).unwrap().contains("fixture-secret"));
    }
    #[test] fn legacy_application_cleanup_removes_only_manifest_names() {
        let d=tempfile::tempdir().unwrap();let root=d.path().join("new");let old=d.path().join("Programs/NetPulse");std::fs::create_dir_all(&root).unwrap();std::fs::create_dir_all(&old).unwrap();
        std::fs::write(root.join("installation-files.txt"),"NetPulse.exe\nnetpulsed.exe\n").unwrap();for name in ["NetPulse.exe","netpulsed.exe","personal.txt"]{std::fs::write(old.join(name),b"fixture").unwrap();}
        assert_eq!(cleanup_legacy_application(&root,&d.path().join("Programs")).unwrap(),2);assert!(old.join("personal.txt").exists());
    }
    #[test] fn shared_parent_allows_profile_creation_but_profiles_have_private_acls() {
        use windows_sys::Win32::{Foundation::*,Security::{*,Authorization::*}};
        let d=tempfile::tempdir().unwrap();let parent=d.path().join("data");std::fs::create_dir(&parent).unwrap();
        permissions(&parent,"D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;;FRFX;;;BU)(A;;0x00000004;;;BU)(A;OICIIO;FA;;;CO)").unwrap();
        let profile=parent.join(crate::service::current_sid().unwrap());private_profile(&profile,&crate::service::current_sid().unwrap()).unwrap();
        let mut sd=std::ptr::null_mut();let name=crate::service::wide(&profile.to_string_lossy());
        let code=unsafe{GetNamedSecurityInfoW(name.as_ptr(),SE_FILE_OBJECT,DACL_SECURITY_INFORMATION,std::ptr::null_mut(),std::ptr::null_mut(),std::ptr::null_mut(),std::ptr::null_mut(),&mut sd)};assert_eq!(code,ERROR_SUCCESS);
        let mut text=std::ptr::null_mut();assert_ne!(unsafe{ConvertSecurityDescriptorToStringSecurityDescriptorW(sd,1,DACL_SECURITY_INFORMATION,&mut text,std::ptr::null_mut())},0);
        let mut n=0;unsafe{while *text.add(n)!=0 {n+=1;}}let sddl=String::from_utf16_lossy(unsafe{std::slice::from_raw_parts(text,n)});
        unsafe{LocalFree(text as _);LocalFree(sd as _);}
        assert!(sddl.contains(&crate::service::current_sid().unwrap()));assert!(!sddl.contains(";;;BU)"));std::fs::write(profile.join("settings.json"),b"{}").unwrap();
    }
    #[test] fn profile_migration_verifies_database_and_retains_modified_or_unknown_files() {
        let d=tempfile::tempdir().unwrap();let root=d.path().join("NetPulse");let old=d.path().join("old");std::fs::create_dir_all(&old).unwrap();
        let db=rusqlite::Connection::open(old.join("netpulse.db")).unwrap();db.execute("CREATE TABLE fixture(value TEXT)",[]).unwrap();db.execute("INSERT INTO fixture VALUES ('saved')",[]).unwrap();drop(db);
        std::fs::write(old.join("vault.bin"),b"opaque user data").unwrap();std::fs::write(old.join("client.lock"),b"held").unwrap();
        let new=resolve_profile(&root,Some(&old)).unwrap();assert!(new.starts_with(root.join("data")));assert_eq!(std::fs::read(new.join("vault.bin")).unwrap(),b"opaque user data");
        assert!(!old.join("vault.bin").exists());assert!(old.join("client.lock").exists());assert!(old.join("migrated-to.txt").exists());
        std::fs::write(old.join("new-file.txt"),b"keep").unwrap();assert_eq!(resolve_profile(&root,Some(&old)).unwrap(),new);assert!(old.join("new-file.txt").exists());
    }
    #[test] fn failed_migration_never_removes_source_or_activates_invalid_database() {
        let d=tempfile::tempdir().unwrap();let root=d.path().join("app");let old=d.path().join("old");std::fs::create_dir(&old).unwrap();std::fs::write(old.join("netpulse.db"),b"invalid").unwrap();
        assert!(resolve_profile(&root,Some(&old)).is_err());assert!(old.join("netpulse.db").exists());assert!(!root.join("data").join(crate::service::current_sid().unwrap()).exists());
    }
    #[test] fn reparse_profile_is_rejected_without_following_it() {
        let d=tempfile::tempdir().unwrap();let other=tempfile::tempdir().unwrap();let link=d.path().join("data");
        let result=std::process::Command::new("cmd.exe").args(["/c","mklink","/J"]).arg(&link).arg(other.path()).creation_flags(0x08000000).output().unwrap();assert!(result.status.success());
        assert!(resolve_profile(d.path(),None).is_err());std::fs::remove_dir(link).unwrap();
    }
    use std::os::windows::process::CommandExt;
}
