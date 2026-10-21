use netpulse_core::{Error, Result};
use tauri::{Emitter, Manager};
pub fn claim_instance() -> bool {
    use windows_sys::Win32::{
        Foundation::{ERROR_ALREADY_EXISTS, GetLastError},
        System::Threading::CreateMutexW,
        UI::WindowsAndMessaging::*,
    };
    let name = format!(
        "Local\\NetPulse2.{}",
        netpulse_core::service::current_sid().unwrap_or_else(|_| "current-user".into())
    );
    let name = netpulse_core::service::wide(&name);
    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    if handle.is_null() {
        return false;
    }
    let existed = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    if existed {
        unsafe { windows_sys::Win32::Foundation::CloseHandle(handle) };
        let current = format!("NetPulse {}", env!("CARGO_PKG_VERSION"));
        for text in [
            &current,
            "NetPulse 3.0.1",
            "NetPulse 3.0",
            "NetPulse 2.0 — Alpha",
            "NetPulse 2.0",
        ] {
            let title = netpulse_core::service::wide(text);
            let window = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
            if !window.is_null() {
                unsafe {
                    ShowWindow(window, SW_RESTORE);
                    SetForegroundWindow(window);
                }
                if text != current {
                    show_startup_error(
                        "Уже работает другая версия NetPulse. Завершите её через значок в трее и запустите новую версию.",
                    );
                }
                break;
            }
        }
        return false;
    }
    static INSTANCE: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    let _ = INSTANCE.set(handle as usize);
    true
}
pub fn show_startup_error(message: &str) {
    let title = netpulse_core::service::wide("NetPulse — запуск");
    let mut message = message.to_string();
    if let Some(root) = std::env::var_os("LOCALAPPDATA") {
        let folder = std::path::PathBuf::from(root).join("local.netpulse.desktop.v2");
        let _ = std::fs::create_dir_all(&folder);
        let path = folder.join("startup-error.txt");
        let _ = std::fs::write(&path, &message);
        message.push_str(&format!("\n\nДиагностика: {}", path.display()));
    }
    let message = netpulse_core::service::wide(&message);
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
            std::ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            0x10,
        );
    }
}
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
        let path = std::env::current_exe().map_err(|e| Error::io("autostart", e))?;
        key.set_value(
            "NetPulse2",
            &format!("\"{}\" --background", path.to_string_lossy()),
        )
        .map_err(|e| Error::io("autostart", e))?;
    } else {
        match key.delete_value("NetPulse2") {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(Error::io("autostart", e)),
        }
    }
    Ok(())
}
pub fn tray(app: &tauri::App) -> std::result::Result<(), Box<dyn std::error::Error>> {
    use tauri::{
        menu::{Menu, MenuItem, PredefinedMenuItem},
        tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    };
    let item = |id, text| MenuItem::with_id(app, id, text, true, None::<&str>);
    let show = item("show", "Показать окно")?;
    let connection = item("connection", "Подключить / отключить")?;
    let clipboard = item("clipboard", "Добавить из буфера обмена")?;
    let qr = item("qr", "Сканировать QR из фотографии")?;
    let settings = item("settings", "Настройки")?;
    let quit = item("quit", "Выход")?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[
            &show,
            &connection,
            &clipboard,
            &qr,
            &settings,
            &separator,
            &quit,
        ],
    )?;
    let mut builder = TrayIconBuilder::with_id("netpulse-tray")
        .tooltip("NetPulse 3.0")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
            if event.id.as_ref() != "show" {
                let _ = app.emit("tray-action", event.id.as_ref());
            }
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) && let Some(window) = tray.app_handle().get_webview_window("main")
            {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}
