//! The elevated updater accepts only an official release tag. It never executes
//! a user-writable cached installer or reads a caller-selected privileged path.
use netpulse_core::{Error,Result,service,client_updates,install_paths};

pub fn install(tag:&str)->Result<()> {
    let root=service::application_directory()?;
    install_paths::validate_root(&root)?;
    let own=std::env::current_exe().map_err(|e|Error::io("client-update-helper",e))?;
    if own.canonicalize().map_err(|e|Error::io("client-update-helper",e))?!=root.join("NetPulseService.exe").canonicalize().map_err(|e|Error::io("client-update-helper",e))?{return Err(Error::new("UpdateHelperLocation","client-update","Run the installed NetPulse update helper"));}
    let release=client_updates::release_for_tag(tag,env!("CARGO_PKG_VERSION"))?;
    let stage=root.join(format!("client-update-{}",uuid::Uuid::new_v4()));
    install_paths::create_protected_directory(&stage)?;
    let result=(|| {
        // Re-download into an administrator-owned directory, closing the
        // verify/elevate/use race in the ordinary user's download cache.
        client_updates::download(&stage,&release)?;
        let verified=client_updates::verify_download(&stage,&release)?;
        service::protect_install_path(&verified)?;
        let maintenance=root.join("NetPulse-Maintenance.exe");
        use windows_sys::Win32::Storage::FileSystem::*;
        if unsafe{MoveFileExW(service::wide(&verified.to_string_lossy()).as_ptr(),service::wide(&maintenance.to_string_lossy()).as_ptr(),MOVEFILE_REPLACE_EXISTING|MOVEFILE_WRITE_THROUGH)}==0{return Err(Error::io("client-update-activate",std::io::Error::last_os_error()));}
        service::protect_install_path(&maintenance)?;
        use std::os::windows::process::CommandExt;
        std::process::Command::new(&maintenance).arg("/WAITFORCLIENT").current_dir(&root).creation_flags(0x08000000).spawn().map_err(|e|Error::io("client-update-install",e))?;
        Ok(())
    })();
    // Exact files/directories created by this invocation; never recurse into
    // unknown files, profiles or another update transaction.
    let tag_dir=stage.join("updates").join(tag);
    let _=std::fs::remove_file(tag_dir.join("NetPulse-Setup.exe"));
    for directory in [&tag_dir,&stage.join("updates"),&stage]{let _=std::fs::remove_dir(directory);}
    result
}
