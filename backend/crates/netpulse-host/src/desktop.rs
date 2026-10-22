use netpulse_core::{Error, Result};
pub fn autostart(enabled: bool) -> Result<()> {
    use winreg::{
        RegKey,
        enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE},
    };
    let root = RegKey::predef(HKEY_CURRENT_USER);
    let key = root
        .open_subkey_with_flags(
            r"Software\Microsoft\Windows\CurrentVersion\Run",
            KEY_READ | KEY_WRITE,
        )
        .map_err(|e| Error::io("autostart", e))?;
    if enabled {
        let path = std::env::current_exe().map_err(|e| Error::io("autostart", e))?.with_file_name("NetPulse.exe");
        key.set_value(
            "NetPulseQt",
            &format!("\"{}\" --background", path.to_string_lossy()),
        )
        .map_err(|e| Error::io("autostart", e))?;
    } else {
        match key.delete_value("NetPulseQt") {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(Error::io("autostart", e)),
        }
    }
    Ok(())
}
