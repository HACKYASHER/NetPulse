//! Narrow local IPC to the Windows service. No paths, shell commands, or raw configs.
use crate::{
    Error, Result,
    model::{Preferences, Profile},
    routes,
    runtime::{EngineGroup, RuntimePlan},
};
use serde::{Deserialize, Serialize};
use std::{
    os::windows::io::AsRawHandle,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::windows::named_pipe::{ClientOptions, NamedPipeServer, ServerOptions},
};
use windows_sys::Win32::{
    Foundation::*,
    Security::{Authorization::*, *},
    System::{Pipes::*, Threading::*},
};
pub const SERVICE_NAME: &str = "NetPulseCoreQt";
pub const MAX_FRAME: usize = 256 * 1024;
pub const PARAMETERS: &str = r"SYSTEM\CurrentControlSet\Services\NetPulseCoreQt\Parameters";
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
pub fn validate_sid(sid: &str) -> Result<()> {
    if !sid.starts_with("S-1-")
        || sid.len() > 184
        || !sid
            .bytes()
            .all(|b| b.is_ascii_digit() || b == b'-' || b == b'S')
    {
        return Err(Error::new(
            "InvalidSid",
            "service-acl",
            "Invalid Windows SID",
        ));
    }
    let mut ptr = std::ptr::null_mut();
    if unsafe { ConvertStringSidToSidW(wide(sid).as_ptr(), &mut ptr) } == 0 {
        return Err(Error::io("service-sid", std::io::Error::last_os_error()));
    }
    unsafe { LocalFree(ptr as _) };
    Ok(())
}
/// Service files are owned by Administrators, never by the unelevated installing
/// user's SID. Removing write ACEs alone would still let a file owner alter DACLs.
pub fn protect_install_path(path: &Path) -> Result<()> {
    use windows_sys::Win32::Security::Authorization::{SE_FILE_OBJECT, SetNamedSecurityInfoW};
    let security = security_descriptor("D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FRFX;;;BU)")?;
    let mut dacl = std::ptr::null_mut();
    let mut present = 0;
    let mut defaulted = 0;
    if unsafe {
        GetSecurityDescriptorDacl(security.pointer(), &mut present, &mut dacl, &mut defaulted)
    } == 0
        || present == 0
    {
        return Err(Error::io(
            "service-file-acl",
            std::io::Error::last_os_error(),
        ));
    }
    let mut owner = std::ptr::null_mut();
    if unsafe { ConvertStringSidToSidW(wide("S-1-5-32-544").as_ptr(), &mut owner) } == 0 {
        return Err(Error::io(
            "service-file-owner",
            std::io::Error::last_os_error(),
        ));
    }
    let mut name = wide(&path.to_string_lossy());
    let code = unsafe {
        SetNamedSecurityInfoW(
            name.as_mut_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION
                | DACL_SECURITY_INFORMATION
                | PROTECTED_DACL_SECURITY_INFORMATION,
            owner,
            std::ptr::null_mut(),
            dacl,
            std::ptr::null_mut(),
        )
    };
    unsafe { LocalFree(owner as _) };
    if code != 0 {
        return Err(Error::new(
            "ServiceDirectorySecurity",
            "service-file-acl",
            format!("Protecting the installation returned Windows error {code}"),
        ));
    }
    Ok(())
}
pub struct Handle(pub HANDLE);
unsafe impl Send for Handle {}
impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            unsafe { CloseHandle(self.0) };
        }
    }
}
fn token_sid(token: HANDLE) -> Result<String> {
    let mut size = 0;
    unsafe { GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut size) };
    if size == 0 || size > 65536 {
        return Err(Error::io("token-user", std::io::Error::last_os_error()));
    }
    let mut bytes = vec![0u64; (size as usize).div_ceil(8)];
    if unsafe { GetTokenInformation(token, TokenUser, bytes.as_mut_ptr() as _, size, &mut size) }
        == 0
    {
        return Err(Error::io("token-user", std::io::Error::last_os_error()));
    }
    let user = unsafe { &*(bytes.as_ptr() as *const TOKEN_USER) };
    let mut string = std::ptr::null_mut();
    if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut string) } == 0 {
        return Err(Error::io("token-sid", std::io::Error::last_os_error()));
    }
    let mut n = 0;
    unsafe {
        while *string.add(n) != 0 {
            n += 1;
        }
    }
    let result = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(string, n) });
    unsafe { LocalFree(string as _) };
    Ok(result)
}
pub fn current_sid() -> Result<String> {
    let mut token = std::ptr::null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(Error::io("current-user", std::io::Error::last_os_error()));
    }
    let handle = Handle(token);
    token_sid(handle.0)
}
pub fn application_directory() -> Result<PathBuf> {
    use windows_sys::Win32::{
        System::Com::CoTaskMemFree,
        UI::Shell::{FOLDERID_ProgramFilesX64, SHGetKnownFolderPath},
    };
    let mut ptr = std::ptr::null_mut();
    let code = unsafe {
        SHGetKnownFolderPath(&FOLDERID_ProgramFilesX64, 0, std::ptr::null_mut(), &mut ptr)
    };
    if code < 0 {
        return Err(Error::new(
            "WindowsFolderError",
            "service-directory",
            format!("Program Files lookup failed ({code})"),
        ));
    }
    let mut n = 0;
    unsafe {
        while *ptr.add(n) != 0 {
            n += 1;
        }
    }
    let path = PathBuf::from(String::from_utf16_lossy(unsafe {
        std::slice::from_raw_parts(ptr, n)
    }));
    unsafe { CoTaskMemFree(ptr as _) };
    Ok(path.join("NetPulse"))
}
pub fn canonical_service_directory() -> Result<PathBuf> { Ok(application_directory()?.join("service")) }
pub fn legacy_service_directory() -> Result<PathBuf> { Ok(application_directory()?.with_file_name("NetPulse Core Qt")) }
/// Accept only the two installer-owned SCM locations while upgrading old clients.
pub fn protected_directory() -> Result<PathBuf> {
    use windows_service::{service::ServiceAccess,service_manager::{ServiceManager,ServiceManagerAccess}};
    let canonical=canonical_service_directory()?;
    let manager=ServiceManager::local_computer(None::<&str>,ServiceManagerAccess::CONNECT).map_err(service_error)?;
    let handle=match manager.open_service(SERVICE_NAME,ServiceAccess::QUERY_CONFIG) {
        Ok(h)=>h,
        Err(windows_service::Error::Winapi(e)) if e.raw_os_error()==Some(1060)=>return Ok(canonical),
        Err(e)=>return Err(service_error(e)),
    };
    let config=handle.query_config().map_err(service_configuration_error)?;
    let actual=config.executable_path.to_string_lossy();
    for directory in [canonical,legacy_service_directory()?] {
        if actual.trim_matches('"').eq_ignore_ascii_case(&directory.join("NetPulseService.exe").to_string_lossy()) {
            crate::layout::normal_directory(&directory)?;return Ok(directory);
        }
    }
    Err(Error::new("ServiceIdentityMismatch","service-directory","Путь службы не принадлежит установке NetPulse. Обслуживание отменено."))
}
pub fn owner_sid() -> Result<String> {
    let root = winreg::RegKey::predef(windows_sys::Win32::System::Registry::HKEY_LOCAL_MACHINE);
    let key = root
        .open_subkey(PARAMETERS)
        .map_err(|e| Error::io("service-owner", e))?;
    key.get_value("OwnerSid")
        .map_err(|e| Error::io("service-owner", e))
}
fn journal_path() -> Result<PathBuf> {
    Ok(protected_directory()?.join("owned-tun.json"))
}
fn save_owned(owned: &routes::NetworkView) -> Result<()> {
    use std::io::Write;
    let path = journal_path()?;
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|e| Error::io("tun-journal", e))?;
        file.write_all(&serde_json::to_vec(owned).map_err(|_| {
            Error::new(
                "TunJournalError",
                "tun-journal",
                "Could not encode owned route snapshot",
            )
        })?)
        .and_then(|_| file.sync_all())
        .map_err(|e| Error::io("tun-journal", e))?;
        drop(file);
        std::fs::rename(&temporary, path).map_err(|e| Error::io("tun-journal", e))
    })();
    let _ = std::fs::remove_file(temporary);
    result
}
fn remove_journal() -> Result<()> {
    match std::fs::remove_file(journal_path()?) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Error::io("tun-journal-remove", e)),
    }
}
pub fn recover_routes() -> Result<()> {
    let path = journal_path()?;
    if !path.exists() {
        return Ok(());
    }
    let bytes = std::fs::read(path).map_err(|e| Error::io("tun-journal", e))?;
    if bytes.len() > 1024 * 1024 {
        return Err(Error::new(
            "TunJournalInvalid",
            "tun-recovery",
            "Owned route journal is too large",
        ));
    }
    let owned: routes::NetworkView = serde_json::from_slice(&bytes).map_err(|_| {
        Error::new(
            "TunJournalInvalid",
            "tun-recovery",
            "Invalid owned route journal",
        )
    })?;
    routes::cleanup_owned(&owned)?;
    if routes::inspect()?
        .routes
        .iter()
        .any(|r| owned.interfaces.iter().any(|i| i.luid == r.luid))
    {
        return Err(Error::new(
            "TunCleanupIncomplete",
            "tun-recovery",
            "Modified routes on the old interface were preserved; inspect Windows routing before retrying",
        ));
    }
    remove_journal()
}
/// Called only from an explicit GUI installation action. Windows displays UAC.
pub fn elevated_install(
    installer: &Path,
    engine_directory: &Path,
    install: bool,
    upgrade: bool,
) -> Result<()> {
    if install {
        let state=installation_status()?;
        if let Some(error)=state.error.filter(|e|matches!(e.code.as_str(),"ServiceRegistrationDamaged"|"ServiceRestartRequired")) { return Err(error); }
    }
    if install { validate_install_source(installer, engine_directory)?; }
    else { readable_service_component(installer)?; }
    let parameters = if install {
        format!(
            "{} {} \"{}\"",
            if upgrade { "--upgrade" } else { "--install" },
            current_sid()?,
            engine_directory.to_string_lossy()
        )
    } else {
        format!("--uninstall {}", current_sid()?)
    };
    elevated_parameters(installer, &parameters)?;
    if install { validate_service_files(&protected_directory()?)?; }
    let s = status()?;
    if s.installed != install {
        return Err(Error::new(
            "ServiceInstallReadback",
            "service-install",
            "Windows service state differs from the installation result",
        ));
    }
    Ok(())
}
fn readable_service_component(path: &Path) -> Result<()> {
    use std::os::windows::fs::MetadataExt;
    let unavailable = |code: i32| Error::new(
        "ServiceComponentsUnavailable", "service-files",
        format!("Файл службы {} отсутствует или недоступен (Windows {code}). Операция остановлена. Проверьте отчёт антивируса; при срабатывании не повторяйте восстановление до проверки файла.",path.file_name().unwrap_or_default().to_string_lossy()),
    );
    let metadata = std::fs::symlink_metadata(path).map_err(|e|unavailable(e.raw_os_error().unwrap_or(0)))?;
    if !metadata.is_file() || metadata.len()==0 || metadata.file_attributes() & 0x400 != 0 { return Err(unavailable(0)); }
    std::fs::File::open(path).map_err(|e|unavailable(e.raw_os_error().unwrap_or(0)))?;
    Ok(())
}
pub fn validate_service_files(directory: &Path) -> Result<()> {
    for name in ["NetPulseService.exe","sing-box.exe","libcronet.dll","xray.exe"] {
        readable_service_component(&directory.join(name))?;
    }
    Ok(())
}
pub fn validate_install_source(installer: &Path, directory: &Path) -> Result<()> {
    readable_service_component(installer)?;
    crate::engine::verify_engine(&directory.join("sing-box.exe"))?;
    crate::runtime::verify_xray(&directory.join("xray.exe"))?;
    Ok(())
}
/// Installer preparation: stop only the verified service owned by this user.
/// Invoked from a staged current-version helper before any application overwrite.
pub fn elevated_stop(installer: &Path) -> Result<()> {
    if !installation_status()?.installed {
        return Ok(());
    }
    elevated_parameters(installer, &format!("--stop {}", current_sid()?))?;
    if installation_status()?.running {
        return Err(Error::new(
            "ServiceStopTimeout",
            "service-stop",
            "Service is still running",
        ));
    }
    Ok(())
}
fn elevated_parameters(installer: &Path, parameters: &str) -> Result<()> {
    use windows_sys::Win32::{System::Threading::*, UI::Shell::*};
    let exe = wide(&installer.to_string_lossy());
    let verb = wide("runas");
    let params = wide(parameters);
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of_val(&info) as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
    info.lpVerb = verb.as_ptr();
    info.lpFile = exe.as_ptr();
    info.lpParameters = params.as_ptr();
    info.nShow = 0;
    if unsafe { ShellExecuteExW(&mut info) } == 0 {
        let e = std::io::Error::last_os_error();
        return Err(if e.raw_os_error() == Some(1223) {
            Error::new(
                "UserCancelled",
                "service-install",
                "Windows UAC installation was cancelled",
            )
        } else {
            Error::io("service-elevation", e)
        });
    }
    let process = Handle(info.hProcess);
    if process.0.is_null() {
        return Err(Error::new(
            "ServiceInstallerFailed",
            "service-install",
            "Windows did not return the installation process",
        ));
    }
    if unsafe { WaitForSingleObject(process.0, 180000) } != WAIT_OBJECT_0 {
        return Err(Error::new(
            "ServiceInstallTimeout",
            "service-install",
            "Installation is still pending; check Windows UAC before retrying",
        ));
    }
    let mut code = 1;
    if unsafe { GetExitCodeProcess(process.0, &mut code) } == 0 {
        return Err(Error::io(
            "service-install-result",
            std::io::Error::last_os_error(),
        ));
    }
    maintenance_result(code)
}
fn maintenance_result(code: u32) -> Result<()> {
    if code == 20 {
        return Err(Error::new("ServiceUpdateInterrupted","service-install","Обслуживание службы остановлено: компонент отсутствует, заблокирован или изменён. Проверьте отчёт антивируса; автоматическое восстановление файлов не выполняется."));
    }
    if code == 21 {
        return Err(Error::new("ServiceStopTimeout","service-install","Служба не подтвердила остановку. Обновление отменено, файлы сохранены. Закройте NetPulse и повторите после перезапуска Windows."));
    }
    if code == 22 { return Err(service_configuration_error(windows_service::Error::Winapi(std::io::Error::from_raw_os_error(2)))); }
    if code == 23 { return Err(Error::new("UserCancelled","service-install","Установка службы отменена в окне подтверждения Windows.")); }
    if code == 24 { return Err(service_error(windows_service::Error::Winapi(std::io::Error::from_raw_os_error(1072)))); }
    if code != 0 {
        return Err(Error::new(
            "ServiceInstallFailed",
            "service-install",
            format!(
                "Windows service installer exited with code {code}; see installation diagnostics"
            ),
        ));
    }
    Ok(())
}
pub fn maintenance_exit_code(error: &Error) -> i32 {
    match error.code.as_str() {
        "ServiceComponentsUnavailable" | "ServiceUpdateInterrupted" | "RuntimeComponentUnavailable" | "ServiceSecurityBlocked" | "ServiceFilesMissing" => 20,
        "ServiceStopTimeout" | "ServiceStopFailed" => 21,
        "ServiceRegistrationDamaged" => 22,
        "UserCancelled" => 23,
        "ServiceRestartRequired" => 24,
        _ => 1,
    }
}
fn pipe_name(sid: &str) -> String {
    format!(r"\\.\pipe\NetPulse.CoreQt.{sid}")
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Status {},
    Diagnostic {
        session: String,
    },
    Start {
        profile: Box<Profile>,
        preferences: Box<Preferences>,
        session: String,
        api_port: u16,
        probe_ports: [u16; 2],
        api_secret: String,
        proxy_secret: String,
    },
    Stop {
        session: String,
    },
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServiceStatus {
    pub installed: bool,
    pub running: bool,
    pub engine_pid: Option<u32>,
    pub owned_by_caller: bool,
    pub tun_ready: bool,
    pub version: String,
    pub error: Option<Error>,
    #[serde(default)]
    pub diagnostic: String,
}
#[derive(Serialize, Deserialize)]
pub struct Reply {
    pub status: Option<ServiceStatus>,
    pub error: Option<Error>,
}
fn service_error(e: windows_service::Error) -> Error {
    let code = match e {
        windows_service::Error::Winapi(e) => e.raw_os_error(),
        _ => None,
    };
    if matches!(code, Some(2 | 3)) {
        return Error::new("ServiceFilesMissing","service-control","Windows не нашла файл или путь службы NetPulse. Проверьте отчёт антивируса; повторная установка заблокированных файлов не выполняется автоматически.");
    }
    if code==Some(1072) {
        return Error::new("ServiceRestartRequired","service-control","Windows ещё завершает удаление службы NetPulse. Закройте клиент и установщик; если состояние сохраняется, перезапустите Windows перед установкой службы.");
    }
    if code == Some(5) {
        return Error::new("ServiceAccessDenied", "service-control", "Windows отказала в доступе к службе NetPulse (код 5). Проверьте доступность файлов и отчёт антивируса. Повторное обновление не выполняется автоматически.");
    }
    if matches!(code, Some(225 | 226)) {
        return Error::new("ServiceSecurityBlocked", "service-control", format!("Windows заблокировала файл службы по результатам проверки безопасности (код {}). Проверьте отчёт антивируса; запуск остановлен.",code.unwrap()));
    }
    Error::new(
        "WindowsServiceError",
        "service-control",
        format!("Windows service operation failed ({code:?})"),
    )
}
pub fn service_configuration_error(error: windows_service::Error) -> Error {
    if matches!(&error,windows_service::Error::Winapi(e) if matches!(e.raw_os_error(),Some(2|3))) {
        let code=match &error {windows_service::Error::Winapi(e)=>e.raw_os_error().unwrap_or(2),_=>2};
        Error::new("ServiceRegistrationDamaged","service-config",format!("Windows ещё видит службу NetPulse, но не может прочитать её регистрацию (код {code}: файл или запись не найдены). Завершите обработку обнаружения антивирусом и перезапустите Windows. Повторная установка службы сейчас отменена."))
    } else { service_error(error) }
}
pub fn status() -> Result<ServiceStatus> {
    let state = installation_status()?;
    if state.running && state.error.is_none() {
        call_once(&Request::Status {}, Duration::from_millis(750))
    } else {
        Ok(state)
    }
}
/// Read SCM without depending on a possibly older or unavailable IPC server.
pub fn installation_status() -> Result<ServiceStatus> {
    use windows_service::{
        service::ServiceAccess,
        service_manager::{ServiceManager, ServiceManagerAccess},
    };
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
        .map_err(service_error)?;
    let service = match manager.open_service(SERVICE_NAME, ServiceAccess::QUERY_STATUS | ServiceAccess::QUERY_CONFIG) {
        Ok(s) => s,
        Err(windows_service::Error::Winapi(e)) if e.raw_os_error() == Some(1060) => {
            return Ok(ServiceStatus {
                installed: false,
                running: false,
                engine_pid: None,
                owned_by_caller: false,
                tun_ready: false,
                version: env!("CARGO_PKG_VERSION").into(),
                error: None,
                diagnostic: String::new(),
            });
        }
        Err(e) => return Err(service_error(e)),
    };
    let s = service.query_status().map_err(service_error)?;
    let configuration_error=service.query_config().err().map(service_configuration_error);
    if s.current_state != windows_service::service::ServiceState::Running {
        return Ok(ServiceStatus {
            installed: true,
            running: false,
            engine_pid: None,
            owned_by_caller: false,
            tun_ready: false,
            version: env!("CARGO_PKG_VERSION").into(),
            error: configuration_error,
            diagnostic: String::new(),
        });
    }
    Ok(ServiceStatus {
        installed: true,
        running: true,
        engine_pid: None,
        owned_by_caller: false,
        tun_ready: false,
        version: String::new(),
        error: configuration_error,
        diagnostic: String::new(),
    })
}
fn expected_service_pid() -> Result<u32> {
    use windows_service::{
        service::ServiceAccess,
        service_manager::{ServiceManager, ServiceManagerAccess},
    };
    let m = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
        .map_err(service_error)?;
    let s = m
        .open_service(
            SERVICE_NAME,
            ServiceAccess::QUERY_STATUS | ServiceAccess::QUERY_CONFIG,
        )
        .map_err(service_error)?;
    let config = s.query_config().map_err(service_configuration_error)?;
    let expected = protected_directory()?.join("NetPulseService.exe");
    // SCM quotes paths containing spaces. Never trust an unrelated service PID.
    let actual = config.executable_path.to_string_lossy();
    if actual.trim_matches('"') != expected.to_string_lossy() {
        return Err(Error::new(
            "ServiceIdentityMismatch",
            "service-auth",
            "Service executable path does not match the protected NetPulse installation",
        ));
    }
    s.query_status()
        .map_err(service_error)?
        .process_id
        .ok_or_else(|| {
            Error::new(
                "ServiceNotRunning",
                "service-auth",
                "NetPulse service is not running",
            )
        })
}
pub fn ensure_started() -> Result<()> {
    use windows_service::{
        service::{ServiceAccess, ServiceState},
        service_manager::{ServiceManager, ServiceManagerAccess},
    };
    let m = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
        .map_err(service_error)?;
    let service = m
        .open_service(
            SERVICE_NAME,
            ServiceAccess::START | ServiceAccess::QUERY_STATUS | ServiceAccess::QUERY_CONFIG,
        )
        .map_err(|error| {
            if matches!(&error, windows_service::Error::Winapi(e) if e.raw_os_error()==Some(1060)) { Error::new(
                "ServiceNotInstalled",
                "tun-prepare",
                "Install the NetPulse Windows service from Settings before enabling TUN",
            ) } else { service_error(error) }
        })?;
    let directory = protected_directory()?;
    let config = service.query_config().map_err(service_configuration_error)?;
    if !config.executable_path.to_string_lossy().trim_matches('"').eq_ignore_ascii_case(&directory.join("NetPulseService.exe").to_string_lossy()) {
        return Err(Error::new("ServiceIdentityMismatch","tun-prepare","Путь службы отличается от защищённой установки NetPulse; запуск отменён."));
    }
    validate_service_files(&directory)?;
    if service.query_status().map_err(service_error)?.current_state != ServiceState::Running {
        service.start::<&str>(&[]).map_err(service_error)?;
    }
    for _ in 0..80 {
        if service.query_status().map_err(service_error)?.current_state == ServiceState::Running {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err(Error::new(
        "ServiceStartTimeout",
        "tun-prepare",
        "NetPulse service did not enter the running state",
    ))
}
pub fn call(request: &Request, timeout: Duration) -> Result<ServiceStatus> {
    let mut last = None;
    for attempt in 0..3 {
        match call_once(request, timeout) {
            Ok(status) => return Ok(status),
            Err(e) => {
                let retry = ["IoError", "ServiceTimeout", "IpcTimeout"].contains(&e.code.as_str());
                last = Some(e);
                if !retry || attempt == 2 {
                    break;
                }
                std::thread::sleep(Duration::from_millis(80));
            }
        }
    }
    Err(last.unwrap())
}
fn call_once(request: &Request, timeout: Duration) -> Result<ServiceStatus> {
    let expected_pid = expected_service_pid()?;
    let sid = current_sid()?;
    if owner_sid()? != sid {
        return Err(Error::new(
            "ServiceOwnerMismatch",
            "service-auth",
            "This service is installed for a different Windows user",
        ));
    }
    let bytes = zeroize::Zeroizing::new(serde_json::to_vec(request).map_err(|_| {
        Error::new(
            "IpcInvalid",
            "service-request",
            "Could not encode service request",
        )
    })?);
    if bytes.len() > MAX_FRAME {
        return Err(Error::new(
            "IpcLimit",
            "service-request",
            "Service request exceeds 256 KiB",
        ));
    }
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| Error::io("service-runtime", e))?;
    rt.block_on(async {
        tokio::time::timeout(timeout, async {
            let client = loop {
                match ClientOptions::new().open(pipe_name(&sid)) {
                    Ok(c) => break c,
                    Err(e) if e.raw_os_error() == Some(231) || e.raw_os_error() == Some(2) => {
                        tokio::time::sleep(Duration::from_millis(30)).await
                    }
                    Err(e) => return Err(Error::io("service-connect", e)),
                }
            };
            let mut server_pid = 0;
            if unsafe { GetNamedPipeServerProcessId(client.as_raw_handle(), &mut server_pid) } == 0
                || server_pid != expected_pid
            {
                return Err(Error::new(
                    "ServiceIdentityMismatch",
                    "service-auth",
                    "IPC server PID does not match Windows SCM",
                ));
            }
            let mut client = client;
            let mut frame = zeroize::Zeroizing::new(Vec::with_capacity(bytes.len() + 4));
            frame.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            frame.extend_from_slice(&bytes);
            client
                .write_all(&frame)
                .await
                .map_err(|e| Error::io("service-write", e))?;
            let n = client
                .read_u32_le()
                .await
                .map_err(|e| Error::io("service-read", e))? as usize;
            if n > MAX_FRAME {
                return Err(Error::new(
                    "IpcLimit",
                    "service-read",
                    "Oversized service response",
                ));
            }
            let mut data = vec![0; n];
            client
                .read_exact(&mut data)
                .await
                .map_err(|e| Error::io("service-read", e))?;
            let reply: Reply = serde_json::from_slice(&data).map_err(|_| {
                Error::new("IpcInvalid", "service-read", "Invalid service response")
            })?;
            // DisconnectNamedPipe discards unread bytes. Acknowledge the entire
            // response before the service recycles this pipe instance.
            client
                .write_all(&[0x06])
                .await
                .map_err(|e| Error::io("service-ack", e))?;
            if let Some(e) = reply.error {
                return Err(e);
            }
            reply
                .status
                .ok_or_else(|| Error::new("IpcInvalid", "service-read", "Missing service status"))
        })
        .await
        .map_err(|_| {
            Error::new(
                "ServiceTimeout",
                "service-ipc",
                "NetPulse service request timed out",
            )
        })?
    })
}
struct ServiceMonitor {
    latest: Result<ServiceStatus>,
    at: std::time::Instant,
}
pub struct ServiceEngine {
    monitor: std::sync::Arc<std::sync::Mutex<ServiceMonitor>>,
    monitor_stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    session: String,
    pid: u32,
    active: bool,
}
impl ServiceEngine {
    pub fn start(
        profile: &Profile,
        prefs: &Preferences,
        api_port: u16,
        probe_ports: [u16; 2],
        api_secret: &str,
        proxy_secret: &str,
    ) -> Result<Self> {
        ensure_started()?;
        if status()?.version != env!("CARGO_PKG_VERSION") {
            return Err(Error::new(
                "ServiceUpgradeRequired",
                "tun-prepare",
                "В настройках обновите службу до версии клиента (UAC). Старая служба не поддерживает новые настройки.",
            ));
        }
        let session = uuid::Uuid::new_v4().to_string();
        let status = call_once(
            &Request::Start {
                profile: Box::new(profile.clone()),
                preferences: Box::new(prefs.clone()),
                session: session.clone(),
                api_port,
                probe_ports,
                api_secret: api_secret.into(),
                proxy_secret: proxy_secret.into(),
            },
            Duration::from_secs(30),
        );
        let status = match status {
            Ok(status) => status,
            Err(error) => {
                // A lost response must not abandon a session owned by this GUI.
                let _ = call_once(
                    &Request::Stop {
                        session: session.clone(),
                    },
                    Duration::from_secs(8),
                );
                return Err(error);
            }
        };
        if !status.tun_ready || status.engine_pid.is_none() {
            let _ = call_once(
                &Request::Stop {
                    session: session.clone(),
                },
                Duration::from_secs(8),
            );
            return Err(Error::new(
                "TunNotReady",
                "tun-ready",
                "The virtual interface and owned routes were not detected",
            ));
        }
        let monitor = std::sync::Arc::new(std::sync::Mutex::new(ServiceMonitor {
            latest: Ok(status.clone()),
            at: std::time::Instant::now(),
        }));
        let monitor_stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let shared = monitor.clone();
        let stop = monitor_stop.clone();
        let lease = session.clone();
        std::thread::spawn(move || {
            while !stop.load(std::sync::atomic::Ordering::Acquire) {
                let mut result = call_once(
                    &Request::Diagnostic {
                        session: lease.clone(),
                    },
                    Duration::from_millis(1500),
                );
                if result.as_ref().is_err_and(|e| e.code == "LeaseMismatch") {
                    result = call_once(&Request::Status {}, Duration::from_millis(1500));
                }
                if let Ok(mut state) = shared.lock() {
                    state.latest = result;
                    state.at = std::time::Instant::now();
                }
                for _ in 0..20 {
                    if stop.load(std::sync::atomic::Ordering::Acquire) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        });
        Ok(Self {
            monitor,
            monitor_stop,
            session,
            pid: status.engine_pid.unwrap(),
            active: true,
        })
    }
    pub fn diagnostic(&self) -> String {
        self.monitor
            .lock()
            .ok()
            .and_then(|s| s.latest.as_ref().ok().map(|v| v.diagnostic.clone()))
            .unwrap_or_default()
    }
    pub fn pid(&self) -> u32 {
        self.pid
    }
    pub fn alive(&mut self) -> Result<bool> {
        if !self.active {
            return Ok(false);
        }
        let cached = self.monitor.lock().map_err(|_| {
            Error::new(
                "ServiceMonitorUnavailable",
                "service-monitor",
                "Service monitor unavailable",
            )
        })?;
        if cached.at.elapsed() > Duration::from_secs(10) {
            return Err(Error::new(
                "ServiceTimeout",
                "service-monitor",
                "Service monitoring is stale; use connection recovery",
            ));
        }
        let s = cached.latest.as_ref().map_err(Clone::clone)?;
        if !s.installed || !s.running {
            return Ok(false);
        }
        Ok(s.owned_by_caller && s.engine_pid == Some(self.pid))
    }
    pub fn stop(&mut self) -> Result<()> {
        if self.active {
            if let Err(error) = call_once(
                &Request::Stop {
                    session: self.session.clone(),
                },
                Duration::from_secs(8),
            ) {
                let stopped = installation_status().is_ok_and(|s| !s.running);
                let released = !stopped
                    && call_once(&Request::Status {}, Duration::from_millis(1500))
                        .is_ok_and(|s| s.engine_pid != Some(self.pid) && s.error.is_none());
                if !stopped && !released {
                    return Err(error);
                }
            }
            self.monitor_stop
                .store(true, std::sync::atomic::Ordering::Release);
            self.active = false;
        }
        Ok(())
    }
}
impl Drop for ServiceEngine {
    fn drop(&mut self) {
        let _ = self.stop();
        self.monitor_stop
            .store(true, std::sync::atomic::Ordering::Release);
    }
}

struct SecurityDescriptor(PSECURITY_DESCRIPTOR);
impl Drop for SecurityDescriptor {
    fn drop(&mut self) {
        unsafe { LocalFree(self.0 as _) };
    }
}
pub fn security_descriptor(sddl: &str) -> Result<HandleSecurity> {
    let mut ptr = std::ptr::null_mut();
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            wide(sddl).as_ptr(),
            1,
            &mut ptr,
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(Error::io("service-acl", std::io::Error::last_os_error()));
    }
    Ok(HandleSecurity {
        descriptor: SecurityDescriptor(ptr),
    })
}
pub struct HandleSecurity {
    descriptor: SecurityDescriptor,
}
impl HandleSecurity {
    pub fn pointer(&self) -> PSECURITY_DESCRIPTOR {
        self.descriptor.0
    }
}
/// Disconnect only after the client has consumed the complete response.
pub async fn reply_ack(pipe: &mut NamedPipeServer, reply: &Reply) -> std::io::Result<()> {
    let bytes = serde_json::to_vec(reply).map_err(std::io::Error::other)?;
    pipe.write_u32_le(bytes.len() as u32).await?;
    pipe.write_all(&bytes).await?;
    if pipe.read_u8().await? != 0x06 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Invalid acknowledgement",
        ));
    }
    Ok(())
}
/// Each pipe has its own bounded read/write lifetime. A stalled reader cannot
/// serialize other clients or keep the service's reactor from polling STOP.
pub async fn serve_connection(
    mut pipe: NamedPipeServer,
    sid: &str,
    controller: std::sync::Arc<std::sync::Mutex<Controller>>,
    engine: &Path,
) {
    let request = tokio::time::timeout(Duration::from_secs(3), async {
        let n = pipe.read_u32_le().await? as usize;
        if n > MAX_FRAME {
            return Err(std::io::Error::other("Frame too large"));
        }
        let mut bytes = zeroize::Zeroizing::new(vec![0; n]);
        pipe.read_exact(&mut bytes).await?;
        Ok(bytes)
    })
    .await;
    let result = match request {
        Ok(Ok(bytes)) => match authenticate_client(&pipe, sid) {
            Ok((pid, process)) => match serde_json::from_slice::<Request>(&bytes) {
                Ok(request) => {
                    let engine = engine.to_path_buf();
                    tokio::task::spawn_blocking(move || {
                        let deadline = std::time::Instant::now() + Duration::from_millis(750);
                        loop {
                            match controller.try_lock() {
                                Ok(mut c) => return c.handle(request, pid, process, &engine),
                                Err(std::sync::TryLockError::Poisoned(_)) => {
                                    return Err(Error::new(
                                        "ServiceCoreError",
                                        "service-request",
                                        "Controller unavailable",
                                    ));
                                }
                                Err(_) if std::time::Instant::now() < deadline => {
                                    std::thread::sleep(Duration::from_millis(10))
                                }
                                Err(_) => {
                                    return Err(Error::new(
                                        "ServiceBusy",
                                        "service-request",
                                        "Another connection operation is still finishing",
                                    ));
                                }
                            }
                        }
                    })
                    .await
                    .unwrap_or_else(|_| {
                        Err(Error::new(
                            "ServiceTaskError",
                            "service-request",
                            "Controller task failed",
                        ))
                    })
                }
                Err(_) => Err(Error::new(
                    "InvalidServiceRequest",
                    "service-request",
                    "Malformed or unsupported command",
                )),
            },
            Err(e) => Err(e),
        },
        _ => Err(Error::new(
            "IpcTimeout",
            "service-read",
            "Incomplete service request",
        )),
    };
    let reply = match result {
        Ok(status) => Reply {
            status: Some(status),
            error: None,
        },
        Err(error) => Reply {
            status: None,
            error: Some(error),
        },
    };
    let _ = tokio::time::timeout(Duration::from_secs(2), reply_ack(&mut pipe, &reply)).await;
    let _ = pipe.disconnect();
}
pub fn create_pipe(sid: &str) -> Result<NamedPipeServer> {
    create_pipe_instance(sid, true)
}
pub fn next_pipe(sid: &str) -> Result<NamedPipeServer> {
    create_pipe_instance(sid, false)
}
fn create_pipe_instance(sid: &str, first: bool) -> Result<NamedPipeServer> {
    validate_sid(sid)?;
    let security = security_descriptor(&format!("D:P(A;;GA;;;SY)(A;;GRGW;;;{sid})"))?;
    let mut attrs = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: security.pointer(),
        bInheritHandle: 0,
    };
    unsafe {
        ServerOptions::new()
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .max_instances(10)
            .in_buffer_size(MAX_FRAME as u32)
            .out_buffer_size(MAX_FRAME as u32)
            .create_with_security_attributes_raw(pipe_name(sid), &mut attrs as *mut _ as _)
    }
    .map_err(|e| Error::io("service-pipe", e))
}
pub fn authenticate_client(pipe: &NamedPipeServer, expected_sid: &str) -> Result<(u32, Handle)> {
    let mut pid = 0;
    if unsafe { GetNamedPipeClientProcessId(pipe.as_raw_handle(), &mut pid) } == 0 {
        return Err(Error::io("client-pid", std::io::Error::last_os_error()));
    }
    if unsafe { ImpersonateNamedPipeClient(pipe.as_raw_handle()) } == 0 {
        return Err(Error::io("client-auth", std::io::Error::last_os_error()));
    }
    struct Revert;
    impl Drop for Revert {
        fn drop(&mut self) {
            unsafe { RevertToSelf() };
        }
    }
    let guard = Revert;
    let mut token = std::ptr::null_mut();
    if unsafe { OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, 1, &mut token) } == 0 {
        return Err(Error::io("client-token", std::io::Error::last_os_error()));
    }
    let token = Handle(token);
    let sid = token_sid(token.0)?;
    drop(guard);
    if sid != expected_sid {
        return Err(Error::new(
            "ClientIdentityMismatch",
            "service-auth",
            "Client SID does not match the installed owner",
        ));
    }
    let handle = Handle(unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            0,
            pid,
        )
    });
    if handle.0.is_null() {
        return Err(Error::io("client-process", std::io::Error::last_os_error()));
    }
    let mut process_token = std::ptr::null_mut();
    if unsafe { OpenProcessToken(handle.0, TOKEN_QUERY, &mut process_token) } == 0 {
        return Err(Error::io(
            "client-process-token",
            std::io::Error::last_os_error(),
        ));
    }
    let process_token = Handle(process_token);
    if token_sid(process_token.0)? != expected_sid {
        return Err(Error::new(
            "ClientIdentityMismatch",
            "service-auth",
            "Client process token differs from pipe token",
        ));
    }
    Ok((pid, handle))
}
#[derive(Default)]
struct TunHealth {
    checked: Option<std::time::Instant>,
    ready: bool,
    missing_since: Option<std::time::Instant>,
}
impl TunHealth {
    fn refresh(&mut self, now: std::time::Instant, inspect: impl FnOnce() -> Result<bool>) {
        if self
            .checked
            .is_some_and(|at| now.duration_since(at) < Duration::from_secs(1))
        {
            return;
        }
        self.checked = Some(now);
        match inspect() {
            Ok(true) => {
                self.ready = true;
                self.missing_since = None;
            }
            Ok(false) => {
                self.ready = false;
                self.missing_since.get_or_insert(now);
            }
            Err(_) => {
                // An IP Helper read failure does not prove the owned routes vanished.
                self.ready = false;
                self.missing_since = None;
            }
        }
    }
    fn lost(&self, now: std::time::Instant) -> bool {
        self.missing_since
            .is_some_and(|at| now.duration_since(at) >= Duration::from_secs(5))
    }
}
#[derive(Default)]
pub struct Controller {
    engine: Option<EngineGroup>,
    plan: Option<RuntimePlan>,
    owner: Option<Handle>,
    owner_pid: u32,
    session: String,
    owned: Option<routes::NetworkView>,
    error: Option<Error>,
    tun_health: TunHealth,
}
impl Controller {
    fn inspect_tun(owned: &routes::NetworkView, families: routes::RouteFamilies) -> Result<bool> {
        let mut current = routes::capture_owned()?;
        current.interfaces.retain(|i| {
            owned
                .interfaces
                .iter()
                .any(|o| o.luid == i.luid && o.guid == i.guid)
        });
        Ok(routes::default_routes_present(&current, families))
    }
    pub fn poll(&mut self) {
        let owner_dead = self
            .owner
            .as_ref()
            .is_some_and(|p| unsafe { WaitForSingleObject(p.0, 0) } == 0);
        let engine_dead = self
            .engine
            .as_mut()
            .is_some_and(|e| !e.alive().unwrap_or(false));
        let now = std::time::Instant::now();
        if self.engine.is_some()
            && let Some(owned) = &self.owned
        {
            let families = self.plan.as_ref().map(RuntimePlan::tun_route_families).unwrap_or_default();
            self.tun_health.refresh(now, || Self::inspect_tun(owned, families));
        } else {
            self.tun_health = TunHealth::default();
        }
        let tun_lost = self.tun_health.lost(now);
        if (owner_dead
            || engine_dead
            || tun_lost
            || (self.engine.is_none() && self.owned.is_some()))
            && let Err(e) = self.stop()
        {
            self.error = Some(e);
        }
    }
    fn stop(&mut self) -> Result<()> {
        let mut failure = None;
        if let Some(e) = self.engine.as_mut() {
            e.stop()?;
        }
        self.engine = None;
        // Wintun removes its interface/routes when the engine closes. Explicitly
        // remove only unchanged rows from this session if any survive.
        if let Some(owned) = self.owned.as_ref() {
            for _ in 0..40 {
                if routes::capture_owned()?.interfaces.is_empty() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            if let Err(e) = routes::cleanup_owned(owned) {
                failure = Some(e);
            }
            let left = routes::inspect()?
                .routes
                .into_iter()
                .any(|r| owned.interfaces.iter().any(|i| i.luid == r.luid));
            if left {
                failure = Some(Error::new(
                    "TunCleanupIncomplete",
                    "tun-cleanup",
                    "Owned interface routes remain; foreign routes were preserved",
                ));
            }
        }
        if let Some(e) = failure {
            return Err(e);
        }
        remove_journal()?;
        self.owned = None;
        self.tun_health = TunHealth::default();
        self.error = None;
        self.owner = None;
        self.owner_pid = 0;
        self.session.clear();
        self.plan = None;
        Ok(())
    }
    fn snapshot(&self, pid: u32) -> ServiceStatus {
        ServiceStatus {
            installed: true,
            running: true,
            engine_pid: if pid == self.owner_pid {
                self.engine.as_ref().map(EngineGroup::pid)
            } else {
                None
            },
            owned_by_caller: pid == self.owner_pid && self.engine.is_some(),
            tun_ready: self.engine.is_some() && self.tun_health.ready,
            version: env!("CARGO_PKG_VERSION").into(),
            error: self.error.clone(),
            diagnostic: String::new(),
        }
    }
    pub fn handle(
        &mut self,
        request: Request,
        pid: u32,
        process: Handle,
        engine_path: &Path,
    ) -> Result<ServiceStatus> {
        self.poll();
        match request {
            Request::Status {} => (),
            Request::Diagnostic { session } => {
                if self.owner_pid != pid || self.session != session {
                    return Err(Error::new(
                        "LeaseMismatch",
                        "service-diagnostic",
                        "Diagnostic belongs to another session",
                    ));
                }
                let mut status = self.snapshot(pid);
                status.diagnostic = self
                    .engine
                    .as_ref()
                    .zip(self.plan.as_ref())
                    .map(|(engine, _plan)| engine.recent_diagnostic())
                    .unwrap_or_default();
                return Ok(status);
            }
            Request::Stop { session } => {
                if (self.engine.is_some() || self.owned.is_some())
                    && (self.owner_pid != pid || self.session != session)
                {
                    return Err(Error::new(
                        "LeaseMismatch",
                        "tun-stop",
                        "This connection belongs to a different owner/session",
                    ));
                }
                if let Err(e) = self.stop() {
                    self.error = Some(e.clone());
                    return Err(e);
                }
                self.error = None;
            }
            Request::Start {
                profile,
                preferences,
                session,
                api_port,
                probe_ports,
                api_secret,
                proxy_secret,
            } => {
                if self.engine.is_some() || self.owned.is_some() {
                    if self.engine.is_some() && self.owner_pid == pid && self.session == session {
                        return Ok(self.snapshot(pid));
                    }
                    if self.owner_pid == pid {
                        // Explicit replacement from the same authenticated process
                        // also recovers a start whose response was lost.
                        self.stop()?;
                    } else {
                        return Err(Error::new(
                            "TunInUse",
                            "tun-start",
                            "Another process owns the current TUN session",
                        ));
                    }
                }
                if !preferences.effective_tun()
                    || [api_port, probe_ports[0], probe_ports[1]]
                        .iter()
                        .any(|p| *p < 1024)
                    || api_port == probe_ports[0]
                    || api_port == probe_ports[1]
                    || probe_ports[0] == probe_ports[1]
                    || [api_port, probe_ports[0], probe_ports[1]].contains(&preferences.proxy_port)
                    || uuid::Uuid::parse_str(&session).is_err()
                    || uuid::Uuid::parse_str(&api_secret).is_err()
                    || uuid::Uuid::parse_str(&proxy_secret).is_err()
                {
                    return Err(Error::new(
                        "InvalidServiceRequest",
                        "tun-start",
                        "Invalid TUN session ports or capabilities",
                    ));
                }
                profile.validate()?;
                preferences.validate()?;
                routes::ensure_no_tun_conflict()?;
                let plan = RuntimePlan::new(
                    engine_path,
                    &profile,
                    &preferences,
                    api_port,
                    &api_secret,
                    &proxy_secret,
                    probe_ports,
                )?;
                plan.validate(engine_path)?;
                let families = plan.tun_route_families();
                self.engine = Some(plan.start(engine_path)?);
                self.plan = Some(plan);
                self.owner = Some(process);
                self.owner_pid = pid;
                self.session = session;
                self.error = None;
                let result = (|| {
                    for _ in 0..100 {
                        let owned = routes::capture_owned()?;
                        if routes::default_routes_present(&owned, families) {
                            save_owned(&owned)?;
                            self.owned = Some(owned);
                            self.tun_health = TunHealth {
                                checked: Some(std::time::Instant::now()),
                                ready: true,
                                missing_since: None,
                            };
                            return Ok(());
                        }
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    Err(Error::new(
                        "TunInterfaceMissing",
                        "tun-ready",
                        "Engine started but Windows did not report the NetPulse virtual interface and routes",
                    ))
                })();
                if let Err(e) = result {
                    let _ = self.stop();
                    return Err(e);
                }
            }
        }
        Ok(self.snapshot(pid))
    }
    pub fn shutdown(&mut self) -> Result<()> {
        self.stop()
    }
}
impl Drop for Controller {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_or_empty_service_components_stop_before_scm_start() {
        let directory=std::env::temp_dir().join(format!("netpulse-service-files-{}",uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        for name in ["NetPulseService.exe","sing-box.exe","libcronet.dll","xray.exe"] {
            assert_eq!(validate_service_files(&directory).unwrap_err().code,"ServiceComponentsUnavailable");
            std::fs::write(directory.join(name),b"fixture").unwrap();
        }
        validate_service_files(&directory).unwrap();
        std::fs::write(directory.join("libcronet.dll"),b"").unwrap();
        assert_eq!(validate_service_files(&directory).unwrap_err().code,"ServiceComponentsUnavailable");
        std::fs::remove_dir_all(directory).unwrap();
        assert_eq!(service_error(windows_service::Error::Winapi(std::io::Error::from_raw_os_error(5))).code,"ServiceAccessDenied");
        assert_eq!(service_error(windows_service::Error::Winapi(std::io::Error::from_raw_os_error(225))).code,"ServiceSecurityBlocked");
        assert!(maintenance_result(0).is_ok());
        assert_eq!(maintenance_result(20).unwrap_err().code,"ServiceUpdateInterrupted");
        assert_eq!(maintenance_result(21).unwrap_err().code,"ServiceStopTimeout");
        assert_eq!(maintenance_result(7).unwrap_err().code,"ServiceInstallFailed");
    }
    #[test]
    fn damaged_registration_and_cancel_keep_distinct_installer_results() {
        for (code,name) in [(20,"ServiceUpdateInterrupted"),(21,"ServiceStopTimeout"),(22,"ServiceRegistrationDamaged"),(23,"UserCancelled"),(24,"ServiceRestartRequired")] {
            let error=maintenance_result(code).unwrap_err();assert_eq!(error.code,name);
            assert_eq!(maintenance_exit_code(&error),code as i32);
        }
        let error=service_error(windows_service::Error::Winapi(std::io::Error::from_raw_os_error(2)));
        assert_eq!(error.code,"ServiceFilesMissing");
        let error=service_configuration_error(windows_service::Error::Winapi(std::io::Error::from_raw_os_error(2)));
        assert_eq!(error.code,"ServiceRegistrationDamaged");assert!(!error.message.contains("Some("));
    }
    #[test]
    fn route_checks_are_coalesced_and_only_confirmed_loss_expires_the_session() {
        let now = std::time::Instant::now();
        let mut health = TunHealth::default();
        let reads = std::cell::Cell::new(0);
        for ms in 0..1000 {
            health.refresh(now + Duration::from_millis(ms), || {
                reads.set(reads.get() + 1);
                Ok(true)
            });
        }
        assert_eq!(reads.get(), 1);
        assert!(health.ready);
        health.refresh(now + Duration::from_secs(1), || Ok(false));
        assert!(!health.ready);
        assert!(!health.lost(now + Duration::from_secs(5)));
        health.refresh(now + Duration::from_secs(5), || {
            Err(Error::new("fixture", "routes", "unavailable"))
        });
        assert!(!health.lost(now + Duration::from_secs(20)));
        health.refresh(now + Duration::from_secs(21), || Ok(false));
        health.refresh(now + Duration::from_secs(26), || Ok(false));
        assert!(health.lost(now + Duration::from_secs(26)));
        health.refresh(now + Duration::from_secs(27), || Ok(true));
        assert!(health.ready);
        assert!(!health.lost(now + Duration::from_secs(28)));
    }
    #[test]
    fn sid_and_typed_request_reject_injection() {
        validate_sid(&current_sid().unwrap()).unwrap();
        assert!(validate_sid("S-1-5-21)(A;;GA;;;WD").is_err());
        assert!(
            serde_json::from_str::<Request>(r#"{"command":"shell","arguments":"fixture"}"#)
                .is_err()
        );
        assert!(serde_json::from_str::<Request>(r#"{"command":"status","pid":123}"#).is_err());
        assert!(
            serde_json::from_str::<Request>(
                r#"{"command":"stop","session":"fixture","config":{}}"#
            )
            .is_err()
        );
    }
    #[test]
    fn real_named_pipe_checks_sid_and_process_identity() {
        let sid = current_sid().unwrap();
        let name = format!(r"\\.\pipe\NetPulse.Test.{}", uuid::Uuid::new_v4());
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let security =
                security_descriptor(&format!("D:P(A;;GA;;;SY)(A;;GRGW;;;{sid})")).unwrap();
            let mut attrs = SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: security.pointer(),
                bInheritHandle: 0,
            };
            let mut server = unsafe {
                ServerOptions::new()
                    .first_pipe_instance(true)
                    .reject_remote_clients(true)
                    .create_with_security_attributes_raw(&name, &mut attrs as *mut _ as _)
            }
            .unwrap();
            let mut client = ClientOptions::new().open(&name).unwrap();
            let task = tokio::spawn(async move {
                client.write_all(b"a").await.unwrap();
                client
            });
            server.connect().await.unwrap();
            let mut byte = [0];
            server.read_exact(&mut byte).await.unwrap();
            let _client = task.await.unwrap();
            let (pid, process) = authenticate_client(&server, &sid).unwrap();
            assert_eq!(pid, std::process::id());
            assert_eq!(unsafe { WaitForSingleObject(process.0, 0) }, 258);
            assert!(authenticate_client(&server, "S-1-0-0").is_err());
            server.disconnect().unwrap();
        });
    }
    #[test]
    fn stalled_pipe_client_cannot_block_other_authenticated_requests() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let sid = current_sid().unwrap();
            let name = format!(r"\\.\pipe\NetPulse.ConcurrentTest.{}", uuid::Uuid::new_v4());
            let mut listener = ServerOptions::new()
                .max_instances(10)
                .reject_remote_clients(true)
                .create(&name)
                .unwrap();
            let controller = std::sync::Arc::new(std::sync::Mutex::new(Controller::default()));
            let frame = serde_json::to_vec(&Request::Status {}).unwrap();
            let mut stalled = ClientOptions::new().open(&name).unwrap();
            stalled.write_u32_le(frame.len() as u32).await.unwrap();
            stalled.write_all(&frame).await.unwrap();
            listener.connect().await.unwrap();
            let next = ServerOptions::new()
                .max_instances(10)
                .create(&name)
                .unwrap();
            let first = std::mem::replace(&mut listener, next);
            let c = controller.clone();
            let owner = sid.clone();
            let task = tokio::spawn(async move {
                serve_connection(first, &owner, c, Path::new("unused.exe")).await;
            });
            let start = std::time::Instant::now();
            let mut client = ClientOptions::new().open(&name).unwrap();
            client.write_u32_le(frame.len() as u32).await.unwrap();
            client.write_all(&frame).await.unwrap();
            listener.connect().await.unwrap();
            let server = tokio::spawn(async move {
                serve_connection(listener, &sid, controller, Path::new("unused.exe")).await;
            });
            let n = tokio::time::timeout(Duration::from_millis(1500), client.read_u32_le())
                .await
                .unwrap()
                .unwrap();
            let mut bytes = vec![0; n as usize];
            client.read_exact(&mut bytes).await.unwrap();
            assert!(
                serde_json::from_slice::<Reply>(&bytes)
                    .unwrap()
                    .status
                    .is_some()
            );
            assert!(start.elapsed() < Duration::from_millis(1500));
            client.write_u8(0x06).await.unwrap();
            server.await.unwrap();
            drop(stalled);
            task.await.unwrap();
        });
    }
    #[test]
    fn persistent_ipc_recovers_from_abandoned_clients() {
        let seconds = std::env::var("NETPULSE_IPC_ENDURANCE_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(2)
            .clamp(1, 600);
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let sid = current_sid().unwrap();
            let name = format!(r"\\.\pipe\NetPulse.Endurance.{}", uuid::Uuid::new_v4());
            let mut listener = ServerOptions::new().max_instances(10).reject_remote_clients(true).create(&name).unwrap();
            let controller = std::sync::Arc::new(std::sync::Mutex::new(Controller::default()));
            let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let flag = stop.clone();
            let server_name = name.clone();
            let server = tokio::spawn(async move {
                let mut clients = tokio::task::JoinSet::new();
                while !flag.load(std::sync::atomic::Ordering::Acquire) {
                    tokio::select! {
                        result = listener.connect() => {
                            result.unwrap();
                            let next = ServerOptions::new().max_instances(10).create(&server_name).unwrap();
                            let connection = std::mem::replace(&mut listener, next);
                            let sid = sid.clone(); let core = controller.clone();
                            clients.spawn(async move { serve_connection(connection,&sid,core,Path::new("unused.exe")).await; });
                        },
                        _ = tokio::time::sleep(Duration::from_millis(100)) => {},
                        Some(_) = clients.join_next(), if !clients.is_empty() => {},
                    }
                }
                while clients.join_next().await.is_some() {}
            });
            let frame = serde_json::to_vec(&Request::Status {}).unwrap();
            let until = std::time::Instant::now() + Duration::from_secs(seconds);
            let mut count = 0; let mut abandoned = std::collections::VecDeque::new();
            while std::time::Instant::now() < until {
                let mut client = loop {
                    match ClientOptions::new().open(&name) {
                        Ok(c) => break c,
                        Err(e) if e.raw_os_error() == Some(231) => tokio::time::sleep(Duration::from_millis(5)).await,
                        Err(e) => panic!("Owned IPC fixture: {e}"),
                    }
                };
                client.write_u32_le(frame.len() as u32).await.unwrap();
                client.write_all(&frame).await.unwrap();
                if count % 4 == 0 {
                    // A client that never consumes or acknowledges its response.
                    abandoned.push_back(client);
                    if abandoned.len() > 3 { abandoned.pop_front(); }
                } else {
                    let n = tokio::time::timeout(Duration::from_millis(1500),client.read_u32_le()).await.unwrap().unwrap();
                    let mut bytes = vec![0;n as usize];client.read_exact(&mut bytes).await.unwrap();
                    assert!(serde_json::from_slice::<Reply>(&bytes).unwrap().status.is_some());
                    client.write_u8(0x06).await.unwrap();
                }
                count += 1;
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            drop(abandoned);
            stop.store(true,std::sync::atomic::Ordering::Release);
            server.await.unwrap();
            assert!(count >= 4);
        });
    }
    #[test]
    fn service_monitor_reads_cache_without_waiting_for_ipc() {
        let mut engine = ServiceEngine {
            monitor: std::sync::Arc::new(std::sync::Mutex::new(ServiceMonitor {
                latest: Err(Error::new(
                    "ServiceTimeout",
                    "service-monitor",
                    "fixture timeout",
                )),
                at: std::time::Instant::now(),
            })),
            monitor_stop: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            session: String::new(),
            pid: 123,
            active: true,
        };
        let started = std::time::Instant::now();
        for _ in 0..1000 {
            assert_eq!(engine.alive().unwrap_err().code, "ServiceTimeout");
            assert!(engine.diagnostic().is_empty());
        }
        assert!(started.elapsed() < Duration::from_millis(250));
        engine.active = false; // Never send a real service command in this fixture.
    }
    #[test]
    fn framed_responses_survive_delayed_reads_and_reconnects() {
        let name = format!(r"\\.\pipe\NetPulse.FrameTest.{}", uuid::Uuid::new_v4());
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let mut server = ServerOptions::new()
                .reject_remote_clients(true)
                .max_instances(10)
                .create(&name)
                .unwrap();
            for _ in 0..6 {
                let client_name = name.clone();
                let task = tokio::spawn(async move {
                    let mut client = loop {
                        match ClientOptions::new().open(&client_name) {
                            Ok(client) => break client,
                            Err(error) if error.raw_os_error() == Some(231) => {
                                tokio::time::sleep(Duration::from_millis(5)).await
                            }
                            Err(error) => panic!("Fixture pipe open: {error}"),
                        }
                    };
                    let request = serde_json::to_vec(&Request::Status {}).unwrap();
                    let mut frame = (request.len() as u32).to_le_bytes().to_vec();
                    frame.extend(request);
                    for part in frame.chunks(3) {
                        client.write_all(part).await.unwrap();
                        tokio::task::yield_now().await;
                    }
                    tokio::time::sleep(Duration::from_millis(30)).await;
                    let n = client.read_u32_le().await.unwrap();
                    let mut bytes = vec![0; n as usize];
                    client.read_exact(&mut bytes).await.unwrap();
                    let reply: Reply = serde_json::from_slice(&bytes).unwrap();
                    assert_eq!(reply.error.unwrap().message.len(), 32 * 1024);
                    client.write_all(&[0x06]).await.unwrap();
                });
                server.connect().await.unwrap();
                let replacement = ServerOptions::new()
                    .reject_remote_clients(true)
                    .max_instances(10)
                    .create(&name)
                    .unwrap();
                let mut connection = std::mem::replace(&mut server, replacement);
                let n = connection.read_u32_le().await.unwrap();
                let mut bytes = vec![0; n as usize];
                connection.read_exact(&mut bytes).await.unwrap();
                assert!(matches!(
                    serde_json::from_slice::<Request>(&bytes).unwrap(),
                    Request::Status {}
                ));
                let reply = Reply {
                    status: None,
                    error: Some(Error::new("Fixture", "test", "x".repeat(32 * 1024))),
                };
                tokio::time::timeout(Duration::from_secs(2), reply_ack(&mut connection, &reply))
                    .await
                    .unwrap()
                    .unwrap();
                connection.disconnect().unwrap();
                task.await.unwrap();
            }
        });
    }
}
