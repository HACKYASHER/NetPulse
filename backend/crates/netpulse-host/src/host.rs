//! Private inherited stdio transport. It opens no TCP or named-pipe listener.
use std::{io::Write, ops::Deref, sync::Mutex};
use serde::Serialize;
static OUTPUT: Mutex<()> = Mutex::new(());
pub fn send(value: &serde_json::Value) -> std::io::Result<()> {
    let _guard = OUTPUT.lock().map_err(|_| std::io::Error::other("Output unavailable"))?;
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer(&mut stdout, value)?;
    stdout.write_all(b"\n")?;
    stdout.flush()
}
pub struct State<'a, T>(pub &'a T);
impl<T> Deref for State<'_, T> {
    type Target = T;
    fn deref(&self) -> &T { self.0 }
}
#[derive(Clone, Copy)]
pub struct AppHandle;
pub trait Emitter {
    fn emit<T: Serialize>(&self, event: &str, payload: T) -> std::io::Result<()>;
}
impl Emitter for AppHandle {
    fn emit<T: Serialize>(&self, event: &str, payload: T) -> std::io::Result<()> {
        send(&serde_json::json!({"event":event,"data":payload}))
    }
}
impl AppHandle {
    pub fn path(&self) -> Self { *self }
    pub fn resource_dir(&self) -> std::io::Result<std::path::PathBuf> {
        std::env::current_exe()?.parent().map(|p|p.to_path_buf())
            .ok_or_else(||std::io::Error::other("Executable directory unavailable"))
    }
    pub fn exit(&self, code: i32) {
        let _ = self.emit("quit-ready", code);
    }
}
pub mod async_runtime {
    pub use tokio::task::spawn_blocking;
}
