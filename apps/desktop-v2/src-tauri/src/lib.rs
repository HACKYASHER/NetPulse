use netpulse_core::{
    Error, Result, engine,
    imports::{self, ImportReport},
    inventory,
    model::{Preferences, ProfileSummary},
    network::{NetworkCore, Snapshot},
    store::{LogEvent, Store, SubscriptionSummary},
};
use serde::Serialize;
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tauri::{Emitter, Manager};
use zeroize::Zeroizing;
mod actions;
mod application_icon;
mod desktop;

struct AppState {
    core: Arc<Mutex<NetworkCore>>,
    data_dir: PathBuf,
    diagnostics: Arc<netpulse_core::diagnostics::Jobs>,
    updates: Arc<actions::UpdateCheck>,
    runtime_preparing: Arc<std::sync::atomic::AtomicBool>,
    startup_smoke: bool,
}
async fn with_core<T: Send + 'static>(
    s: &tauri::State<'_, AppState>,
    operation: &'static str,
    f: impl FnOnce(&mut NetworkCore) -> Result<T> + Send + 'static,
) -> Result<T> {
    if s.runtime_preparing
        .load(std::sync::atomic::Ordering::Acquire)
        && [
            "connect",
            "install_engine",
            "activate_core_bundle",
            "update_core",
            "install_compatibility_runtime",
            "manage_service",
        ]
        .contains(&operation)
    {
        return Err(Error::new(
            "RuntimePreparing",
            "startup",
            "Ядра ещё проверяются. Дождитесь завершения проверки.",
        ));
    }
    let core = s.core.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut core = core.lock().map_err(|_| {
            Error::new(
                "CoreUnavailable",
                "command",
                "Native application core is unavailable",
            )
        })?;
        let result = core.guarded(f);
        if let Err(e) = &result {
            let _ = core
                .store
                .log("ERROR", "Application", &e.stage, &e.code, &e.message);
        }
        result
    })
    .await
    .map_err(|_| {
        Error::new(
            "CoreTaskFailed",
            "command",
            "Native command task could not finish",
        )
    })?
}
#[derive(Serialize)]
struct Bootstrap {
    snapshot: Snapshot,
    preferences: Preferences,
    profiles: Vec<ProfileSummary>,
    subscriptions: Vec<SubscriptionSummary>,
    logs: Vec<LogEvent>,
    engine_ready: bool,
    engine_version: String,
    xray_version: String,
    app_version: &'static str,
    xray_ready: bool,
    runtime_error: Option<Error>,
    runtime_preparing: bool,
    startup_smoke: bool,
    service: netpulse_core::service::ServiceStatus,
}
#[tauri::command]
async fn bootstrap(state: tauri::State<'_, AppState>) -> Result<Bootstrap> {
    let preparing = state
        .runtime_preparing
        .load(std::sync::atomic::Ordering::Acquire);
    let startup_smoke = state.startup_smoke;
    with_core(&state, "bootstrap", move |c| {
        Ok(Bootstrap {
            snapshot: c.snapshot(),
            preferences: c.store.preferences()?,
            profiles: c.store.profiles()?,
            subscriptions: c.store.subscriptions()?,
            logs: c.store.logs()?,
            engine_ready: !preparing && engine::verify_engine(&c.engine_path).is_ok(),
            engine_version: netpulse_core::core_releases::version(
                c.engine_path.parent().unwrap(),
                "sing-box",
            ),
            xray_version: netpulse_core::core_releases::version(
                c.engine_path.parent().unwrap(),
                "xray",
            ),
            app_version: env!("CARGO_PKG_VERSION"),
            xray_ready: !preparing
                && netpulse_core::runtime::verify_xray(&c.engine_path.with_file_name("xray.exe"))
                    .is_ok(),
            runtime_preparing: preparing,
            startup_smoke,
            runtime_error: if preparing {
                None
            } else {
                engine::verify_engine(&c.engine_path)
                    .and_then(|()| {
                        netpulse_core::runtime::verify_xray(
                            &c.engine_path.with_file_name("xray.exe"),
                        )
                    })
                    .err()
            },
            service: (if startup_smoke {
                netpulse_core::service::installation_status()
            } else {
                netpulse_core::service::status()
            })
            .unwrap_or_else(|e| {
                let mut status = netpulse_core::service::installation_status().unwrap_or(
                    netpulse_core::service::ServiceStatus {
                        installed: false,
                        running: false,
                        engine_pid: None,
                        owned_by_caller: false,
                        tun_ready: false,
                        version: String::new(),
                        error: None,
                        diagnostic: String::new(),
                    },
                );
                status.error = Some(e);
                status
            }),
        })
    })
    .await
}
#[tauri::command]
async fn connect(state: tauri::State<'_, AppState>, id: String) -> Result<Snapshot> {
    state.diagnostics.cancel_all();
    with_core(&state, "connect", move |c| {
        let p = c.store.preferences()?;
        if !p.tun && !p.system_proxy {
            return Err(Error::new(
                "ConnectionModeRequired",
                "connect-mode",
                "Включите TUN и/или System Proxy перед подключением",
            ));
        }
        c.connect(&id)
    })
    .await
}
#[tauri::command]
async fn disconnect(state: tauri::State<'_, AppState>) -> Result<Snapshot> {
    state.diagnostics.cancel_all();
    with_core(&state, "disconnect", |c| {
        let snapshot = c.disconnect()?;
        let mut p = c.store.preferences()?;
        p.tun = false;
        p.system_proxy = false;
        c.apply_preferences(p)?;
        Ok(snapshot)
    })
    .await
}
#[tauri::command]
async fn log_days(state: tauri::State<'_, AppState>) -> Result<Vec<String>> {
    let directory = state.data_dir.clone();
    tauri::async_runtime::spawn_blocking(move || netpulse_core::audit::days(&directory))
        .await
        .map_err(|_| {
            Error::new(
                "LogReadFailed",
                "audit-days",
                "Не удалось прочитать каталог журналов",
            )
        })?
}
#[tauri::command]
async fn log_read(
    state: tauri::State<'_, AppState>,
    day: String,
    category: String,
) -> Result<String> {
    let directory = state.data_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        netpulse_core::audit::read(&directory, &day, &category)
    })
    .await
    .map_err(|_| Error::new("LogReadFailed", "audit-read", "Не удалось прочитать журнал"))?
}
#[tauri::command]
async fn profile_transport(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<netpulse_core::transport_options::ProfileTransportView> {
    with_core(&state, "profile_transport", move |c| {
        Ok(c.store.profile(&id)?.transport_view())
    })
    .await
}
#[tauri::command]
async fn save_profile_transport(
    state: tauri::State<'_, AppState>,
    id: String,
    value: Option<netpulse_core::transport_options::ProfileTransport>,
) -> Result<()> {
    state.diagnostics.cancel_all();
    with_core(&state, "save_profile_transport", move |c| {
        c.set_profile_transport(&id, value)
    })
    .await
}
#[tauri::command]
async fn profile_json(state: tauri::State<'_, AppState>, id: String) -> Result<String> {
    with_core(&state, "profile_json", move |c| {
        c.store.ensure_profile_editable(&id)?;
        let p = c.store.profile(&id)?;
        serde_json::to_string_pretty(p.outbound_override.as_ref().unwrap_or(&p.outbound))
            .map_err(|_| Error::new("InvalidJson", "profile-json", "Cannot encode configuration"))
    })
    .await
}
#[tauri::command]
async fn save_profile_json(
    state: tauri::State<'_, AppState>,
    id: String,
    json: String,
) -> Result<()> {
    state.diagnostics.cancel_all();
    let json = Zeroizing::new(json);
    with_core(&state, "save_profile_json", move |c| {
        c.set_profile_json(&id, &json)
    })
    .await
}
#[tauri::command]
async fn playground_processes() -> Result<Vec<netpulse_core::playground::Process>> {
    tauri::async_runtime::spawn_blocking(netpulse_core::playground::processes)
        .await
        .map_err(|_| {
            Error::new(
                "ProcessListFailed",
                "playground",
                "Не удалось прочитать список процессов",
            )
        })?
}
#[tauri::command]
async fn connection_analysis(
    state: tauri::State<'_, AppState>,
    pid: Option<u32>,
    path: String,
) -> Result<netpulse_core::playground::Analysis> {
    let request = with_core(&state, "connection_analysis", move |c| {
        c.analysis_request(pid, path)
    })
    .await?;
    tauri::async_runtime::spawn_blocking(move || request.run())
        .await
        .map_err(|_| {
            Error::new(
                "AnalysisFailed",
                "playground",
                "Не удалось прочитать соединения",
            )
        })?
}
#[tauri::command]
async fn snapshot(state: tauri::State<'_, AppState>) -> Result<Snapshot> {
    with_core(&state, "snapshot", |c| c.refresh_metrics()).await
}
fn import_payload(c: &mut NetworkCore, text: &str) -> Result<ImportReport> {
    let trimmed = text.trim();
    if !trimmed.contains(['\r', '\n'])
        && let Ok(u) = url_parse_subscription(trimmed)
    {
        let id = c.store.add_subscription("Imported subscription", &u)?;
        return c.update_subscription(&id);
    }
    c.store.import(imports::parse(text)?, None)
}
fn url_parse_subscription(text: &str) -> std::result::Result<String, ()> {
    if text.starts_with("https://")
        && !text
            .split_once("://")
            .unwrap()
            .1
            .split('/')
            .next()
            .unwrap_or("")
            .contains('@')
    {
        Ok(text.into())
    } else {
        Err(())
    }
}
#[tauri::command]
async fn import_text(state: tauri::State<'_, AppState>, text: String) -> Result<ImportReport> {
    let text = Zeroizing::new(text);
    with_core(&state, "import_text", move |c| import_payload(c, &text)).await
}
#[tauri::command]
async fn import_clipboard(state: tauri::State<'_, AppState>) -> Result<ImportReport> {
    with_core(&state, "import_clipboard", |c| {
        let text = clipboard()?;
        import_payload(c, &text)
    })
    .await
}
#[cfg(windows)]
fn clipboard() -> Result<Zeroizing<String>> {
    use windows_sys::Win32::System::{DataExchange::*, Memory::*};
    if unsafe { OpenClipboard(std::ptr::null_mut()) } == 0 {
        return Err(Error::io("clipboard-open", std::io::Error::last_os_error()));
    }
    struct ClipboardGuard;
    impl Drop for ClipboardGuard {
        fn drop(&mut self) {
            unsafe { CloseClipboard() };
        }
    }
    let _guard = ClipboardGuard;
    let handle = unsafe { GetClipboardData(13) };
    if handle.is_null() {
        return Err(Error::new(
            "ClipboardEmpty",
            "clipboard-read",
            "Clipboard has no text",
        ));
    }
    let size = unsafe { GlobalSize(handle) };
    if !(2..=imports::MAX_IMPORT * 2).contains(&size) {
        return Err(Error::new(
            "ImportLimit",
            "clipboard-read",
            "Clipboard text is empty or exceeds 8 MiB",
        ));
    }
    let ptr = unsafe { GlobalLock(handle) } as *const u16;
    if ptr.is_null() {
        return Err(Error::io("clipboard-lock", std::io::Error::last_os_error()));
    }
    let slice = unsafe { std::slice::from_raw_parts(ptr, size / 2) };
    let n = slice.iter().position(|v| *v == 0).unwrap_or(slice.len());
    let text = String::from_utf16(slice.get(..n).unwrap_or_default());
    unsafe { GlobalUnlock(handle) };
    text.map(Zeroizing::new).map_err(|_| {
        Error::new(
            "ClipboardInvalid",
            "clipboard-read",
            "Clipboard contains invalid Unicode",
        )
    })
}
#[cfg(not(windows))]
fn clipboard() -> Result<Zeroizing<String>> {
    Err(Error::new(
        "PlatformUnsupported",
        "clipboard-read",
        "Windows clipboard required",
    ))
}
#[cfg(windows)]
fn copy_clipboard(text: &str) -> Result<()> {
    use windows_sys::Win32::{
        Foundation::GlobalFree,
        System::{DataExchange::*, Memory::*},
    };
    if unsafe { OpenClipboard(std::ptr::null_mut()) } == 0 {
        return Err(Error::io("clipboard-open", std::io::Error::last_os_error()));
    }
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            unsafe { CloseClipboard() };
        }
    }
    let _guard = Guard;
    let wide: Zeroizing<Vec<u16>> = Zeroizing::new(text.encode_utf16().chain(Some(0)).collect());
    let handle = unsafe { GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2) };
    if handle.is_null() {
        return Err(Error::io(
            "clipboard-allocate",
            std::io::Error::last_os_error(),
        ));
    }
    let ptr = unsafe { GlobalLock(handle) } as *mut u16;
    if ptr.is_null() {
        unsafe { GlobalFree(handle) };
        return Err(Error::io("clipboard-lock", std::io::Error::last_os_error()));
    }
    unsafe {
        std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
        GlobalUnlock(handle)
    };
    if unsafe { EmptyClipboard() } == 0 || unsafe { SetClipboardData(13, handle) }.is_null() {
        let e = std::io::Error::last_os_error();
        unsafe { GlobalFree(handle) };
        return Err(Error::io("clipboard-write", e));
    }
    Ok(())
}
#[cfg(not(windows))]
fn copy_clipboard(_text: &str) -> Result<()> {
    Err(Error::new(
        "PlatformUnsupported",
        "clipboard-write",
        "Windows clipboard required",
    ))
}
#[tauri::command]
async fn copy_proxy_endpoint(state: tauri::State<'_, AppState>) -> Result<bool> {
    with_core(&state, "copy_proxy_endpoint", |c| {
        let endpoint = c.local_proxy_uri()?;
        copy_clipboard(&endpoint)?;
        Ok(true)
    })
    .await
}
fn read_limited(path: &Path, limit: usize) -> Result<Zeroizing<Vec<u8>>> {
    let file = std::fs::File::open(path).map_err(|e| Error::io("file-open", e))?;
    let mut raw = Zeroizing::new(vec![]);
    file.take(limit as u64 + 1)
        .read_to_end(&mut raw)
        .map_err(|e| Error::io("file-read", e))?;
    if raw.len() > limit {
        return Err(Error::new(
            "ImportLimit",
            "file-read",
            "Selected file exceeds its size limit",
        ));
    }
    Ok(raw)
}
fn decode_qr(bytes: &[u8]) -> Result<Zeroizing<String>> {
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| Error::new("QrImageInvalid", "qr-read", "Could not detect image format"))?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|_| {
            Error::new(
                "QrImageInvalid",
                "qr-read",
                "Could not decode image within resource limits",
            )
        })?
        .to_luma8();
    let (width, height) = image.dimensions();
    // Dark-mode coloured QR codes have the opposite polarity to printed codes.
    // Keep all processing local. Try both polarities and bounded contrast passes.
    for inverted in [false, true] {
        for threshold in [None, Some(80u8), Some(128u8), Some(176u8)] {
            let mut candidate = image.clone();
            for pixel in candidate.pixels_mut() {
                let value = if inverted { 255 - pixel[0] } else { pixel[0] };
                pixel[0] = threshold.map_or(value, |t| if value >= t { 255 } else { 0 });
            }
            // Restore a quiet zone for a code cropped tightly to its modules.
            let pad = (width.min(height) / 20).clamp(16, 128);
            let mut padded =
                image::GrayImage::from_pixel(width + pad * 2, height + pad * 2, image::Luma([255]));
            image::imageops::replace(&mut padded, &candidate, pad as i64, pad as i64);
            let mut prepared = rqrr::PreparedImage::prepare(padded);
            let mut texts = vec![];
            for grid in prepared.detect_grids() {
                if let Ok((_, content)) = grid.decode()
                    && !texts.contains(&content)
                {
                    texts.push(content);
                }
            }
            if !texts.is_empty() {
                return Ok(Zeroizing::new(texts.join("\n")));
            }
        }
    }
    Err(Error::new(
        "QrNotFound",
        "qr-read",
        "QR-код не распознан. Выберите исходное изображение, сохранив поля вокруг кода.",
    ))
}
#[tauri::command]
fn copy_log_text(text: String) -> Result<()> {
    if text.len() > 1024 * 1024 {
        return Err(Error::new(
            "LogTooLarge",
            "log-copy",
            "Log exceeds copy limit",
        ));
    }
    copy_clipboard(&text)
}
#[tauri::command]
async fn copy_event(state: tauri::State<'_, AppState>, id: String) -> Result<bool> {
    with_core(&state, "copy_event", move |c| {
        let event = c.store.event(&id)?;
        copy_clipboard(&serde_json::to_string_pretty(&event).map_err(|_| {
            Error::new("CopyFailed", "event-copy", "Не удалось подготовить событие")
        })?)?;
        Ok(true)
    })
    .await
}
#[tauri::command]
fn copy_ip(ip: String) -> Result<bool> {
    let ip = ip
        .parse::<std::net::IpAddr>()
        .map_err(|_| Error::new("InvalidIp", "ip-copy", "Неверный IP адрес"))?;
    copy_clipboard(&ip.to_string())?;
    Ok(true)
}
#[tauri::command]
async fn import_file(state: tauri::State<'_, AppState>, qr: bool) -> Result<Option<ImportReport>> {
    with_core(&state, "import_file", move |c| {
        let dialog = rfd::FileDialog::new().set_title(if qr {
            "Import QR image"
        } else {
            "Import profiles"
        });
        let dialog = if qr {
            dialog.add_filter("QR photographs", &["png", "jpg", "jpeg", "webp"])
        } else {
            dialog
        };
        let Some(path) = dialog.pick_file() else {
            return Ok(None);
        };
        let bytes = read_limited(
            &path,
            if qr {
                32 * 1024 * 1024
            } else {
                imports::MAX_IMPORT
            },
        )?;
        let text = if qr {
            decode_qr(&bytes)?
        } else {
            Zeroizing::new(String::from_utf8(bytes.to_vec()).map_err(|_| {
                Error::new("ImportError", "file-read", "Profile file must be UTF-8")
            })?)
        };
        Ok(Some(import_payload(c, &text)?))
    })
    .await
}
#[tauri::command]
async fn delete_profile(state: tauri::State<'_, AppState>, id: String) -> Result<()> {
    with_core(&state, "delete_profile", move |c| {
        if c.snapshot().active.is_some_and(|p| p.id == id) {
            return Err(Error::new(
                "ProfileInUse",
                "profile-delete",
                "Disconnect before deleting the active profile",
            ));
        }
        c.store.delete_profile(&id)
    })
    .await
}
#[tauri::command]
async fn add_subscription(
    state: tauri::State<'_, AppState>,
    name: String,
    url: String,
    protected: Option<bool>,
) -> Result<ImportReport> {
    let url = Zeroizing::new(url);
    with_core(&state, "add_subscription", move |c| {
        let id =
            c.store
                .add_subscription_with_protection(&name, &url, protected.unwrap_or(true))?;
        c.update_subscription(&id)
    })
    .await
}
#[tauri::command]
async fn copy_subscription_url(state: tauri::State<'_, AppState>, id: String) -> Result<bool> {
    with_core(&state, "copy_subscription_url", move |c| {
        let url = c.store.copyable_subscription_url(&id)?;
        copy_clipboard(&url)?;
        Ok(true)
    })
    .await
}
#[tauri::command]
async fn save_subscription(
    state: tauri::State<'_, AppState>,
    id: String,
    name: String,
    interval_hours: u32,
    protected: bool,
    lock_configuration: Option<bool>,
) -> Result<()> {
    with_core(&state, "save_subscription", move |c| {
        c.store
            .edit_subscription(&id, &name, interval_hours, protected)?;
        if lock_configuration == Some(true) {
            c.store.lock_subscription(&id)?;
        }
        Ok(())
    })
    .await
}
#[tauri::command]
async fn import_routing() -> Result<Option<netpulse_core::routing::RoutingSettings>> {
    tauri::async_runtime::spawn_blocking(|| {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("NetPulse routing", &["json"])
            .pick_file()
        else {
            return Ok(None);
        };
        let bytes = read_limited(&path, 2 * 1024 * 1024)?;
        Ok(Some(netpulse_core::routing::RoutingSettings::parse(
            &bytes,
        )?))
    })
    .await
    .map_err(|_| {
        Error::new(
            "RoutingImportFailed",
            "routing-import",
            "Не удалось открыть файл",
        )
    })?
}
#[tauri::command]
async fn export_routing(routing: netpulse_core::routing::RoutingSettings) -> Result<bool> {
    tauri::async_runtime::spawn_blocking(move || {
        let checked = routing.apply(&Preferences::default())?;
        let Some(path) = rfd::FileDialog::new()
            .set_file_name("netpulse-routing.json")
            .add_filter("NetPulse routing", &["json"])
            .save_file()
        else {
            return Ok(false);
        };
        let bytes = serde_json::to_vec_pretty(
            &netpulse_core::routing::RoutingSettings::from_preferences(&checked),
        )
        .map_err(|_| {
            Error::new(
                "RoutingExportFailed",
                "routing-export",
                "Не удалось подготовить настройки",
            )
        })?;
        std::fs::write(path, bytes).map_err(|e| Error::io("routing-export", e))?;
        Ok(true)
    })
    .await
    .map_err(|_| {
        Error::new(
            "RoutingExportFailed",
            "routing-export",
            "Не удалось сохранить файл",
        )
    })?
}
#[tauri::command]
async fn update_subscription(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<ImportReport> {
    with_core(&state, "update_subscription", move |c| {
        c.update_subscription(&id)
    })
    .await
}
#[tauri::command]
async fn delete_subscription(state: tauri::State<'_, AppState>, id: String) -> Result<()> {
    with_core(&state, "delete_subscription", move |c| {
        if c.snapshot()
            .active
            .is_some_and(|p| p.subscription_id.as_deref() == Some(&id))
        {
            return Err(Error::new(
                "ProfileInUse",
                "subscription-delete",
                "Disconnect before deleting the active subscription",
            ));
        }
        c.store.delete_subscription(&id)
    })
    .await
}
#[tauri::command]
async fn save_preferences(
    state: tauri::State<'_, AppState>,
    preferences: Preferences,
) -> Result<bool> {
    state.diagnostics.cancel_all();
    with_core(&state, "save_preferences", move |c| {
        let old = c.store.preferences()?;
        if old.autostart != preferences.autostart {
            desktop::autostart(preferences.autostart)?;
        }
        let autostart_changed = old.autostart != preferences.autostart;
        if let Err(e) = c.apply_preferences(preferences) {
            if autostart_changed {
                let _ = desktop::autostart(old.autostart);
            }
            return Err(e);
        }
        Ok(true)
    })
    .await
}
#[tauri::command]
async fn quick_network(
    state: tauri::State<'_, AppState>,
    tun: Option<bool>,
    system_proxy: Option<bool>,
    route_mode: Option<netpulse_core::model::RouteMode>,
    transport_engine: Option<String>,
) -> Result<bool> {
    state.diagnostics.cancel_all();
    with_core(&state, "quick_network", move |c| {
        let mut p = c.store.preferences()?;
        if let Some(v) = tun {
            p.tun = v;
        }
        if let Some(v) = system_proxy {
            p.system_proxy = v;
        }
        if let Some(v) = route_mode {
            p.route_mode = v;
        }
        if let Some(v) = transport_engine {
            p.transport_engine = v;
        }
        if !p.tun && !p.system_proxy && c.snapshot().engine_pid.is_some() {
            c.disconnect()?;
        }
        c.apply_preferences(p)?;
        Ok(true)
    })
    .await
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct UiPatch {
    app_view: Option<String>,
    server_view: Option<String>,
    favorites: Option<Vec<String>>,
    favorite_id: Option<String>,
}
#[tauri::command]
async fn quick_ui(state: tauri::State<'_, AppState>, patch: UiPatch) -> Result<Preferences> {
    with_core(&state, "quick_ui", move |c| {
        let mut p = c.store.preferences()?;
        if let Some(v) = patch.app_view {
            p.app_view = v;
        }
        if let Some(v) = patch.server_view {
            p.server_view = v;
        }
        if let Some(v) = patch.favorites {
            p.favorites = v;
        }
        if let Some(id) = patch.favorite_id {
            c.store.profile(&id)?;
            if p.favorites.contains(&id) {
                p.favorites.retain(|v| v != &id);
            } else {
                p.favorites.push(id);
            }
        }
        p = p.normalized();
        p.validate()?;
        c.store.save_preferences(&p)?;
        Ok(p)
    })
    .await
}
#[tauri::command]
async fn active_latency(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<netpulse_core::diagnostics::LatencyResult> {
    let (profile, prefs, path, route) = with_core(&state, "active_latency", move |c| {
        if c.snapshot().active.is_none_or(|p| p.id != id) {
            return Err(Error::new(
                "ConnectionChanged",
                "active-latency",
                "Connection changed",
            ));
        }
        Ok((
            c.store.profile(&id)?,
            c.store.preferences()?,
            c.engine_path.clone(),
            c.diagnostic_route(),
        ))
    })
    .await?;
    let method = prefs.ping_method.clone();
    tauri::async_runtime::spawn_blocking(move || {
        netpulse_core::diagnostics::latency_on_route(
            profile,
            prefs,
            path,
            &method,
            &std::sync::atomic::AtomicBool::new(false),
            Some(&route),
        )
    })
    .await
    .map_err(|_| Error::new("LatencyFailed", "active-latency", "Measurement task failed"))
}
#[tauri::command]
async fn check_core_updates() -> Result<Vec<netpulse_core::core_updates::CoreVersion>> {
    tauri::async_runtime::spawn_blocking(netpulse_core::core_updates::check)
        .await
        .map_err(|_| Error::new("CoreUpdateFailed", "core-update", "Update task failed"))
}

#[tauri::command]
async fn list_core_releases(engine: String) -> Result<Vec<netpulse_core::core_releases::Release>> {
    tauri::async_runtime::spawn_blocking(move || netpulse_core::core_releases::list(&engine))
        .await
        .map_err(|_| {
            Error::new(
                "CoreUpdateFailed",
                "core-release",
                "Release lookup task failed",
            )
        })?
}
#[tauri::command]
async fn update_core(
    state: tauri::State<'_, AppState>,
    release: netpulse_core::core_releases::Release,
) -> Result<serde_json::Value> {
    state.diagnostics.cancel_all();
    let data = state.data_dir.clone();
    with_core(&state, "update_core", move |c| {
        if c.snapshot().engine_pid.is_some() {
            return Err(Error::new(
                "ConnectionInUse",
                "core-update",
                "Отключите VPN перед обновлением ядра",
            ));
        }
        let profiles = c
            .store
            .profiles()?
            .iter()
            .map(|p| c.store.profile(&p.id))
            .collect::<Result<Vec<_>>>()?;
        let engine = release.engine.clone();
        let previous = netpulse_core::core_releases::version(&data.join("engine"), &engine);
        netpulse_core::core_releases::install(release, &data, &profiles, &c.store.preferences()?)?;
        let installed = netpulse_core::core_releases::executable_version(&data.join("engine").join(if engine == "xray" {"xray.exe"} else {"sing-box.exe"}), &engine)?;
        c.store.log("INFO", "Update", "core-installed", "Verified", &format!("{engine}: {previous} -> {installed}; executable version and SHA-256 verified"))?;
        Ok(serde_json::json!({"engine":engine,"previous":previous,"installed":installed,"cache_clean": !std::fs::read_dir(data.join("core-downloads")).is_ok_and(|mut i| i.next().is_some())}))
    })
    .await
}
#[tauri::command]
fn open_core_page(url: String) -> Result<()> {
    if ![
        "https://github.com/XTLS/Xray-core",
        "https://github.com/SagerNet/sing-box",
        "https://github.com/SagerNet/sing-geoip",
        "https://github.com/SagerNet/sing-geosite",
        "https://github.com/HACKYASHER/NetPulse",
    ]
    .iter()
    .any(|base| url == *base || url.starts_with(&format!("{base}/releases")))
        || url.contains(['\r', '\n', '"'])
    {
        return Err(Error::new(
            "InvalidLink",
            "core-page",
            "Only official core GitHub pages are allowed",
        ));
    }
    use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
    let verb = netpulse_core::service::wide("open");
    let url = netpulse_core::service::wide(&url);
    if unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            url.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    } as usize
        <= 32
    {
        return Err(Error::new(
            "OpenLinkFailed",
            "core-page",
            "Could not open official page",
        ));
    }
    Ok(())
}
#[tauri::command]
async fn activate_core_bundle(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    rollback: bool,
) -> Result<bool> {
    let resources = app
        .path()
        .resource_dir()
        .map_err(|_| Error::new("ResourceMissing", "core-update", "Bundled runtimes missing"))?;
    let data = state.data_dir.clone();
    with_core(&state, "activate_core_bundle", move |c| {
        if c.snapshot().engine_pid.is_some() {
            return Err(Error::new(
                "ConnectionInUse",
                "core-update",
                "Disconnect before updating or rolling back runtimes",
            ));
        }
        if rollback {
            netpulse_core::core_updates::rollback(&data)?;
        } else {
            netpulse_core::core_updates::activate_bundled(&resources, &data)?;
        }
        Ok(true)
    })
    .await
}
#[tauri::command]
async fn hide_to_tray(window: tauri::WebviewWindow) -> Result<bool> {
    window
        .hide()
        .map_err(|_| Error::new("WindowHideFailed", "tray", "Could not hide window"))?;
    Ok(true)
}
#[tauri::command]
async fn quit_application(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<bool> {
    state.diagnostics.cancel_all();
    with_core(&state, "quit_application", |c| {
        c.disconnect()?;
        Ok(())
    })
    .await?;
    app.exit(0);
    Ok(true)
}
#[tauri::command]
async fn start_latency(
    state: tauri::State<'_, AppState>,
    ids: Vec<String>,
    method: String,
) -> Result<String> {
    if ids.is_empty()
        || ids.len() > 2000
        || ![
            "tcp_connect_rtt",
            "proxy_https_rtt",
            "proxy_https_head_rtt",
            "icmp_rtt",
        ]
        .contains(&method.as_str())
    {
        return Err(Error::new(
            "InvalidLatencyTest",
            "latency-start",
            "Select 1–2000 profiles and a valid measurement method",
        ));
    }
    let (profiles, prefs, path) = with_core(&state, "start_latency", move |c| {
        let mut unique = ids;
        unique.sort();
        unique.dedup();
        Ok((
            unique
                .iter()
                .map(|id| c.store.profile(id))
                .collect::<Result<Vec<_>>>()?,
            c.store.preferences()?,
            c.engine_path.clone(),
        ))
    })
    .await?;
    let jobs = state.diagnostics.clone();
    let core = state.core.clone();
    let (id, cancel) = jobs.begin("latency", profiles.len())?;
    let result_id = id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let queue = Mutex::new(profiles.into_iter());
        let work = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            std::thread::scope(|scope| {
                for _ in 0..2 {
                    let jobs = &jobs;
                    let core = &core;
                    let id = &id;
                    let prefs = &prefs;
                    let path = &path;
                    let method = &method;
                    let cancel = &cancel;
                    let queue = &queue;
                    scope.spawn(move || {
                        loop {
                            if cancel.load(std::sync::atomic::Ordering::SeqCst) {
                                break;
                            }
                            let profile = queue.lock().ok().and_then(|mut q| q.next());
                            let Some(profile) = profile else {
                                break;
                            };
                            let result = netpulse_core::diagnostics::latency(
                                profile,
                                prefs.clone(),
                                path.clone(),
                                method,
                                cancel,
                            );
                            let value = serde_json::to_value(result).unwrap_or_default();
                            if let Ok(c) = core.lock() {
                                let _ = c.store.add_measurement("latency", &value);
                            }
                            jobs.append(id, value);
                        }
                    });
                }
            })
        }));
        jobs.finish(&id, work.err().map(|_|Error::new("DiagnosticWorkerFailed","latency-job","Проверка остановлена из-за ошибки рабочего процесса; можно запустить её повторно")));
    });
    Ok(result_id)
}
#[tauri::command]
async fn start_speed(
    state: tauri::State<'_, AppState>,
    request: netpulse_core::diagnostics::SpeedRequest,
) -> Result<String> {
    request.validate()?;
    let route = with_core(&state, "start_speed", |c| Ok(c.diagnostic_route())).await?;
    if request.mode != "direct" && route.vpn_port.is_none() {
        return Err(Error::new(
            "NotConnected",
            "speed-route",
            "Connect before starting a VPN or Compare test",
        ));
    }
    let jobs = state.diagnostics.clone();
    let core = state.core.clone();
    let modes = if request.mode == "compare" {
        vec!["direct", "vpn"]
    } else if request.mode == "vpn" {
        vec!["vpn"]
    } else {
        vec!["direct"]
    };
    let (id, cancel) = jobs.begin("speed", modes.len())?;
    let result_id = id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let budget = request.max_payload_mib * 1024 * 1024 / modes.len() as u64;
        for mode in modes {
            if cancel.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }
            let result = netpulse_core::diagnostics::speed_observed(
                &request,
                &route,
                mode,
                budget,
                &cancel,
                &|sample| jobs.progress(&id, sample),
            );
            let value = serde_json::to_value(result).unwrap_or_default();
            if let Ok(c) = core.lock() {
                let _ = c.store.add_measurement("speed", &value);
            }
            jobs.append(&id, value);
        }
        jobs.finish(&id, None);
    });
    Ok(result_id)
}
#[tauri::command]
async fn current_diagnostic(
    state: tauri::State<'_, AppState>,
) -> Result<Option<netpulse_core::diagnostics::JobStatus>> {
    state.diagnostics.active()
}
#[tauri::command]
async fn diagnostic_status(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<netpulse_core::diagnostics::JobStatus> {
    state.diagnostics.status(&id)
}
#[tauri::command]
async fn cancel_diagnostic(state: tauri::State<'_, AppState>, id: String) -> Result<()> {
    state.diagnostics.cancel(&id)
}
#[tauri::command]
async fn diagnostic_history(
    state: tauri::State<'_, AppState>,
    kind: String,
) -> Result<Vec<serde_json::Value>> {
    with_core(&state, "diagnostic_history", move |c| {
        c.store.measurements(&kind)
    })
    .await
}
#[tauri::command]
async fn select_profile(state: tauri::State<'_, AppState>, id: String) -> Result<bool> {
    state.diagnostics.cancel_all();
    with_core(&state, "select_profile", move |c| {
        c.select_profile(&id)?;
        Ok(true)
    })
    .await
}
#[tauri::command]
async fn install_compatibility_runtime(state: tauri::State<'_, AppState>) -> Result<()> {
    let dir = state.data_dir.clone();
    with_core(&state, "install_compatibility_runtime", move |c| {
        if c.snapshot().engine_pid.is_some() {
            return Err(Error::new(
                "EngineInUse",
                "xray-install",
                "Disconnect before changing runtimes",
            ));
        }
        netpulse_core::runtime::install_xray(&dir)
    })
    .await
}
#[tauri::command]
async fn manage_service(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    install: bool,
    upgrade: Option<bool>,
) -> Result<()> {
    let resource = app
        .path()
        .resource_dir()
        .map_err(|_| {
            Error::new(
                "ResourceMissing",
                "service-install",
                "Service installer resource is missing",
            )
        })?
        .join("NetPulseService.exe");
    with_core(&state, "manage_service", move |c| {
        let recovery = install && upgrade.unwrap_or(false);
        if c.snapshot().engine_pid.is_some() && !recovery {
            return Err(Error::new(
                "ConnectionInUse",
                "service-install",
                "Disconnect before installing or removing the Windows service",
            ));
        }
        if recovery {
            // Disconnect restores the proxy first. A service timeout keeps its
            // lease until the verified UAC helper confirms SCM shutdown.
            if let Err(e) = c.disconnect()
                && !matches!(
                    e.code.as_str(),
                    "ServiceTimeout" | "IpcTimeout" | "ServiceBusy" | "IoError"
                )
            {
                return Err(e);
            }
        }
        let directory = c.engine_path.parent().ok_or_else(|| {
            Error::new(
                "EngineMissing",
                "service-install",
                "Install the network engine first",
            )
        })?;
        if install {
            engine::verify_engine(&c.engine_path)?;
        }
        netpulse_core::service::elevated_install(
            &resource,
            directory,
            install,
            upgrade.unwrap_or(false),
        )?;
        if recovery {
            c.disconnect()?;
        }
        let _ = c.store.log(
            "INFO",
            "Service",
            "service-install",
            "ServiceUpdated",
            "Windows service operation completed",
        );
        Ok(())
    })
    .await
}
#[tauri::command]
async fn read_application_catalog(
    state: tauri::State<'_, AppState>,
) -> Result<Option<Vec<inventory::Application>>> {
    let directory = state.data_dir.clone();
    with_core(&state, "application-catalog-read", move |_| {
        inventory::read_catalog(&directory)
    })
    .await
}
#[tauri::command]
async fn save_application_catalog(
    state: tauri::State<'_, AppState>,
    applications: Vec<inventory::Application>,
) -> Result<()> {
    let directory = state.data_dir.clone();
    with_core(&state, "application-catalog-save", move |_| {
        inventory::write_catalog(&directory, applications)
    })
    .await
}
#[tauri::command]
async fn scan_applications() -> Result<Vec<inventory::Application>> {
    tauri::async_runtime::spawn_blocking(inventory::scan)
        .await
        .map_err(|_| {
            Error::new(
                "ScanFailed",
                "applications-scan",
                "Application scan could not finish",
            )
        })?
}
#[tauri::command]
async fn application_icon(path: String) -> Result<Option<String>> {
    tauri::async_runtime::spawn_blocking(move || application_icon::read(&path))
        .await
        .map_err(|_| Error::new("IconReadFailed", "applications-icon", "Icon lookup failed"))?
}
#[tauri::command]
async fn choose_exe() -> Result<Option<inventory::Application>> {
    tauri::async_runtime::spawn_blocking(|| {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Windows application", &["exe"])
            .pick_file()
        else {
            return Ok(None);
        };
        if path
            .extension()
            .is_none_or(|e| !e.eq_ignore_ascii_case("exe"))
        {
            return Err(Error::new(
                "InvalidApplication",
                "applications-add",
                "Select an EXE file",
            ));
        }
        Ok(Some(inventory::Application {
            name: path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            path: path.to_string_lossy().into_owned(),
            source: "manual".into(),
        }))
    })
    .await
    .map_err(|_| {
        Error::new(
            "DialogFailed",
            "applications-add",
            "Application picker failed",
        )
    })?
}
#[tauri::command]
async fn public_ip(state: tauri::State<'_, AppState>) -> Result<String> {
    with_core(&state, "public_ip", |c| c.public_ip()).await
}
#[tauri::command]
async fn public_ips(
    state: tauri::State<'_, AppState>,
) -> Result<netpulse_core::network::PublicIps> {
    let route = with_core(&state, "public_ips", |c| Ok(c.diagnostic_route())).await?;
    tauri::async_runtime::spawn_blocking(move || netpulse_core::network::public_ips(route))
        .await
        .map_err(|_| {
            Error::new(
                "PublicIpFailed",
                "public-ip",
                "Не удалось завершить проверку IP",
            )
        })
}
#[tauri::command]
async fn install_engine(state: tauri::State<'_, AppState>) -> Result<()> {
    let dir = state.data_dir.clone();
    with_core(&state, "install_engine", move |c| {
        if c.snapshot().engine_pid.is_some() {
            return Err(Error::new(
                "EngineInUse",
                "engine-install",
                "Disconnect before installing the engine",
            ));
        }
        engine::install(&dir)
    })
    .await
}
#[tauri::command]
async fn clear_logs(state: tauri::State<'_, AppState>) -> Result<()> {
    with_core(&state, "clear_logs", |c| c.store.clear_logs()).await
}
#[tauri::command]
async fn clear_file_logs(state: tauri::State<'_, AppState>) -> Result<()> {
    with_core(&state, "clear_file_logs", |c| c.store.clear_file_logs()).await
}
#[tauri::command]
async fn logs(state: tauri::State<'_, AppState>) -> Result<Vec<LogEvent>> {
    with_core(&state, "logs", |c| c.store.logs()).await
}
#[tauri::command]
async fn export_data(state: tauri::State<'_, AppState>, kind: String) -> Result<bool> {
    with_core(&state, "export_data", move |c| {
        let (name, bytes) = match kind.as_str() {
            "settings" => (
                "netpulse-settings.json",
                c.store.export_settings()?.into_bytes(),
            ),
            "logs" => (
                "netpulse-logs.json",
                serde_json::to_vec_pretty(&c.store.logs()?).unwrap(),
            ),
            "backup" => ("netpulse-backup.npb", c.store.backup()?),
            _ => return Err(Error::new("ExportKind", "export", "Unknown export type")),
        };
        let Some(path) = rfd::FileDialog::new().set_file_name(name).save_file() else {
            return Ok(false);
        };
        std::fs::write(path, bytes).map_err(|e| Error::io("export-write", e))?;
        Ok(true)
    })
    .await
}
#[tauri::command]
async fn restore_data(state: tauri::State<'_, AppState>, backup: bool) -> Result<bool> {
    with_core(&state, "restore_data", move |c| {
        if c.snapshot().engine_pid.is_some() {
            return Err(Error::new(
                "ConnectionInUse",
                "restore",
                "Disconnect before restoring data",
            ));
        }
        let Some(path) = rfd::FileDialog::new().pick_file() else {
            return Ok(false);
        };
        let bytes = read_limited(&path, 64 * 1024 * 1024)?;
        if backup {
            c.store.restore(&bytes)?;
        } else {
            let prefs: Preferences = serde_json::from_slice(&bytes).map_err(|_| {
                Error::new("ImportError", "settings-import", "Invalid settings JSON")
            })?;
            c.store.save_preferences(&prefs)?;
        }
        Ok(true)
    })
    .await
}
#[tauri::command]
async fn delete_local_data(state: tauri::State<'_, AppState>) -> Result<()> {
    with_core(&state, "delete_local_data", |c| {
        c.disconnect()?;
        c.store.clear_all()
    })
    .await
}

#[tauri::command]
async fn startup_smoke_failed(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    message: String,
) -> Result<()> {
    if !state.startup_smoke {
        return Err(Error::new(
            "SmokeOnly",
            "startup-smoke",
            "Test mode required",
        ));
    }
    std::fs::write(state.data_dir.join("startup-failure.json"), serde_json::to_vec(&serde_json::json!({"result":"failed", "reason":message.chars().take(500).collect::<String>()})).unwrap())
        .map_err(|e| Error::io("startup-smoke", e))?;
    app.exit(1);
    Ok(())
}
#[tauri::command]
async fn startup_smoke_ready(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    views: Vec<String>,
) -> Result<()> {
    if !state.startup_smoke
        || state
            .runtime_preparing
            .load(std::sync::atomic::Ordering::Acquire)
    {
        return Ok(());
    }
    let expected = [
        "dashboard",
        "connections",
        "routing",
        "activity",
        "networkSettings:general",
        "networkSettings:tls",
        "networkSettings:dns",
        "networkSettings:rules",
        "networkSettings:playground",
        "settings:appearance",
        "settings:ping",
        "settings:cores",
        "settings:about",
        "settings:data",
        "connections:import",
        "dashboard:picker",
    ];
    if views.iter().map(String::as_str).ne(expected) {
        return Err(Error::new(
            "SmokeViewsMissing",
            "startup-smoke",
            "Expected native UI views were not rendered",
        ));
    }
    let directory = state.data_dir.clone();
    let result = with_core(&state, "startup_smoke", move |c| {
        let preferences = c.store.preferences()?;
        let snapshot = c.snapshot();
        if preferences.tun || preferences.system_proxy || snapshot.engine_pid.is_some() || !c.store.profiles()?.is_empty() {
            return Err(Error::new("UnsafeSmokeState", "startup-smoke", "Smoke test must remain disconnected and isolated"));
        }
        Ok(serde_json::json!({"result":"passed", "version":env!("CARGO_PKG_VERSION"), "webview_ready":true,
            "runtime_ready":engine::verify_engine(&c.engine_path).is_ok(), "system_network_changed":false, "browser_server_used":false, "rendered_views":views, "form_help_coverage":true, "layout_geometry":true}))
    }).await?;
    std::fs::write(
        directory.join("startup-result.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .map_err(|e| Error::io("startup-smoke", e))?;
    app.exit(0);
    Ok(())
}
pub fn run() {
    // Explicit packaging-only mode: a new data/WebView directory and hidden
    // window, no single-instance interference, autostart, subscriptions or VPN.
    let args = std::env::args_os().collect::<Vec<_>>();
    let smoke = if args.get(1).is_some_and(|a| a == "--startup-smoke") {
        let directory = args
            .get(2)
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| std::process::exit(2));
        if std::fs::create_dir(&directory).is_err() {
            std::process::exit(2);
        }
        Some(directory)
    } else {
        None
    };
    if smoke.is_none() && !desktop::claim_instance() {
        return;
    }
    let mut context = tauri::generate_context!();
    let startup_failure_directory = smoke.clone();
    if let Some(directory) = &smoke {
        for window in &mut context.config_mut().app.windows {
            window.visible = false;
            window.data_directory = Some(directory.join("webview"));
        }
    }
    tauri::Builder::default()
        .setup(move |app| {
            let startup_smoke = smoke.is_some();
            let dir = smoke.clone().unwrap_or(app.path().app_local_data_dir()?);
            std::fs::create_dir_all(&dir)?;
            let panic_directory = dir.clone();
            let previous_hook = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                let event = LogEvent {
                    id: uuid::Uuid::new_v4().to_string(),
                    timestamp: netpulse_core::store::now(),
                    level: "CRITICAL".into(),
                    subsystem: "Application".into(),
                    stage: "panic".into(),
                    code: "RustPanic".into(),
                    message: info
                        .location()
                        .map(|l| format!("Runtime panic at line {}", l.line()))
                        .unwrap_or_else(|| "Runtime panic".into()),
                };
                let _ = netpulse_core::audit::append(&panic_directory, &event);
                previous_hook(info);
            }));
            let recovery = netpulse_core::proxy_recovery::recover_stale(&dir.join("recovery"));
            let store = Store::open(&dir.join("netpulse.db"))?;
            let launch_preferences = store.preferences()?.for_startup();
            store.save_preferences(&launch_preferences)?;
            if let Err(e) = recovery {
                store.log("ERROR", "Recovery", &e.stage, &e.code, &e.message)?;
            }
            if let Some(window) = app.get_webview_window("main") {
                let prefs = store.preferences()?;
                let _ = window.set_size(tauri::LogicalSize::new(
                    prefs.window.width,
                    prefs.window.height,
                ));
                if prefs.window.maximized {
                    let _ = window.maximize();
                }
            }
            store.log(
                "INFO",
                "Application",
                "startup",
                "Started",
                "NetPulse native core started",
            )?;
            let handle = app.handle().clone();
            let mut core = NetworkCore::new(store, engine::installed_engine(&dir))?;
            core.observe(move |state| {
                let _ = handle.emit_to("main", "connection-state", state);
            });
            let updates = Arc::new(actions::UpdateCheck::default());
            if !startup_smoke {
                actions::start_update_monitor(app.handle().clone(), dir.clone(), updates.clone());
            }
            let runtime_preparing = Arc::new(std::sync::atomic::AtomicBool::new(true));
            let preparing = runtime_preparing.clone();
            let resources = app.path().resource_dir()?;
            let runtime_directory = dir.clone();
            let runtime_handle = app.handle().clone();
            // Runtime validation/copying may wait on Windows security scanning.
            // Keep the UI available, and serialize engine operations via this flag.
            std::thread::spawn(move || {
                let result = std::panic::catch_unwind(|| {
                    netpulse_core::core_updates::ensure_bundled(&resources, &runtime_directory)
                })
                .unwrap_or_else(|_| {
                    Err(Error::new(
                        "RuntimePreparationFailed",
                        "startup",
                        "Не удалось проверить ядра. Переустановите официальный выпуск NetPulse.",
                    ))
                });
                if let Err(e) = result {
                    let _ = netpulse_core::audit::append(
                        &runtime_directory,
                        &LogEvent {
                            id: uuid::Uuid::new_v4().to_string(),
                            timestamp: netpulse_core::store::now(),
                            level: "ERROR".into(),
                            subsystem: "Core".into(),
                            stage: e.stage,
                            code: e.code,
                            message: e.message,
                        },
                    );
                }
                preparing.store(false, std::sync::atomic::Ordering::Release);
                let _ = runtime_handle.emit("runtime-prepared", ());
            });
            app.manage(AppState {
                core: Arc::new(Mutex::new(core)),
                data_dir: dir,
                diagnostics: Arc::new(netpulse_core::diagnostics::Jobs::default()),
                updates,
                runtime_preparing,
                startup_smoke,
            });
            if !startup_smoke {
                desktop::tray(app)?;
                actions::subscription_monitor(
                    app.handle().clone(),
                    app.state::<AppState>().core.clone(),
                );
                actions::selection_monitor(
                    app.handle().clone(),
                    app.state::<AppState>().core.clone(),
                    app.state::<AppState>().diagnostics.clone(),
                );
            }
            let monitor = app.state::<AppState>().core.clone();
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    if let Ok(mut core) = monitor.try_lock() {
                        let snapshot = core.snapshot();
                        let _ = handle.emit("connection-snapshot", snapshot);
                    }
                }
            });
            if std::env::args().any(|a| a == "--background")
                && let Some(window) = app.get_webview_window("main")
            {
                let _ = window.hide();
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Resized(_))
                && window.is_minimized().unwrap_or(false)
                && let Some(state) = window.try_state::<AppState>()
                && let Ok(core) = state.core.try_lock()
                && core.store.preferences().is_ok_and(|p| p.minimize_to_tray)
            {
                let _ = window.hide();
            }
            if let tauri::WindowEvent::Resized(size) = event
                && let Some(state) = window.try_state::<AppState>()
                && let Ok(c) = state.core.try_lock()
                && let Ok(mut prefs) = c.store.preferences()
            {
                let scale = window.scale_factor().unwrap_or(1.0);
                let width = (size.width as f64 / scale).round() as u32;
                let height = (size.height as f64 / scale).round() as u32;
                if (960..=7680).contains(&width) && (680..=4320).contains(&height) {
                    let next = netpulse_core::model::WindowState {
                        width,
                        height,
                        maximized: window.is_maximized().unwrap_or(false),
                    };
                    if prefs.window != next {
                        prefs.window = next;
                        let _ = c.store.save_preferences(&prefs);
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            actions::ui_event,
            actions::open_log_folder,
            actions::check_client_update,
            actions::download_client_update,
            actions::install_client_update,
            actions::start_best_server,
            actions::start_mtu,
            log_days,
            log_read,
            profile_transport,
            save_profile_transport,
            profile_json,
            save_profile_json,
            playground_processes,
            connection_analysis,
            bootstrap,
            connect,
            disconnect,
            snapshot,
            import_text,
            import_clipboard,
            import_file,
            delete_profile,
            add_subscription,
            copy_subscription_url,
            save_subscription,
            import_routing,
            export_routing,
            update_subscription,
            delete_subscription,
            save_preferences,
            quick_network,
            quick_ui,
            active_latency,
            check_core_updates,
            list_core_releases,
            update_core,
            open_core_page,
            activate_core_bundle,
            hide_to_tray,
            quit_application,
            select_profile,
            start_latency,
            start_speed,
            diagnostic_status,
            current_diagnostic,
            cancel_diagnostic,
            diagnostic_history,
            manage_service,
            install_compatibility_runtime,
            read_application_catalog,
            save_application_catalog,
            scan_applications,
            application_icon,
            choose_exe,
            public_ip,
            public_ips,
            install_engine,
            clear_logs,
            clear_file_logs,
            logs,
            export_data,
            restore_data,
            delete_local_data,
            copy_proxy_endpoint,
            copy_event,
            copy_log_text,
            copy_ip,
            startup_smoke_ready,
            startup_smoke_failed
        ])
        .build(context)
        .unwrap_or_else(|error| {
            if let Some(directory) = &startup_failure_directory {
                let _ = std::fs::write(directory.join("startup-failure.txt"), error.to_string());
            } else {
                desktop::show_startup_error(&format!("Не удалось открыть NetPulse. {error}"));
            }
            std::process::exit(1)
        })
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit)
                && let Some(state) = app.try_state::<AppState>()
                && let Ok(mut c) = state.core.lock()
            {
                state.diagnostics.cancel_all();
                let _ = c.disconnect();
            }
        });
}

#[cfg(test)]
mod photo_tests {
    use super::*;
    #[test]
    fn photo_qr_distinguishes_subscription_and_key() {
        let subscription = decode_qr(include_bytes!("../fixtures/qr-subscription.png")).unwrap();
        assert_eq!(
            url_parse_subscription(&subscription).unwrap(),
            "https://example.org/subscription-fixture"
        );
        let profile = decode_qr(include_bytes!("../fixtures/qr-profile.png")).unwrap();
        assert!(url_parse_subscription(&profile).is_err());
        assert_eq!(imports::parse(&profile).unwrap().profiles.len(), 1);
        assert!(url_parse_subscription("https://user:fixture@example.org:443").is_err());
        assert!(decode_qr(b"damaged photo").is_err());
    }
    #[test]
    fn inverted_coloured_qr_and_rotated_photo_decode_locally() {
        use image::{ImageEncoder, Rgb};
        let original = image::load_from_memory(include_bytes!("../fixtures/qr-subscription.png"))
            .unwrap()
            .to_luma8();
        let coloured = image::RgbImage::from_fn(original.width(), original.height(), |x, y| {
            if original.get_pixel(x, y)[0] < 128 {
                Rgb([53, 197, 214])
            } else {
                Rgb([16, 17, 24])
            }
        });
        for sample in [coloured.clone(), image::imageops::rotate90(&coloured)] {
            let mut bytes = vec![];
            image::codecs::png::PngEncoder::new(&mut bytes)
                .write_image(
                    sample.as_raw(),
                    sample.width(),
                    sample.height(),
                    image::ExtendedColorType::Rgb8,
                )
                .unwrap();
            assert_eq!(
                decode_qr(&bytes).unwrap().as_str(),
                "https://example.org/subscription-fixture"
            );
        }
        // Optional private regression input; never copied into the source tree or printed.
        if let Some(path) = std::env::var_os("NETPULSE_TEST_QR_IMAGE") {
            let bytes = read_limited(Path::new(&path), 16 * 1024 * 1024).unwrap();
            assert!(!decode_qr(&bytes).unwrap().is_empty());
        }
    }
}
