//! Native local-only application core. Credentials never cross the GUI boundary.
mod connection_monitor;
pub mod core_releases;
pub mod core_updates;
pub mod diagnostics;
pub mod engine;
mod icmp;
pub mod imports;
pub mod inventory;
pub mod model;
pub mod network;
pub mod proxy;
#[cfg(windows)]
pub mod proxy_recovery;
pub mod runtime;
mod runtime_log;
pub mod store;
mod strict_json;
pub mod vault;

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, thiserror::Error, Serialize, Deserialize, Clone)]
#[error("{code}: {message}")]
pub struct Error {
    pub code: String,
    pub stage: String,
    pub message: String,
}
impl Error {
    pub fn new(code: &str, stage: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            stage: stage.into(),
            message: message.into(),
        }
    }
    pub fn io(stage: &str, e: std::io::Error) -> Self {
        Self::new(
            "IoError",
            stage,
            format!("{} (OS {:?})", e.kind(), e.raw_os_error()),
        )
    }
}
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(windows)]
pub mod routes;
#[cfg(windows)]
pub mod service;

mod app_rules;
pub mod audit;
pub mod client_updates;
pub mod mtu;
pub mod routing;
pub mod selection;
pub mod transport_options;

pub mod advanced_routing;
pub mod playground;
