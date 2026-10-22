//! Separate current-user recovery process. It never belongs to the engine job.
use crate::{
    Error, Result,
    proxy::{ProxyState, apply, read},
    vault,
};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::Duration,
};
use windows_sys::Win32::{Foundation::*, System::Threading::*};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    owner: u32,
    event: String,
    before: ProxyState,
    applied: ProxyState,
}
struct Handle(HANDLE);
unsafe impl Send for Handle {}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
pub struct Recovery {
    event: Handle,
    child: Child,
    path: PathBuf,
    finished: bool,
}
impl Recovery {
    pub fn arm(directory: &Path, before: &ProxyState, applied: &ProxyState) -> Result<Self> {
        std::fs::create_dir_all(directory).map_err(|e| Error::io("proxy-journal", e))?;
        let id = uuid::Uuid::new_v4();
        let name = format!("Local\\NetPulse.ProxyRecovery.{id}");
        let security = crate::service::security_descriptor(&format!(
            "D:P(A;;GA;;;SY)(A;;GA;;;{})",
            crate::service::current_sid()?
        ))?;
        let attrs = windows_sys::Win32::Security::SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<windows_sys::Win32::Security::SECURITY_ATTRIBUTES>()
                as u32,
            lpSecurityDescriptor: security.pointer(),
            bInheritHandle: 0,
        };
        let event = Handle(unsafe { CreateEventW(&attrs, 1, 0, wide(&name).as_ptr()) });
        if event.0.is_null() {
            return Err(Error::io(
                "proxy-recovery-event",
                std::io::Error::last_os_error(),
            ));
        }
        let journal = Journal {
            owner: std::process::id(),
            event: name,
            before: before.clone(),
            applied: applied.clone(),
        };
        let path = directory.join(format!("proxy-{id}.npj"));
        let sealed = vault::seal(&zeroize::Zeroizing::new(
            serde_json::to_vec(&journal).map_err(|_| {
                Error::new(
                    "RecoveryInvalid",
                    "proxy-journal",
                    "Could not encode recovery journal",
                )
            })?,
        ))?;
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .map_err(|e| Error::io("proxy-journal", e))?;
        file.write_all(&sealed)
            .and_then(|_| file.sync_all())
            .map_err(|e| Error::io("proxy-journal", e))?;
        drop(file);
        let mut command =
            Command::new(std::env::current_exe().map_err(|e| Error::io("proxy-recovery-exe", e))?);
        command
            .arg("--proxy-recover")
            .arg(&path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
        let child = command
            .spawn()
            .map_err(|e| Error::io("proxy-recovery-start", e));
        let mut child = match child {
            Ok(c) => c,
            Err(e) => {
                let _ = std::fs::remove_file(&path);
                return Err(e);
            }
        };
        let mut stdout = child.stdout.take().ok_or_else(|| {
            Error::new(
                "RecoveryUnavailable",
                "proxy-recovery",
                "Recovery handshake pipe is missing",
            )
        })?;
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut bytes = [0; 5];
            let result = stdout.read_exact(&mut bytes).map(|_| bytes == *b"READY");
            let _ = tx.send(result);
        });
        if !matches!(rx.recv_timeout(Duration::from_secs(4)), Ok(Ok(true)))
            || child
                .try_wait()
                .map_err(|e| Error::io("proxy-recovery-ready", e))?
                .is_some()
        {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(&path);
            return Err(Error::new(
                "RecoveryUnavailable",
                "proxy-recovery",
                "Recovery helper did not confirm ownership of the GUI process",
            ));
        }
        Ok(Self {
            event,
            child,
            path,
            finished: false,
        })
    }
    pub fn finish(&mut self) -> Result<()> {
        if self.finished {
            return Ok(());
        }
        if unsafe { SetEvent(self.event.0) } == 0 {
            return Err(Error::io(
                "proxy-recovery-stop",
                std::io::Error::last_os_error(),
            ));
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while self
            .child
            .try_wait()
            .map_err(|e| Error::io("proxy-recovery-stop", e))?
            .is_none()
        {
            if std::time::Instant::now() > deadline {
                return Err(Error::new(
                    "RecoveryStopTimeout",
                    "proxy-recovery",
                    "Recovery process did not acknowledge completion",
                ));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let status = self
            .child
            .wait()
            .map_err(|e| Error::io("proxy-recovery-result", e))?;
        if !status.success() {
            // Preserve the journal for retry/startup recovery when the worker
            // could not restore Windows settings. Process exit is not success.
            return Err(Error::new(
                "RecoveryFailed",
                "proxy-recovery-result",
                "Recovery worker failed; journal preserved",
            ));
        }
        self.finished = true;
        let _ = std::fs::remove_file(&self.path);
        Ok(())
    }
    pub fn pid(&self) -> u32 {
        self.child.id()
    }
}
// Do not terminate an armed watchdog on drop: if restoration failed, it must
// stay alive until its owning GUI exits and retry the compare-before-restore.
pub fn run(path: &Path) -> Result<()> {
    let raw = std::fs::read(path).map_err(|e| Error::io("proxy-recovery-read", e))?;
    if raw.len() > 1024 * 1024 {
        return Err(Error::new(
            "RecoveryInvalid",
            "proxy-recovery",
            "Recovery journal exceeds its limit",
        ));
    }
    let data = vault::open(&raw)?;
    let journal: Journal = serde_json::from_slice(&data).map_err(|_| {
        Error::new(
            "RecoveryInvalid",
            "proxy-recovery",
            "Invalid recovery journal",
        )
    })?;
    if !journal.event.starts_with("Local\\NetPulse.ProxyRecovery.") {
        return Err(Error::new(
            "RecoveryInvalid",
            "proxy-recovery",
            "Invalid recovery event",
        ));
    }
    let owner = Handle(unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, journal.owner) });
    let event = Handle(unsafe {
        OpenEventW(
            SYNCHRONIZATION_SYNCHRONIZE,
            0,
            wide(&journal.event).as_ptr(),
        )
    });
    if owner.0.is_null() || event.0.is_null() {
        return Err(Error::new(
            "RecoveryOwnerMissing",
            "proxy-recovery",
            "Cannot watch the owning GUI process",
        ));
    }
    std::io::stdout()
        .write_all(b"READY")
        .and_then(|_| std::io::stdout().flush())
        .map_err(|e| Error::io("proxy-recovery-ready", e))?;
    match unsafe { WaitForMultipleObjects(2, [owner.0, event.0].as_ptr(), 0, INFINITE) } {
        n if n == WAIT_OBJECT_0 || n == WAIT_OBJECT_0 + 1 => {
            // The explicit event is also an immediate recovery request. A
            // failed GUI restoration must not wait until the GUI exits.
            if read()? == journal.applied {
                apply(&journal.before)?;
            }
        }
        _ => {
            return Err(Error::io(
                "proxy-recovery-wait",
                std::io::Error::last_os_error(),
            ));
        }
    }
    let _ = std::fs::remove_file(path);
    Ok(())
}
/// A helper killed with the desktop may leave an encrypted journal. Recover only
/// when its original PID no longer exists and the proxy still equals our state.
pub fn recover_stale(directory: &Path) -> Result<usize> {
    if !directory.exists() {
        return Ok(0);
    }
    let mut recovered = 0;
    for entry in std::fs::read_dir(directory)
        .map_err(|e| Error::io("proxy-recovery-scan", e))?
        .flatten()
        .take(100)
    {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("proxy-")
            || !name.ends_with(".npj")
            || entry.file_type().is_ok_and(|t| !t.is_file())
        {
            continue;
        }
        if entry.metadata().is_ok_and(|m| m.len() > 1024 * 1024) {
            continue;
        }
        let raw = std::fs::read(&path).map_err(|e| Error::io("proxy-recovery-read", e))?;
        let Ok(data) = vault::open(&raw) else {
            continue;
        };
        let Ok(journal) = serde_json::from_slice::<Journal>(&data) else {
            continue;
        };
        let owner = Handle(unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, journal.owner) });
        let definitely_dead = if owner.0.is_null() {
            std::io::Error::last_os_error().raw_os_error() == Some(87)
        } else {
            (unsafe { WaitForSingleObject(owner.0, 0) }) == WAIT_OBJECT_0
        };
        if definitely_dead {
            if read()? == journal.applied {
                apply(&journal.before)?;
                recovered += 1;
            }
            std::fs::remove_file(path).map_err(|e| Error::io("proxy-journal-remove", e))?;
        }
    }
    Ok(recovered)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_worker_preserves_journal_instead_of_acknowledging_recovery() {
        use std::os::windows::process::CommandExt;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("owned-test.npj");
        std::fs::write(&path, b"owned fixture, no network settings").unwrap();
        let event = Handle(unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) });
        assert!(!event.0.is_null());
        let child = Command::new("cmd.exe")
            .args(["/d", "/c", "exit", "1"])
            .creation_flags(0x08000000)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut recovery = Recovery {
            event,
            child,
            path: path.clone(),
            finished: false,
        };
        assert_eq!(recovery.finish().unwrap_err().code, "RecoveryFailed");
        assert!(path.is_file());
        assert!(!recovery.finished);
        assert!(recovery.finish().is_err());
        assert!(path.is_file());
    }
}
