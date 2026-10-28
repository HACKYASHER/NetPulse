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
use host::Emitter;
mod host;
use zeroize::Zeroizing;
mod actions;
mod application_icon;
mod desktop;

struct AppState {
    core: Arc<Mutex<NetworkCore>>,
    data_dir: PathBuf,
    diagnostics: Arc<netpulse_core::diagnostics::Jobs>,
    runtime_preparing: Arc<std::sync::atomic::AtomicBool>,
    startup_smoke: bool,
}
fn blocked_during_runtime_change(operation:&str)->bool {
    if matches!(operation,"install_client_update"|"start_best_server"|"start_mtu_tuning"|"start_mtu"|"mtu-tuning"){return true;}
    matches!(operation,"connect"|"install_engine"|"activate_core_bundle"|"update_core"|"install_compatibility_runtime"|"manage_service"|"restore_data"|"delete_local_data"|"client-update"|"save_preferences"|"quick_network"|"select_profile"|"save_profile_transport"|"save_profile_json"|"import_text"|"import_clipboard"|"import_file"|"delete_profile"|"add_subscription"|"save_subscription"|"update_subscription"|"delete_subscription"|"start_latency"|"active_latency"|"start_speed")
}
struct RuntimeOperation(Arc<std::sync::atomic::AtomicBool>);
impl RuntimeOperation {
    fn acquire(flag:&Arc<std::sync::atomic::AtomicBool>)->Result<Self>{
        use std::sync::atomic::Ordering;
        flag.compare_exchange(false,true,Ordering::AcqRel,Ordering::Acquire).map_err(|_|Error::new("RuntimePreparing","runtime-operation","Другая операция с ядрами или обновлением ещё выполняется."))?;
        Ok(Self(flag.clone()))
    }
}
impl Drop for RuntimeOperation {fn drop(&mut self){self.0.store(false,std::sync::atomic::Ordering::Release);let _=host::AppHandle.emit("runtime-prepared",serde_json::Value::Null);}}
#[cfg(test)]
mod runtime_operation_tests {
    use super::*;
    #[test] fn runtime_transaction_is_exclusive_and_releases_on_failure() {
        use std::sync::atomic::{AtomicBool,Ordering};
        let flag=Arc::new(AtomicBool::new(false));
        {let _first=RuntimeOperation::acquire(&flag).unwrap();assert!(RuntimeOperation::acquire(&flag).is_err());assert!(flag.load(Ordering::Acquire));}
        assert!(!flag.load(Ordering::Acquire));assert!(RuntimeOperation::acquire(&flag).is_ok());
    }
    #[test] fn runtime_mutations_and_diagnostic_launches_are_blocked_while_reads_remain_available() {
        for method in ["connect","delete_local_data","restore_data","client-update","manage_service","save_preferences","select_profile","start_latency","active_latency","start_speed"]{assert!(blocked_during_runtime_change(method),"{method}");}
        for method in ["bootstrap","snapshot","logs","disconnect","quit_application"]{assert!(!blocked_during_runtime_change(method),"{method}");}
    }
}
async fn with_core<T: Send + 'static>(
    s: &host::State<'_, AppState>,
    operation: &'static str,
    f: impl FnOnce(&mut NetworkCore) -> Result<T> + Send + 'static,
) -> Result<T> {
    if s.runtime_preparing
        .load(std::sync::atomic::Ordering::Acquire)
        && blocked_during_runtime_change(operation)
    {
        return Err(Error::new(
            "RuntimePreparing",
            "startup",
            "Ядра ещё проверяются. Дождитесь завершения проверки.",
        ));
    }
    let preparing=s.runtime_preparing.clone();
    let core = s.core.clone();
    host::async_runtime::spawn_blocking(move || {
        let mut core = core.lock().map_err(|_| {
            Error::new(
                "CoreUnavailable",
                "command",
                "Native application core is unavailable",
            )
        })?;
        if preparing.load(std::sync::atomic::Ordering::Acquire) && blocked_during_runtime_change(operation) {return Err(Error::new("RuntimePreparing","startup","Дождитесь завершения операции с ядрами."));}
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
async fn bootstrap(state: host::State<'_, AppState>) -> Result<Bootstrap> {
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
async fn connect(state: host::State<'_, AppState>, id: String) -> Result<Snapshot> {
    state.diagnostics.cancel_all();
    let isolated=state.startup_smoke;
    with_core(&state, "connect", move |c| {
        let p = c.store.preferences()?;
        if !p.tun && !p.system_proxy {
            return Err(Error::new(
                "ConnectionModeRequired",
                "connect-mode",
                "Включите TUN и/или System Proxy перед подключением",
            ));
        }
        if isolated{return Err(Error::new("VerificationOnly","verification","TUN and System Proxy are disabled in verification sessions"));}
        c.connect(&id)
    })
    .await
}
async fn disconnect(state: host::State<'_, AppState>) -> Result<Snapshot> {
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
async fn log_days(state: host::State<'_, AppState>) -> Result<Vec<String>> {
    let directory = state.data_dir.clone();
    host::async_runtime::spawn_blocking(move || netpulse_core::audit::days(&directory))
        .await
        .map_err(|_| {
            Error::new(
                "LogReadFailed",
                "audit-days",
                "Не удалось прочитать каталог журналов",
            )
        })?
}
async fn log_read(
    state: host::State<'_, AppState>,
    day: String,
    category: String,
) -> Result<String> {
    let directory = state.data_dir.clone();
    host::async_runtime::spawn_blocking(move || {
        netpulse_core::audit::read(&directory, &day, &category)
    })
    .await
    .map_err(|_| Error::new("LogReadFailed", "audit-read", "Не удалось прочитать журнал"))?
}
async fn profile_transport(
    state: host::State<'_, AppState>,
    id: String,
) -> Result<netpulse_core::transport_options::ProfileTransportView> {
    with_core(&state, "profile_transport", move |c| {
        Ok(c.store.profile(&id)?.transport_view())
    })
    .await
}
async fn save_profile_transport(
    state: host::State<'_, AppState>,
    id: String,
    value: Option<netpulse_core::transport_options::ProfileTransport>,
) -> Result<()> {
    state.diagnostics.cancel_all();
    with_core(&state, "save_profile_transport", move |c| {
        c.set_profile_transport(&id, value)
    })
    .await
}
async fn profile_json(state: host::State<'_, AppState>, id: String) -> Result<String> {
    with_core(&state, "profile_json", move |c| {
        c.store.ensure_profile_editable(&id)?;
        let p = c.store.profile(&id)?;
        serde_json::to_string_pretty(p.outbound_override.as_ref().unwrap_or(&p.outbound))
            .map_err(|_| Error::new("InvalidJson", "profile-json", "Cannot encode configuration"))
    })
    .await
}
async fn save_profile_json(
    state: host::State<'_, AppState>,
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
async fn playground_processes(state:host::State<'_,AppState>) -> Result<Vec<netpulse_core::playground::Process>> {
    let request=with_core(&state,"playground-processes",|c|c.analysis_request(None,String::new())).await?;
    host::async_runtime::spawn_blocking(move || {
        let mut processes=netpulse_core::playground::processes()?;
        if let Ok(sample)=request.run(){netpulse_core::playground::attach_network(&mut processes,&sample.connections);}
        Ok(processes)
    }).await.map_err(|_|Error::new("ProcessListFailed","playground","Не удалось прочитать сетевую активность процессов"))?
}
async fn connection_analysis(
    state: host::State<'_, AppState>,
    pid: Option<u32>,
    path: String,
) -> Result<netpulse_core::playground::Analysis> {
    let request = with_core(&state, "connection_analysis", move |c| {
        c.analysis_request(pid, path)
    })
    .await?;
    host::async_runtime::spawn_blocking(move || request.run())
        .await
        .map_err(|_| {
            Error::new(
                "AnalysisFailed",
                "playground",
                "Не удалось прочитать соединения",
            )
        })?
}
async fn snapshot(state: host::State<'_, AppState>) -> Result<Snapshot> {
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
async fn import_text(state: host::State<'_, AppState>, text: String) -> Result<ImportReport> {
    let text = Zeroizing::new(text);
    with_core(&state, "import_text", move |c| import_payload(c, &text)).await
}
async fn import_clipboard(state: host::State<'_, AppState>) -> Result<ImportReport> {
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
async fn copy_proxy_endpoint(state: host::State<'_, AppState>) -> Result<bool> {
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
        .thumbnail(2048,2048)
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
#[cfg(test)]
mod qr_audit_tests {
    use super::*;
    #[test] fn updated_decoder_reads_owned_qr_in_both_polarities() {
        let bytes=include_bytes!("../tests/fixtures/import-qr.png");
        let expected="socks5://127.0.0.1:1080#Owned QR";
        assert_eq!(decode_qr(bytes).unwrap().as_str(),expected);
        let mut image=image::load_from_memory(bytes).unwrap();image.invert();
        let mut dark=std::io::Cursor::new(Vec::new());image.write_to(&mut dark,image::ImageFormat::Png).unwrap();
        assert_eq!(decode_qr(dark.get_ref()).unwrap().as_str(),expected);
    }
    #[test] fn malformed_qr_images_fail_without_panicking() {
        for bytes in [b"".as_slice(),b"not an image",b"\x89PNG\r\n\x1a\n",b"RIFF\x00\x00\x00\x00WEBP"]{assert!(decode_qr(bytes).is_err());}
    }
}
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
async fn copy_event(state: host::State<'_, AppState>, id: String) -> Result<bool> {
    with_core(&state, "copy_event", move |c| {
        let event = c.store.event(&id)?;
        copy_clipboard(&serde_json::to_string_pretty(&event).map_err(|_| {
            Error::new("CopyFailed", "event-copy", "Не удалось подготовить событие")
        })?)?;
        Ok(true)
    })
    .await
}
fn copy_ip(ip: String) -> Result<bool> {
    let ip = ip
        .parse::<std::net::IpAddr>()
        .map_err(|_| Error::new("InvalidIp", "ip-copy", "Неверный IP адрес"))?;
    copy_clipboard(&ip.to_string())?;
    Ok(true)
}
async fn import_file(state: host::State<'_, AppState>, qr: bool) -> Result<Option<ImportReport>> {
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
async fn delete_profile(state: host::State<'_, AppState>, id: String) -> Result<()> {
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
async fn add_subscription(
    state: host::State<'_, AppState>,
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
async fn copy_subscription_url(state: host::State<'_, AppState>, id: String) -> Result<bool> {
    with_core(&state, "copy_subscription_url", move |c| {
        let url = c.store.copyable_subscription_url(&id)?;
        copy_clipboard(&url)?;
        Ok(true)
    })
    .await
}
async fn save_subscription(
    state: host::State<'_, AppState>,
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
async fn import_routing() -> Result<Option<netpulse_core::routing::RoutingSettings>> {
    host::async_runtime::spawn_blocking(|| {
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
async fn export_routing(routing: netpulse_core::routing::RoutingSettings) -> Result<bool> {
    host::async_runtime::spawn_blocking(move || {
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
async fn update_subscription(
    state: host::State<'_, AppState>,
    id: String,
) -> Result<ImportReport> {
    with_core(&state, "update_subscription", move |c| {
        c.update_subscription(&id)
    })
    .await
}
async fn delete_subscription(state: host::State<'_, AppState>, id: String) -> Result<()> {
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
async fn save_preferences(
    state: host::State<'_, AppState>,
    preferences: Preferences,
) -> Result<bool> {
    state.diagnostics.cancel_all();
    let isolated = state.startup_smoke;
    with_core(&state, "save_preferences", move |c| {
        preferences.clone().normalized().validate()?;
        let old = c.store.preferences()?;
        if !isolated && old.autostart != preferences.autostart {
            desktop::autostart(preferences.autostart)?;
        }
        let autostart_changed = !isolated && old.autostart != preferences.autostart;
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
async fn quick_network(
    state: host::State<'_, AppState>,
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
        p.validate()?;
        if (tun.is_some() || system_proxy.is_some())
            && !p.tun && !p.system_proxy && c.snapshot().engine_pid.is_some() {
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
async fn quick_ui(state: host::State<'_, AppState>, patch: UiPatch) -> Result<Preferences> {
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
async fn active_latency(
    state: host::State<'_, AppState>,
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
    let mut prefs=prefs;
    prefs.ping_proxy_mode="keepalive".into();
    let method = "proxy_https_rtt".to_string();
    host::async_runtime::spawn_blocking(move || {
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
async fn check_core_updates() -> Result<Vec<netpulse_core::core_updates::CoreVersion>> {
    host::async_runtime::spawn_blocking(netpulse_core::core_updates::check)
        .await
        .map_err(|_| Error::new("CoreUpdateFailed", "core-update", "Update task failed"))
}

async fn list_core_releases(engine: String) -> Result<Vec<netpulse_core::core_releases::Release>> {
    let task=host::async_runtime::spawn_blocking(move || netpulse_core::core_releases::list(&engine));
    release_lookup_deadline(task,std::time::Duration::from_secs(12)).await
}
async fn release_lookup_deadline(task:tokio::task::JoinHandle<Result<Vec<netpulse_core::core_releases::Release>>>,limit:std::time::Duration)->Result<Vec<netpulse_core::core_releases::Release>> {
    tokio::time::timeout(limit,task).await.map_err(|_|Error::new("CoreUpdateUnavailable","core-release","GitHub не ответил за 12 секунд. Встроенные ядра доступны без загрузки; повторите проверку позже."))?
        .map_err(|_| {
            Error::new(
                "CoreUpdateFailed",
                "core-release",
                "Release lookup task failed",
            )
        })?
}
#[cfg(test)]
mod release_deadline_tests {
    use super::*;
    #[tokio::test]
    async fn blocked_os_work_cannot_delay_the_ui_release_response() {
        let task=tokio::task::spawn_blocking(||{std::thread::sleep(std::time::Duration::from_millis(200));Ok(Vec::new())});
        let before=std::time::Instant::now();
        assert_eq!(release_lookup_deadline(task,std::time::Duration::from_millis(20)).await.unwrap_err().code,"CoreUpdateUnavailable");
        assert!(before.elapsed()<std::time::Duration::from_millis(150));
    }
}
async fn update_core(
    state: host::State<'_, AppState>,
    release: netpulse_core::core_releases::Release,
) -> Result<serde_json::Value> {
    let guard=RuntimeOperation::acquire(&state.runtime_preparing)?;
    state.diagnostics.cancel_all();
    let data=state.data_dir.clone();let core=state.core.clone();
    host::async_runtime::spawn_blocking(move || {
        let _guard=guard;
        let result=(|| {
            let (profiles,prefs)={
                let mut c=core.lock().map_err(|_|Error::new("CoreUnavailable","core-update","Network core unavailable"))?;
                if c.snapshot().engine_pid.is_some(){return Err(Error::new("ConnectionInUse","core-update","Отключите VPN перед обновлением ядра"));}
                (c.store.profiles()?.iter().map(|p|c.store.profile(&p.id)).collect::<Result<Vec<_>>>()?,c.store.preferences()?)
            };
            let engine=release.engine.clone();let previous=netpulse_core::core_releases::version(&data.join("engine"),&engine);
            let exe=std::env::current_exe().map_err(|e|Error::io("core-bundle",e))?;
            netpulse_core::core_releases::install_with_bundle(release,&data,&profiles,&prefs,Some(&exe.parent().unwrap().join("cores")))?;
            let installed=netpulse_core::core_releases::executable_version(&data.join("engine").join(if engine=="xray" {"xray.exe"}else{"sing-box.exe"}),&engine)?;
            if let Ok(c)=core.lock(){c.store.log("INFO","Update","core-installed","Verified",&format!("{engine}: {previous} -> {installed}; executable version and SHA-256 verified"))?;}
            Ok(serde_json::json!({"engine":engine,"previous":previous,"installed":installed,"cache_clean":!std::fs::read_dir(data.join("core-downloads")).is_ok_and(|mut i|i.next().is_some())}))
        })();
        if let Err(e)=&result {if let Ok(c)=core.lock(){let _=c.store.log("ERROR","Update",&e.stage,&e.code,&e.message);}}
        result
    }).await.map_err(|_|Error::new("CoreTaskFailed","core-update","Core update task failed"))?
}
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
async fn activate_core_bundle(
    app: host::AppHandle,
    state: host::State<'_, AppState>,
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

async fn quit_application(
    app: host::AppHandle,
    state: host::State<'_, AppState>,
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
async fn start_latency(
    state: host::State<'_, AppState>,
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
    host::async_runtime::spawn_blocking(move || {
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
async fn start_speed(
    state: host::State<'_, AppState>,
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
    host::async_runtime::spawn_blocking(move || {
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
async fn current_diagnostic(
    state: host::State<'_, AppState>,
) -> Result<Option<netpulse_core::diagnostics::JobStatus>> {
    state.diagnostics.active()
}
async fn diagnostic_status(
    state: host::State<'_, AppState>,
    id: String,
) -> Result<netpulse_core::diagnostics::JobStatus> {
    state.diagnostics.status(&id)
}
async fn cancel_diagnostic(state: host::State<'_, AppState>, id: String) -> Result<()> {
    state.diagnostics.cancel(&id)
}
async fn diagnostic_history(
    state: host::State<'_, AppState>,
    kind: String,
) -> Result<Vec<serde_json::Value>> {
    with_core(&state, "diagnostic_history", move |c| {
        c.store.measurements(&kind)
    })
    .await
}
async fn select_profile(state: host::State<'_, AppState>, id: String) -> Result<bool> {
    state.diagnostics.cancel_all();
    with_core(&state, "select_profile", move |c| {
        c.select_profile(&id)?;
        Ok(true)
    })
    .await
}
async fn install_compatibility_runtime(state: host::State<'_, AppState>) -> Result<()> {
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
async fn manage_service(
    _app: host::AppHandle,
    state: host::State<'_, AppState>,
    install: bool,
    upgrade: Option<bool>,
) -> Result<()> {
    let resource=netpulse_core::service::installer_executable()?;
    with_core(&state, "manage_service", move |c| {
        if install {
            let directory = c.engine_path.parent().ok_or_else(||Error::new("EngineMissing","service-install","Install the network engine first"))?;
            netpulse_core::service::validate_install_source(&resource,directory)?;
        }
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
async fn read_application_catalog(
    state: host::State<'_, AppState>,
) -> Result<Option<Vec<inventory::Application>>> {
    let directory = state.data_dir.clone();
    with_core(&state, "application-catalog-read", move |_| {
        inventory::read_catalog(&directory)
    })
    .await
}
async fn save_application_catalog(
    state: host::State<'_, AppState>,
    applications: Vec<inventory::Application>,
) -> Result<()> {
    let directory = state.data_dir.clone();
    with_core(&state, "application-catalog-save", move |_| {
        inventory::write_catalog(&directory, applications)
    })
    .await
}
async fn scan_applications() -> Result<Vec<inventory::Application>> {
    host::async_runtime::spawn_blocking(inventory::scan)
        .await
        .map_err(|_| {
            Error::new(
                "ScanFailed",
                "applications-scan",
                "Application scan could not finish",
            )
        })?
}
async fn application_icon(path: String) -> Result<Option<String>> {
    host::async_runtime::spawn_blocking(move || application_icon::read(&path))
        .await
        .map_err(|_| Error::new("IconReadFailed", "applications-icon", "Icon lookup failed"))?
}
async fn choose_exe() -> Result<Option<inventory::Application>> {
    host::async_runtime::spawn_blocking(|| {
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
async fn public_ip(state: host::State<'_, AppState>) -> Result<String> {
    with_core(&state, "public_ip", |c| c.public_ip()).await
}
async fn public_ips(
    state: host::State<'_, AppState>,
) -> Result<netpulse_core::network::PublicIps> {
    let route = with_core(&state, "public_ips", |c| Ok(c.diagnostic_route())).await?;
    host::async_runtime::spawn_blocking(move || netpulse_core::network::public_ips(route))
        .await
        .map_err(|_| {
            Error::new(
                "PublicIpFailed",
                "public-ip",
                "Не удалось завершить проверку IP",
            )
        })
}
async fn install_engine(state: host::State<'_, AppState>) -> Result<()> {
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
async fn clear_logs(state: host::State<'_, AppState>) -> Result<()> {
    with_core(&state, "clear_logs", |c| c.store.clear_logs()).await
}
async fn clear_file_logs(state: host::State<'_, AppState>) -> Result<()> {
    with_core(&state, "clear_file_logs", |c| c.store.clear_file_logs()).await
}
async fn logs(state: host::State<'_, AppState>) -> Result<Vec<LogEvent>> {
    with_core(&state, "logs", |c| c.store.logs()).await
}
async fn export_data(state: host::State<'_, AppState>, kind: String) -> Result<bool> {
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
async fn restore_data(state: host::State<'_, AppState>, backup: bool) -> Result<bool> {
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
async fn delete_local_data(state: host::State<'_, AppState>) -> Result<()> {
    with_core(&state, "delete_local_data", |c| {
        c.disconnect()?;
        c.store.clear_all()
    })
    .await
}




async fn check_client_update() -> Result<serde_json::Value> {
    let result=host::async_runtime::spawn_blocking(||netpulse_core::client_updates::check(env!("CARGO_PKG_VERSION"))).await
        .map_err(|_|Error::new("UpdateTaskFailed","client-update","Update check failed"))?;
    Ok(match result { Ok(release)=>serde_json::json!({"release":release,"checked_at":netpulse_core::store::now()}),
        Err(error)=>serde_json::json!({"error":error}) })
}
async fn download_client_update(state:host::State<'_,AppState>, release:netpulse_core::client_updates::Release)->Result<()> {
    let directory=state.data_dir.clone();
    host::async_runtime::spawn_blocking(move ||netpulse_core::client_updates::download(&directory,&release)).await
        .map_err(|_|Error::new("UpdateTaskFailed","client-update","Download task failed"))?
}
async fn install_client_update(state:host::State<'_,AppState>, release:netpulse_core::client_updates::Release)->Result<()> {
    if state.startup_smoke{return Err(Error::new("VerificationOnly","client-update","Установка обновления отключена в проверочном сеансе."));}
    let guard=RuntimeOperation::acquire(&state.runtime_preparing)?;
    state.diagnostics.cancel_all();
    let directory=state.data_dir.clone();let core=state.core.clone();
    host::async_runtime::spawn_blocking(move || {
        let _guard=guard;
        netpulse_core::client_updates::revalidate(&release)?;
        netpulse_core::client_updates::verify_download(&directory,&release)?;
        netpulse_core::service::elevated_client_update(&release.tag)?;
        // The installer waits for this client to exit. Keep an existing TUN
        // available during its protected download, then disconnect normally.
        let mut c=core.lock().map_err(|_|Error::new("CoreUnavailable","client-update","Network core unavailable"))?;c.disconnect().map(|_|())
    }).await.map_err(|_|Error::new("UpdateTaskFailed","client-update","Installation task failed"))?
}
