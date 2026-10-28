#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use netpulse_core::{Error, Result, service};
mod maintenance;
mod client_update;
use std::{
    ffi::OsString,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
};
define_windows_service!(ffi_main, service_main);
fn maintenance_exit(error: &Error) -> i32 {
    service::maintenance_exit_code(error)
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).is_some_and(|a|a=="--client-update") {
        let result=if args.len()==3{client_update::install(&args[2])}else{Err(Error::new("InstallArguments","client-update","Expected an official release tag"))};
        if let Err(e)=result{eprintln!("{e}");std::process::exit(1);}return;
    }
    if args.get(1).is_some_and(|a| a == "--self-test") {
        let result = (|| {
            if args.len() != 3 {
                return Err(Error::new(
                    "SelfTestArguments",
                    "service-self-test",
                    "Expected a new output file",
                ));
            }
            service::validate_sid(&service::current_sid()?)?;
            let directory = service::canonical_service_directory()?;
            if !directory.is_absolute() {
                return Err(Error::new(
                    "ProtectedDirectoryInvalid",
                    "service-self-test",
                    "Protected directory is not absolute",
                ));
            }
            let _ = service::security_descriptor("D:P(A;;GA;;;SY)(A;;GA;;;BA)")?;
            let result = serde_json::json!({"result":"passed","version":env!("CARGO_PKG_VERSION"),"sid_validation":true,"protected_directory_lookup":true,"security_descriptor":true,"system_changes":false});
            use std::io::Write;
            std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&args[2])
                .and_then(|mut f| {
                    f.write_all(serde_json::to_string_pretty(&result).unwrap().as_bytes())
                })
                .map_err(|e| Error::io("service-self-test", e))
        })();
        if result.is_err() {
            std::process::exit(1);
        }
        return;
    }
    if args.get(1).is_some_and(|a| a == "--installer-stop") {
        let result = std::env::current_exe()
            .map_err(|e| Error::io("service-source", e))
            .and_then(|exe| service::elevated_stop(&exe));
        std::process::exit(if result.is_ok() { 0 } else { 1 });
    }
    if args.get(1).is_some_and(|a| a == "--stop") {
        let result = if args.len() == 3 {
            stop_installed(&args[2])
        } else {
            Err(Error::new(
                "InstallArguments",
                "service-stop",
                "Expected owner SID",
            ))
        };
        std::process::exit(if result.is_ok() { 0 } else { 1 });
    }
    if args.get(1).is_some_and(|a| a == "--upgrade") {
        let result = (|| {
            if args.len() != 4 {
                return Err(Error::new(
                    "InstallArguments",
                    "service-upgrade",
                    "Expected owner and engine directory",
                ));
            }
            upgrade(&args)
        })();
        if let Err(e) = result {
            eprintln!("{e}");
            std::process::exit(maintenance_exit(&e));
        }
        return;
    }
    if args.get(1).is_some_and(|a| a == "--install") {
        if let Err(e) = install(&args) {
            eprintln!("{e}");
            std::process::exit(maintenance_exit(&e));
        }
        return;
    }
    if args.get(1).is_some_and(|a| a == "--uninstall") {
        if let Err(e) = args
            .get(2)
            .ok_or_else(|| {
                Error::new(
                    "InstallArguments",
                    "service-uninstall",
                    "Expected owner SID",
                )
            })
            .and_then(|sid| stop_installed(sid))
            .and_then(|()| uninstall())
        {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return;
    }
    if service_dispatcher::start(service::SERVICE_NAME, ffi_main).is_err() {
        std::process::exit(1);
    }
}
fn service_main(_args: Vec<OsString>) {
    let _ = run();
}
fn run() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let status =
        service_control_handler::register(service::SERVICE_NAME, move |event| match event {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                flag.store(true, Ordering::SeqCst);
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        })?;
    let make_status = |state| ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: state,
        controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::from_secs(5),
        process_id: None,
    };
    status.set_service_status(make_status(ServiceState::StartPending))?;
    let controller = Arc::new(Mutex::new(service::Controller::default()));
    let sid = service::owner_sid()?;
    let engine = service::protected_directory()?.join("sing-box.exe");
    netpulse_core::engine::verify_engine(&engine)?;
    service::recover_routes()?;
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(async {
        let mut pipe = service::create_pipe(&sid)?;
        let slots = Arc::new(tokio::sync::Semaphore::new(8));
        let mut clients = tokio::task::JoinSet::new();
        let mut poll = tokio::time::interval(Duration::from_millis(300));
        poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let polling = Arc::new(AtomicBool::new(false));
        status.set_service_status(make_status(ServiceState::Running))
            .map_err(|_| Error::new("ServiceStatusError", "service-start", "Could not report running status"))?;
        while !stop.load(Ordering::SeqCst) {
            tokio::select! {
                result = pipe.connect() => {
                    if result.is_err() { let _ = pipe.disconnect(); continue; }
                    // Keep a listener alive while independent clients finish their ACKs.
                    let replacement = service::next_pipe(&sid)?;
                    let connection = std::mem::replace(&mut pipe, replacement);
                    let Ok(permit) = slots.clone().try_acquire_owned() else { drop(connection); continue; };
                    let controller = controller.clone();
                    let sid = sid.clone();
                    let engine = engine.clone();
                    clients.spawn(async move {
                        let _permit = permit;
                        service::serve_connection(connection, &sid, controller, &engine).await;
                    });
                },
                _ = poll.tick() => {
                    // IP Helper and engine shutdown must never block the IPC reactor.
                    if !polling.swap(true, Ordering::AcqRel) {
                        let controller = controller.clone();
                        let polling = polling.clone();
                        tokio::task::spawn_blocking(move || {
                            if let Ok(mut c) = controller.try_lock() { c.poll(); }
                            polling.store(false, Ordering::Release);
                        });
                    }
                },
                Some(_) = clients.join_next(), if !clients.is_empty() => {},
            }
        }
        clients.abort_all();
        let core = controller.clone();
        tokio::task::spawn_blocking(move || {
            core.lock().map_err(|_| Error::new("ServiceCoreError", "service-stop", "Controller unavailable"))?.shutdown()
        }).await.map_err(|_| Error::new("ServiceTaskError", "service-stop", "Shutdown task failed"))??;
        Ok::<(), Error>(())
    })?;
    status.set_service_status(make_status(ServiceState::Stopped))?;
    Ok(())
}
struct InstallAttempt {
    directory: std::path::PathBuf,
    registration: Option<windows_service::service::Service>,
    committed: bool,
}
impl Drop for InstallAttempt {
    fn drop(&mut self) {
        if !self.committed {
            if let Some(service) = self.registration.take() {
                let _ = service.delete();
            }
            // Fixed root filenames were absent; existing backup directories stay untouched. No recursion.
            for name in [
                "NetPulseService.exe",
                "sing-box.exe",
                "libcronet.dll",
                "xray.exe",
                "sing-box-release.json",
                "xray-release.json",
            ] {
                let _ = std::fs::remove_file(self.directory.join(name));
            }
        }
        let _ = std::fs::remove_file(self.directory.join(".install-in-progress"));
    }
}
fn install(args: &[String]) -> Result<()> {
    use windows_service::{
        service::{ServiceAccess, ServiceErrorControl, ServiceInfo, ServiceStartType},
        service_manager::{ServiceManager, ServiceManagerAccess},
    };
    if args.len() != 4 {
        return Err(Error::new(
            "InstallArguments",
            "service-install",
            "Expected owner SID and engine source directory",
        ));
    }
    let owner = &args[2];
    service::validate_sid(owner)?;
    let _ = service::security_descriptor(&format!(
        "D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;CCLCRPLO;;;{owner})"
    ))?;
    let source = std::path::Path::new(&args[3]);
    netpulse_core::layout::local_directory(source)?;
    let approvals = netpulse_core::core_releases::authenticated_approvals(source)?;
    netpulse_core::engine::verify_engine(&source.join("sing-box.exe"))?;
    let dest = service::protected_directory()?;
    if dest.exists() {
        use std::os::windows::fs::MetadataExt;
        if dest
            .symlink_metadata()
            .map_err(|e| Error::io("service-directory", e))?
            .file_attributes()
            & 0x400
            != 0
        {
            return Err(Error::new(
                "UnsafeInstallDirectory",
                "service-install",
                "Service directory must not be a reparse point",
            ));
        }
    }
    std::fs::create_dir_all(&dest).map_err(|e| Error::io("service-directory", e))?;
    service::protect_install_path(&dest)?;
    maintenance::validate_fresh_install_directory(&dest)?;
    let manager = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    )
    .map_err(|_| {
        Error::new(
            "AdministratorRequired",
            "service-install",
            "Approve the normal Windows UAC installation prompt",
        )
    })?;
    if manager
        .open_service(service::SERVICE_NAME, ServiceAccess::QUERY_STATUS)
        .is_ok()
    {
        return Err(Error::new(
            "ServiceAlreadyInstalled",
            "service-install",
            "Uninstall the existing NetPulse service explicitly before replacing its protected files",
        ));
    }
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(dest.join(".install-in-progress"))
        .map_err(|e| Error::io("service-install-lease", e))?;
    let mut attempt = InstallAttempt {
        directory: dest.clone(),
        registration: None,
        committed: false,
    };
    std::fs::copy(
        std::env::current_exe().map_err(|e| Error::io("service-source", e))?,
        dest.join("NetPulseService.exe"),
    )
    .map_err(|e| Error::io("service-copy", e))?;
    for name in ["sing-box.exe", "libcronet.dll"] {
        std::fs::copy(source.join(name), dest.join(name))
            .map_err(|e| Error::io("service-engine-copy", e))?;
    }
    netpulse_core::core_releases::write_approvals(&dest, &approvals)?;
    netpulse_core::engine::verify_engine(&dest.join("sing-box.exe"))?;
    if source.join("xray.exe").exists() {
        netpulse_core::runtime::verify_xray(&source.join("xray.exe"))?;
        std::fs::copy(source.join("xray.exe"), dest.join("xray.exe"))
            .map_err(|e| Error::io("service-xray-copy", e))?;
        netpulse_core::runtime::verify_xray(&dest.join("xray.exe"))?;
    }
    for name in [
        "NetPulseService.exe",
        "sing-box.exe",
        "libcronet.dll",
        "xray.exe",
    ] {
        let path = dest.join(name);
        if path.exists() {
            service::protect_install_path(&path)?;
        }
    }
    let info = ServiceInfo {
        name: service::SERVICE_NAME.into(),
        display_name: "NetPulse Network Core 2".into(),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::OnDemand,
        error_control: ServiceErrorControl::Normal,
        executable_path: dest.join("NetPulseService.exe"),
        launch_arguments: vec![],
        dependencies: vec![],
        account_name: None,
        account_password: None,
    };
    let handle = manager
        .create_service(&info, ServiceAccess::ALL_ACCESS)
        .map_err(|_| {
            Error::new(
                "ServiceInstallFailed",
                "service-install",
                "Windows could not create the NetPulse service",
            )
        })?;
    attempt.registration = Some(handle);
    let handle = attempt.registration.as_ref().unwrap();
    handle.set_description("NetPulse TUN controller. Only the installed user may request validated network sessions; no arbitrary command execution.").map_err(|_|Error::new("ServiceInstallFailed","service-install","Could not set service description"))?;
    let registry = winreg::RegKey::predef(windows_sys::Win32::System::Registry::HKEY_LOCAL_MACHINE);
    let (key, _) = registry
        .create_subkey(service::PARAMETERS)
        .map_err(|e| Error::io("service-owner-save", e))?;
    key.set_value("OwnerSid", owner)
        .map_err(|e| Error::io("service-owner-save", e))?;
    let acl = service::security_descriptor(&format!(
        "D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;CCLCRPLO;;;{owner})"
    ))?;
    if unsafe {
        windows_sys::Win32::System::Services::SetServiceObjectSecurity(
            handle.raw_handle(),
            windows_sys::Win32::Security::DACL_SECURITY_INFORMATION,
            acl.pointer(),
        )
    } == 0
    {
        return Err(Error::io("service-access", std::io::Error::last_os_error()));
    }
    attempt.committed = true;
    Ok(())
}
fn upgrade(args: &[String]) -> Result<()> {
    use std::os::windows::fs::MetadataExt;
    use windows_service::{
        service::{ServiceAccess, ServiceState},
        service_manager::{ServiceManager, ServiceManagerAccess},
    };
    service::validate_sid(&args[2])?;
    let source = std::path::Path::new(&args[3]);
    netpulse_core::layout::local_directory(source)?;
    let approvals = netpulse_core::core_releases::authenticated_approvals(source)?;
    netpulse_core::engine::verify_engine(&source.join("sing-box.exe"))?;
    netpulse_core::runtime::verify_xray(&source.join("xray.exe"))?;
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
        .map_err(|_| {
            Error::new(
                "ServiceAccess",
                "service-upgrade",
                "Cannot open Windows service manager",
            )
        })?;
    let handle = match manager.open_service(
        service::SERVICE_NAME,
        ServiceAccess::QUERY_CONFIG | ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::CHANGE_CONFIG,
    ) {
        Ok(handle) => handle,
        Err(windows_service::Error::Winapi(e)) if e.raw_os_error() == Some(1060) => {
            return install(args);
        }
        Err(_) => {
            return Err(Error::new(
                "ServiceAccess",
                "service-upgrade",
                "Cannot open installed NetPulse service",
            ));
        }
    };
    let dest = service::protected_directory()?;
    let config = handle.query_config().map_err(service::service_configuration_error)?;
    if config.executable_path.to_string_lossy().trim_matches('"')
        != dest.join("NetPulseService.exe").to_string_lossy()
        || dest
            .symlink_metadata()
            .map_err(|e| Error::io("service-upgrade", e))?
            .file_attributes()
            & 0x400
            != 0
    {
        return Err(Error::new(
            "ServiceIdentity",
            "service-upgrade",
            "Unexpected service path; installation preserved",
        ));
    }
    let owner: String =
        winreg::RegKey::predef(windows_sys::Win32::System::Registry::HKEY_LOCAL_MACHINE)
            .open_subkey(service::PARAMETERS)
            .and_then(|k| k.get_value("OwnerSid"))
            .map_err(|e| Error::io("service-owner", e))?;
    if owner != args[2] {
        return Err(Error::new(
            "ServiceOwner",
            "service-upgrade",
            "Installed service belongs to another user",
        ));
    }
    // A damaged installed bundle can indicate quarantine. Do not repeatedly
    // repopulate missing files through the maintenance button.
    service::validate_service_files(&dest)?;
    if dest!=service::canonical_service_directory()? {
        return relocate_service(&args[2],source,&approvals,&dest,&handle);
    }
    let helper = std::env::current_exe().map_err(|e| Error::io("service-source",e))?;
    let mut sources = vec![("NetPulseService.exe", Some(helper))];
    for name in ["sing-box.exe", "libcronet.dll", "xray.exe"] {
        sources.push((name, Some(source.join(name))));
    }
    let mut transaction = maintenance::Upgrade::prepare(&dest, &sources, &approvals, service::protect_install_path)?;
    if handle
        .query_status()
        .map_err(|_| {
            Error::new(
                "ServiceStatus",
                "service-upgrade",
                "Cannot read service state",
            )
        })?
        .current_state
        != ServiceState::Stopped
    {
        handle.stop().map_err(|_| Error::new("ServiceStopFailed","service-upgrade","Windows не подтвердила запрос остановки службы; обновление отменено."))?;
    }
    let mut stopped = false;
    for _ in 0..100 {
        if handle
            .query_status()
            .is_ok_and(|s| s.current_state == ServiceState::Stopped)
        {
            stopped = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    if !stopped {
        return Err(Error::new(
            "ServiceStopTimeout", "service-upgrade",
            "Служба не подтвердила остановку. Обновление отменено; принудительное завершение и замена файлов не выполнялись.",
        ));
    }
    service::recover_routes()?;
    transaction.apply(service::protect_install_path, || {
        service::validate_service_files(&dest)?;
        netpulse_core::engine::verify_engine(&dest.join("sing-box.exe"))?;
        netpulse_core::runtime::verify_xray(&dest.join("xray.exe"))?;
        Ok(())
    })
}

fn relocate_service(owner: &str, source: &std::path::Path, approvals: &[netpulse_core::core_releases::Approval], old: &std::path::Path, handle: &windows_service::service::Service) -> Result<()> {
    use windows_service::service::{ServiceErrorControl,ServiceInfo,ServiceStartType};
    let dest=service::canonical_service_directory()?;
    netpulse_core::layout::prepare_install_root(&service::application_directory()?)?;
    netpulse_core::layout::normal_directory(&dest)?;
    std::fs::create_dir_all(&dest).map_err(|e|Error::io("service-relocate",e))?;
    service::protect_install_path(&dest)?;maintenance::validate_fresh_install_directory(&dest)?;
    std::fs::OpenOptions::new().create_new(true).write(true).open(dest.join(".install-in-progress")).map_err(|e|Error::io("service-relocate",e))?;
    let mut attempt=InstallAttempt{directory:dest.clone(),registration:None,committed:false};
    std::fs::copy(std::env::current_exe().map_err(|e|Error::io("service-source",e))?,dest.join("NetPulseService.exe")).map_err(|e|Error::io("service-relocate",e))?;
    for name in ["sing-box.exe","libcronet.dll","xray.exe"] {std::fs::copy(source.join(name),dest.join(name)).map_err(|e|Error::io("service-relocate",e))?;}
    netpulse_core::core_releases::write_approvals(&dest,approvals)?;
    service::validate_service_files(&dest)?;netpulse_core::engine::verify_engine(&dest.join("sing-box.exe"))?;netpulse_core::runtime::verify_xray(&dest.join("xray.exe"))?;
    for name in maintenance::FILES {if dest.join(name).exists(){service::protect_install_path(&dest.join(name))?;}}
    // Preserve the old working service until the complete new copy is verified.
    stop_installed(owner)?;service::recover_routes()?;
    let info=ServiceInfo{name:service::SERVICE_NAME.into(),display_name:"NetPulse Network Core".into(),service_type:ServiceType::OWN_PROCESS,start_type:ServiceStartType::OnDemand,error_control:ServiceErrorControl::Normal,executable_path:dest.join("NetPulseService.exe"),launch_arguments:vec![],dependencies:vec![],account_name:None,account_password:None};
    handle.change_config(&info).map_err(service::service_configuration_error)?;
    // SCM now references these files. Even if readback fails, keep both copies.
    attempt.committed=true;
    let actual=handle.query_config().map_err(service::service_configuration_error)?;
    if !actual.executable_path.to_string_lossy().trim_matches('"').eq_ignore_ascii_case(&info.executable_path.to_string_lossy()) {return Err(Error::new("ServiceInstallReadback","service-relocate","Windows не подтвердила новый путь службы; оба комплекта сохранены."));}
    for name in maintenance::FILES {
        let path=old.join(name);
        match std::fs::remove_file(&path) {Ok(())=>{},Err(e) if e.kind()==std::io::ErrorKind::NotFound=>{},Err(e)=>return Err(Error::io("service-legacy-cleanup",e))}
    }
    let _=std::fs::remove_dir(old);
    Ok(())
}

fn stop_installed(owner_sid: &str) -> Result<()> {
    use std::os::windows::fs::MetadataExt;
    use windows_service::{
        service::ServiceAccess,
        service_manager::{ServiceManager, ServiceManagerAccess},
    };
    service::validate_sid(owner_sid)?;
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
        .map_err(|_| {
            Error::new(
                "ServiceAccess",
                "service-stop",
                "Cannot open service manager",
            )
        })?;
    let handle = match manager.open_service(
        service::SERVICE_NAME,
        ServiceAccess::QUERY_CONFIG | ServiceAccess::QUERY_STATUS | ServiceAccess::STOP,
    ) {
        Ok(handle) => handle,
        Err(windows_service::Error::Winapi(e)) if e.raw_os_error() == Some(1060) => return Ok(()),
        Err(_) => {
            return Err(Error::new(
                "ServiceAccess",
                "service-stop",
                "Cannot open installed service",
            ));
        }
    };
    let dest = service::protected_directory()?;
    let config = handle.query_config().map_err(service::service_configuration_error)?;
    let expected = dest.join("NetPulseService.exe");
    if !config
        .executable_path
        .to_string_lossy()
        .trim_matches('"')
        .eq_ignore_ascii_case(&expected.to_string_lossy())
        || dest
            .symlink_metadata()
            .map_err(|e| Error::io("service-stop", e))?
            .file_attributes()
            & 0x400
            != 0
    {
        return Err(Error::new(
            "ServiceIdentity",
            "service-stop",
            "Unexpected service path; nothing stopped",
        ));
    }
    let owner: String =
        winreg::RegKey::predef(windows_sys::Win32::System::Registry::HKEY_LOCAL_MACHINE)
            .open_subkey(service::PARAMETERS)
            .and_then(|k| k.get_value("OwnerSid"))
            .map_err(|e| Error::io("service-owner", e))?;
    if owner != owner_sid {
        return Err(Error::new(
            "ServiceOwner",
            "service-stop",
            "Installed service belongs to another user",
        ));
    }
    if handle
        .query_status()
        .is_ok_and(|s| s.current_state == ServiceState::Stopped)
    {
        return service::recover_routes();
    }
    let _ = handle.stop();
    for _ in 0..100 {
        if handle
            .query_status()
            .is_ok_and(|s| s.current_state == ServiceState::Stopped)
        {
            return service::recover_routes();
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(Error::new(
        "ServiceStopTimeout", "service-stop",
        "Служба не подтвердила остановку. Закройте подключение NetPulse и повторите после перезапуска Windows; файлы сохранены.",
    ))
}
fn uninstall() -> Result<()> {
    use windows_service::{
        service::{ServiceAccess, ServiceState},
        service_manager::{ServiceManager, ServiceManagerAccess},
    };
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
        .map_err(|_| {
            Error::new(
                "AdministratorRequired",
                "service-uninstall",
                "Administrator rights required",
            )
        })?;
    let service = manager
        .open_service(
            service::SERVICE_NAME,
            ServiceAccess::STOP | ServiceAccess::QUERY_STATUS | ServiceAccess::DELETE,
        )
        .map_err(|_| {
            Error::new(
                "ServiceNotInstalled",
                "service-uninstall",
                "NetPulse service is not installed",
            )
        })?;
    if service
        .query_status()
        .map_err(|_| {
            Error::new(
                "ServiceStatusError",
                "service-uninstall",
                "Could not read service status",
            )
        })?
        .current_state
        != ServiceState::Stopped
    {
        service.stop().map_err(|_| {
            Error::new(
                "ServiceStopFailed",
                "service-uninstall",
                "Could not stop the NetPulse service",
            )
        })?;
        for _ in 0..100 {
            if service
                .query_status()
                .map_err(|_| {
                    Error::new(
                        "ServiceStatusError",
                        "service-uninstall",
                        "Could not read service status",
                    )
                })?
                .current_state
                == ServiceState::Stopped
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    if service
        .query_status()
        .map_err(|_| {
            Error::new(
                "ServiceStatusError",
                "service-uninstall",
                "Cannot verify stopped state",
            )
        })?
        .current_state
        != ServiceState::Stopped
    {
        return Err(Error::new(
            "ServiceStopTimeout",
            "service-uninstall",
            "Service has not stopped; protected files were preserved",
        ));
    }
    // Resolve the verified SCM location before deletion marks the registration
    // for removal. Querying SCM afterward can return ERROR_SERVICE_MARKED_FOR_DELETE.
    let dest = netpulse_core::service::protected_directory()?;
    service.delete().map_err(|_| {
        Error::new(
            "ServiceDeleteFailed",
            "service-uninstall",
            "Could not remove the NetPulse service registration",
        )
    })?;
    // Fixed Program Files paths only. No recursive delete or computed traversal.
    for name in [
        "sing-box.exe",
        "libcronet.dll",
        "xray.exe",
        "NetPulseService.exe",
        "sing-box-release.json",
        "xray-release.json",
    ] {
        let path = dest.join(name);
        if path.exists() {
            std::fs::remove_file(path).map_err(|e| Error::io("service-files-remove", e))?;
        }
    }
    // Remove only this installer's fixed backup filenames in UUID directories.
    // Unknown files, reparse points and unrelated directories are preserved.
    use std::os::windows::fs::MetadataExt;
    for entry in std::fs::read_dir(&dest).map_err(|e| Error::io("service-backups", e))? {
        let entry = entry.map_err(|e| Error::io("service-backups", e))?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name
            .strip_prefix("upgrade-")
            .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok())
        {
            continue;
        }
        let directory = entry.path();
        let metadata =
            std::fs::symlink_metadata(&directory).map_err(|e| Error::io("service-backups", e))?;
        if !metadata.is_dir() || metadata.file_attributes() & 0x400 != 0 {
            continue;
        }
        if directory.parent() != Some(dest.as_path()) {
            continue;
        }
        for name in [
            "NetPulseService.exe",
            "sing-box.exe",
            "libcronet.dll",
            "xray.exe",
            "previous-NetPulseService.exe",
            "previous-sing-box.exe",
            "previous-libcronet.dll",
            "previous-xray.exe",
            "sing-box-release.json",
            "xray-release.json",
            "previous-sing-box-release.json",
            "previous-xray-release.json",
        ] {
            let file = directory.join(name);
            if let Ok(metadata) = std::fs::symlink_metadata(&file)
                && metadata.is_file()
                && metadata.file_attributes() & 0x400 == 0
            {
                std::fs::remove_file(file).map_err(|e| Error::io("service-backup-remove", e))?;
            }
        }
        let _ = std::fs::remove_dir(directory);
    }
    let _ = std::fs::remove_dir(dest);
    Ok(())
}
