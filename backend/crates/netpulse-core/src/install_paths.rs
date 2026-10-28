//! Machine-wide installation identity and selectable, protected local roots.
use crate::{Error,Result};
use std::path::{Path,PathBuf};
const KEY:&str=r"Software\NetPulseQt";
pub fn registered_root()->Option<PathBuf> {
    use winreg::{RegKey,enums::*};
    RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey_with_flags(KEY,KEY_READ|KEY_WOW64_64KEY).ok()?.get_value::<String,_>("InstallDir").ok().map(PathBuf::from)
}
pub fn previous_roots()->Vec<PathBuf> {
    use winreg::{RegKey,enums::*};
    RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey_with_flags(KEY,KEY_READ|KEY_WOW64_64KEY).ok().and_then(|k|k.get_value::<Vec<String>,_>("PreviousInstallDirs").ok()).unwrap_or_default().into_iter().map(PathBuf::from).collect()
}
fn same(a:&Path,b:&Path)->bool {a.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy())}
fn registered(root:&Path)->bool {registered_root().into_iter().chain(previous_roots()).any(|p|same(&p,root))}
fn pending(root:&Path)->bool {
    use winreg::{RegKey,enums::*};
    RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey_with_flags(KEY,KEY_READ|KEY_WOW64_64KEY).ok().and_then(|key|key.get_value::<String,_>("PendingInstallDir").ok()).is_some_and(|p|same(Path::new(&p),root))
}
fn retained_profiles_only(root:&Path)->bool {
    use std::os::windows::fs::MetadataExt;
    let Ok(entries)=std::fs::read_dir(root) else{return false;};let entries=entries.collect::<std::io::Result<Vec<_>>>();
    entries.is_ok_and(|entries|entries.len()==1&&entries[0].file_name().to_string_lossy().eq_ignore_ascii_case("data")&&entries[0].path().symlink_metadata().is_ok_and(|m|m.is_dir()&&m.file_attributes()&0x400==0))
}
pub(crate) fn record_pending_root(root:&Path)->Result<()> {
    use winreg::{RegKey,enums::*};
    let (key,_)=RegKey::predef(HKEY_LOCAL_MACHINE).create_subkey_with_flags(KEY,KEY_READ|KEY_WRITE|KEY_WOW64_64KEY).map_err(|e|Error::io("install-location",e))?;
    key.set_value("PendingInstallDir",&root.to_string_lossy().into_owned()).map_err(|e|Error::io("install-location",e))
}
pub fn record_root(root:&Path)->Result<()> {
    use winreg::{RegKey,enums::*};
    let mut previous=previous_roots();
    if let Some(old)=registered_root().filter(|p|!same(p,root)){previous.retain(|p|!same(p,&old));previous.insert(0,old);}
    previous.retain(|p|!same(p,root));
    let (key,_)=RegKey::predef(HKEY_LOCAL_MACHINE).create_subkey_with_flags(KEY,KEY_READ|KEY_WRITE|KEY_WOW64_64KEY).map_err(|e|Error::io("install-location",e))?;
    key.set_value("PreviousInstallDirs",&previous.iter().map(|p|p.to_string_lossy().into_owned()).collect::<Vec<_>>()).map_err(|e|Error::io("install-location",e))?;
    key.set_value("InstallDir",&root.to_string_lossy().into_owned()).map_err(|e|Error::io("install-location",e))?;
    if pending(root){let _=key.delete_value("PendingInstallDir");}Ok(())
}
pub fn profile_sources(root:&Path)->Result<Vec<PathBuf>> {
    let sid=crate::service::current_sid()?;
    let mut sources=previous_roots().into_iter().filter(|p|!same(p,root)).map(|p|p.join("data").join(&sid)).collect::<Vec<_>>();
    if let Some(local)=std::env::var_os("LOCALAPPDATA"){sources.push(PathBuf::from(local).join("NetPulseQt"));}
    Ok(sources)
}
fn invalid()->Error {Error::new("UnsafeInstallDirectory","install-location","Выберите отдельную папку на локальном диске, например D:\\NetPulse. Родительская папка должна быть защищена от подмены без прав администратора.")}
pub fn validate_root(root:&Path)->Result<()> {
    validate_location(root,false)
}
/// A new root is not registered until finalization succeeds. It must already
/// have the protected owner/ACL established before any payload was extracted.
pub fn validate_prepared_root(root:&Path)->Result<()> {validate_location(root,true)}
fn validate_location(root:&Path,prepared:bool)->Result<()> {
    use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;
    if !root.is_absolute() || root.components().count()<3 || root.components().any(|c|matches!(c,std::path::Component::ParentDir|std::path::Component::CurDir)){return Err(invalid());}
    let text=root.to_string_lossy();let bytes=text.as_bytes();
    if bytes.len()<3 || bytes[1]!=b':' || !bytes[0].is_ascii_alphabetic(){return Err(invalid());}
    if root.components().filter_map(|c|if let std::path::Component::Normal(p)=c{Some(p.to_string_lossy())}else{None}).any(|p|p.ends_with([' ','.'])||p.chars().any(|c|c.is_control()||"<>:\"|?*".contains(c))){return Err(invalid());}
    if unsafe{GetDriveTypeW(crate::service::wide(&format!("{}:\\",bytes[0] as char)).as_ptr())}!=3{return Err(invalid());} // DRIVE_FIXED
    if let Some(windows)=std::env::var_os("WINDIR"){if text.to_lowercase().starts_with(&PathBuf::from(windows).to_string_lossy().to_lowercase()){return Err(invalid());}}
    for env in ["ProgramFiles","ProgramFiles(x86)","USERPROFILE","PUBLIC"] {if let Some(p)=std::env::var_os(env){if same(root,&PathBuf::from(p)){return Err(invalid());}}}
    crate::layout::normal_directory(root)?;
    for parent in root.parent().ok_or_else(invalid)?.ancestors().filter(|p|p.exists()){trusted_parent(parent)?;}
    if root.exists() && std::fs::read_dir(root).map_err(|e|Error::io("install-location",e))?.next().is_some(){
        let pending=pending(root);
        if !registered(root) && !prepared && !pending && !retained_profiles_only(root){return Err(Error::new("InstallDirectoryNotEmpty","install-location","Выберите пустую папку или зарегистрированную папку NetPulse. Посторонние файлы сохранены."));}
        validate_payload_tree(root,true)?;
    }
    Ok(())
}
pub(crate) fn sid_text(sid:windows_sys::Win32::Security::PSID)->Option<String> {
    use windows_sys::Win32::{Foundation::LocalFree,Security::Authorization::ConvertSidToStringSidW};
    let mut text=std::ptr::null_mut();if sid.is_null() || unsafe{ConvertSidToStringSidW(sid,&mut text)}==0{return None;}
    let mut n=0;unsafe{while *text.add(n)!=0{n+=1;}}let value=String::from_utf16_lossy(unsafe{std::slice::from_raw_parts(text,n)});unsafe{LocalFree(text as _);}
    Some(value)
}
fn trusted_sid(sid:windows_sys::Win32::Security::PSID)->bool {sid_text(sid).is_some_and(|v|matches!(v.as_str(),"S-1-5-18"|"S-1-5-32-544"|"S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464"))}
fn trusted_parent(path:&Path)->Result<()> {
    trusted_acl(path,false)
}
fn trusted_acl(path:&Path,payload:bool)->Result<()> {
    use windows_sys::Win32::{Foundation::*,Security::{*,Authorization::*}};
    let (mut owner,mut dacl,mut sd)=(std::ptr::null_mut(),std::ptr::null_mut(),std::ptr::null_mut());
    let code=unsafe{GetNamedSecurityInfoW(crate::service::wide(&path.to_string_lossy()).as_ptr(),SE_FILE_OBJECT,OWNER_SECURITY_INFORMATION|DACL_SECURITY_INFORMATION,&mut owner,std::ptr::null_mut(),&mut dacl,std::ptr::null_mut(),&mut sd)};
    if code!=ERROR_SUCCESS{return Err(Error::io("install-parent-security",std::io::Error::from_raw_os_error(code as i32)));}
    let result=validate_security(owner,dacl,payload);unsafe{LocalFree(sd as _);}result
}
fn validate_security(owner:windows_sys::Win32::Security::PSID,dacl:*mut windows_sys::Win32::Security::ACL,payload:bool)->Result<()> {
    use windows_sys::Win32::{Foundation::*,Security::*};
        if !trusted_sid(owner) || dacl.is_null(){return Err(invalid());}
        let mut info:ACL_SIZE_INFORMATION=unsafe{std::mem::zeroed()};
        if unsafe{GetAclInformation(dacl,&mut info as *mut _ as _,std::mem::size_of_val(&info) as u32,AclSizeInformation)}==0{return Err(invalid());}
        for index in 0..info.AceCount {
            let mut ace=std::ptr::null_mut();if unsafe{GetAce(dacl,index,&mut ace)}==0{return Err(invalid());}
            let header=unsafe{&*(ace as *const ACE_HEADER)};if header.AceFlags & INHERIT_ONLY_ACE as u8!=0{continue;}
            if header.AceType==0 {
                let allowed=unsafe{&*(ace as *const ACCESS_ALLOWED_ACE)};
                // FILE_DELETE_CHILD | DELETE | WRITE_DAC | WRITE_OWNER.
                let forbidden=0x000d0040|GENERIC_ALL|if payload {GENERIC_WRITE|0x00000116}else{0};
                if allowed.Mask & forbidden!=0 && !trusted_sid(std::ptr::addr_of!(allowed.SidStart) as _){return Err(invalid());}
            } else if matches!(header.AceType,5|9|11) {return Err(invalid());}
        }
        Ok(())
}
/// Inspect the protected payload before the elevated installer overwrites it.
/// User profiles are deliberately writable and are never traversed here.
fn validate_payload_tree(path:&Path,root:bool)->Result<()> {
    use std::os::windows::fs::MetadataExt;
    let metadata=path.symlink_metadata().map_err(|e|Error::io("install-security",e))?;
    if metadata.file_attributes() & 0x400!=0 {return Err(invalid());}
    trusted_acl(path,true)?;
    if metadata.is_dir(){for entry in std::fs::read_dir(path).map_err(|e|Error::io("install-security",e))?{
        let entry=entry.map_err(|e|Error::io("install-security",e))?;
        if root && entry.file_name().to_string_lossy().eq_ignore_ascii_case("data") {crate::layout::normal_directory(&entry.path())?;continue;}
        validate_payload_tree(&entry.path(),false)?;
    }}
    Ok(())
}
/// Create a directory with its final owner/DACL in the creation call. Never
/// adopt a competing directory or expose a newly created directory as writable.
pub fn create_protected_directory(path:&Path)->Result<()> {
    use windows_sys::Win32::{Storage::FileSystem::CreateDirectoryW,Security::SECURITY_ATTRIBUTES};
    let sd=crate::service::security_descriptor("O:BAG:BAD:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FRFX;;;BU)")?;
    let attrs=SECURITY_ATTRIBUTES{nLength:std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,lpSecurityDescriptor:sd.pointer(),bInheritHandle:0};
    if unsafe{CreateDirectoryW(crate::service::wide(&path.to_string_lossy()).as_ptr(),&attrs)}==0{return Err(Error::io("install-directory",std::io::Error::last_os_error()));}
    Ok(())
}
pub fn cleanup_previous_payload(root:&Path)->Result<usize> {
    use std::os::windows::fs::MetadataExt;
    let manifest=std::fs::read_to_string(root.join("installation-files.txt")).map_err(|e|Error::io("previous-installation",e))?;
    let names=manifest.lines().map(PathBuf::from).collect::<Vec<_>>();
    if names.iter().any(|p|p.is_absolute()||p.components().any(|c|!matches!(c,std::path::Component::Normal(_)))||p.components().next().is_some_and(|c|c.as_os_str().to_string_lossy().eq_ignore_ascii_case("data"))){return Err(invalid());}
    let mut removed=0;
    for old in previous_roots().into_iter().filter(|p|!same(p,root)) {
        if !old.join("installation-files.txt").is_file()||validate_root(&old).is_err(){continue;}
        for name in &names {
            let file=old.join(name);
            if crate::layout::normal_directory(file.parent().unwrap()).is_ok() && file.symlink_metadata().is_ok_and(|m|m.is_file()&&m.file_attributes() & 0x400==0) && std::fs::remove_file(file).is_ok(){removed+=1;}
        }
        let mut dirs=names.iter().filter_map(|p|p.parent()).flat_map(|p|p.ancestors()).map(|p|old.join(p)).collect::<Vec<_>>();
        dirs.sort_by_key(|p|std::cmp::Reverse(p.components().count()));dirs.dedup();
        for dir in dirs {if crate::layout::normal_directory(&dir).is_ok(){let _=std::fs::remove_dir(dir);}}
    }
    Ok(removed)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn protected_payload_rejects_foreign_owners_and_writable_acls() {
        use windows_sys::Win32::Security::*;
        for (sddl,valid) in [
            ("O:BAD:P(A;;FA;;;BA)(A;;FRFX;;;BU)",true),
            ("O:BAD:P(A;;FA;;;BA)(A;;FW;;;BU)",false),
            ("O:BAD:P(A;;FA;;;BA)(A;;0x40;;;BU)",false),
            ("O:BAD:P(A;;FA;;;BA)(A;;GA;;;WD)",false),
            ("O:S-1-5-21-1-2-3-1001D:P(A;;FA;;;BA)(A;;FRFX;;;BU)",false),
        ] {
            let sd=crate::service::security_descriptor(sddl).unwrap();
            let (mut owner,mut defaulted,mut present,mut dacl)=(std::ptr::null_mut(),0,0,std::ptr::null_mut());
            assert_ne!(unsafe{GetSecurityDescriptorOwner(sd.pointer(),&mut owner,&mut defaulted)},0);
            assert_ne!(unsafe{GetSecurityDescriptorDacl(sd.pointer(),&mut present,&mut dacl,&mut defaulted)},0);
            assert_eq!(validate_security(owner,dacl,true).is_ok(),valid,"{sddl}");
        }
    }
    #[test] fn existing_payload_junction_is_rejected_before_target_changes() {
        use std::os::windows::process::CommandExt;
        let d=tempfile::tempdir().unwrap();let target=d.path().join("target");std::fs::create_dir(&target).unwrap();
        let sentinel=target.join("keep.txt");std::fs::write(&sentinel,b"unchanged").unwrap();let link=d.path().join("payload");
        let made=std::process::Command::new("cmd.exe").args(["/c","mklink","/J"]).arg(&link).arg(&target).creation_flags(0x08000000).output().unwrap();assert!(made.status.success());
        assert!(validate_payload_tree(&link,false).is_err());assert_eq!(std::fs::read(sentinel).unwrap(),b"unchanged");std::fs::remove_dir(link).unwrap();
    }
    #[test] fn selectable_installation_checks_default_parent_without_creating_files() {
        let root=crate::service::default_application_directory().unwrap().with_file_name(format!("NetPulse-Location-Test-{}",uuid::Uuid::new_v4()));
        assert!(!root.exists());validate_root(&root).unwrap();assert!(!root.exists());
        let d=tempfile::tempdir().unwrap();std::fs::create_dir(d.path().join("data")).unwrap();assert!(retained_profiles_only(d.path()));
        std::fs::write(d.path().join("unknown.txt"),b"preserve").unwrap();assert!(!retained_profiles_only(d.path()));
    }
    #[test] fn root_and_windows_and_parent_traversal_cannot_be_installation_targets() {
        for path in [r"C:\",r"C:\Windows\NetPulse",r"C:\Program Files\..\Windows\NetPulse",r"relative\NetPulse",r"C:\NetPulse.",r"C:\NetPulse ",r"C:\NetPulse:stream"]{assert!(validate_root(Path::new(path)).is_err());}
    }
}
