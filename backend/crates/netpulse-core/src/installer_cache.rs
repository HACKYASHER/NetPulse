//! Preserve the running installer without inheriting a download folder's ACL.
use crate::{Error,Result};
use std::{io::{Read,Write},path::Path};
pub fn save(root:&Path,source:&Path)->Result<()> {
    crate::install_paths::validate_prepared_root(root)?;
    let pending=root.join(format!("maintenance-{}.tmp",uuid::Uuid::new_v4()));
    let result=(|| {
        use std::os::windows::fs::OpenOptionsExt;
        let mut input=std::fs::OpenOptions::new().read(true).share_mode(1).open(source).map_err(|e|Error::io("installer-cache",e))?;
        let size=input.metadata().map_err(|e|Error::io("installer-cache",e))?.len();
        if !(1024..=256*1024*1024).contains(&size){return Err(Error::new("InstallerCacheInvalid","installer-cache","Invalid installer size"));}
        let mut header=[0;2];input.read_exact(&mut header).map_err(|e|Error::io("installer-cache",e))?;
        if header!=*b"MZ"{return Err(Error::new("InstallerCacheInvalid","installer-cache","Invalid installer header"));}
        // CreateNew inherits the protected root. CopyFile can copy source file
        // security on some Windows versions, so copy bytes into our own file.
        let mut output=std::fs::OpenOptions::new().create_new(true).write(true).share_mode(0).open(&pending).map_err(|e|Error::io("installer-cache",e))?;
        output.write_all(&header).map_err(|e|Error::io("installer-cache",e))?;
        let copied=std::io::copy(&mut input.take(size-1),&mut output).map_err(|e|Error::io("installer-cache",e))?;
        output.flush().and_then(|_|output.sync_all()).map_err(|e|Error::io("installer-cache",e))?;drop(output);
        if copied!=size-2{return Err(Error::new("InstallerCacheInvalid","installer-cache","Installer changed while copying"));}
        crate::service::protect_install_path(&pending)?;
        let destination=root.join("NetPulse-Maintenance.exe");
        use windows_sys::Win32::Storage::FileSystem::*;
        if unsafe{MoveFileExW(crate::service::wide(&pending.to_string_lossy()).as_ptr(),crate::service::wide(&destination.to_string_lossy()).as_ptr(),MOVEFILE_REPLACE_EXISTING|MOVEFILE_WRITE_THROUGH)}==0{return Err(Error::io("installer-cache",std::io::Error::last_os_error()));}Ok(())
    })();let _=std::fs::remove_file(pending);result
}
