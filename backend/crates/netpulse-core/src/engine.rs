use crate::{
    Error, Result,
    model::{Preferences, Profile, RouteMode},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

fn sensitive(v: &Value, key: &str, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            for (k, v) in m {
                sensitive(v, k, out);
            }
        }
        Value::Array(a) => {
            for v in a {
                sensitive(v, key, out);
            }
        }
        Value::String(s)
            if [
                "password",
                "id",
                "pass",
                "publickey",
                "shortid",
                "servername",
                "address",
                "host",
                "servicename",
                "uuid",
                "public_key",
                "short_id",
                "username",
                "secret",
                "server",
                "server_name",
                "path",
                "service_name",
            ]
            .contains(&key.to_ascii_lowercase().as_str())
                && !s.is_empty() =>
        {
            out.push(s.clone())
        }
        _ => (),
    }
}

pub const ENGINE_VERSION: &str = "1.14.3";
pub const ENGINE_SHA256: &str = "5c19bb3f8c12d4e85b3d47fe63123a7624b314b58dd7a8e32f51004f1c4d2faf";
pub const CRONET_SHA256: &str = "3217c6260fbca5f16072e0b79735742f40109a63bb0ff88fd6b96dd6b54a2928";

pub fn verify_engine(path: &Path) -> Result<()> {
    std::fs::File::open(path).map_err(|e| component_access_error(&path.to_string_lossy(), e))?;
    if crate::core_releases::verify_custom(path, "sing-box")? {
        return Ok(());
    }
    for (p, expected) in [
        (path.to_path_buf(), ENGINE_SHA256),
        (path.with_file_name("libcronet.dll"), CRONET_SHA256),
    ] {
        let mut file =
            std::fs::File::open(&p).map_err(|e| component_access_error(&p.to_string_lossy(), e))?;
        let mut digest = Sha256::new();
        let mut block = [0u8; 65536];
        loop {
            let n = file
                .read(&mut block)
                .map_err(|e| Error::io("engine-verify", e))?;
            if n == 0 {
                break;
            }
            digest.update(&block[..n]);
        }
        if digest
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
            != expected
        {
            return Err(Error::new(
                "EngineHashMismatch",
                "engine-verify",
                "Engine component differs from pinned official release",
            ));
        }
    }
    Ok(())
}
pub(crate) fn component_access_error(component: &str, error: std::io::Error) -> Error {
    if matches!(
        error.kind(),
        std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
    ) || matches!(error.raw_os_error(), Some(225 | 226))
    {
        Error::new(
            "RuntimeComponentUnavailable",
            "engine-verify",
            format!(
                "{}: {component} (Windows {}). Проверьте указанный путь и отчёт защиты. Восстановление доступно через официальный установщик NetPulse; настройки и подписки сохраняются.",
                if error.kind()==std::io::ErrorKind::NotFound {"Файл не найден"} else if matches!(error.raw_os_error(),Some(225|226)) {"Файл заблокирован защитой Windows"} else {"Windows отказала в доступе к файлу"},
                error.raw_os_error().unwrap_or(0)
            ),
        )
    } else {
        Error::io("engine-verify", error)
    }
}
pub fn configuration(
    profile: &Profile,
    prefs: &Preferences,
    api_port: u16,
    api_secret: &str,
    proxy_secret: &str,
) -> Result<Value> {
    profile.validate()?;
    let profile = profile.resolved_with(prefs)?;
    let normalized = prefs.clone().normalized();
    let prefs = &normalized;
    prefs.validate()?;
    let mut outbound = profile.outbound.clone();
    outbound["tag"] = json!("proxy");
    let mut rules = vec![
        json!({"port":53,"action":"sniff"}),
        json!({"protocol":"dns","action":"hijack-dns"}),
    ];
    if prefs.effective_tun() && prefs.sniffing.enabled {
        // TUN needs host discovery. Mixed/SOCKS/CONNECT already supplies
        // its target; sniffing it before CONNECT acknowledgement can stall
        // clients that wait for the acknowledgement before sending data.
        let mut sniff = json!({"inbound":["tun-in"],"action":"sniff","sniffer":prefs.sniffing.protocols(),"timeout":format!("{}ms",prefs.sniffing.timeout_ms)});
        if !prefs.sniff_excluded_apps.is_empty() {
            // Invert the process condition only. Inverting a basic rule also
            // inverts inbound=tun-in and accidentally sniffs every SOCKS/CONNECT.
            sniff.as_object_mut().unwrap().remove("inbound");
            sniff["type"] = json!("logical");
            sniff["mode"] = json!("and");
            sniff["rules"] = json!([
                {"inbound":["tun-in"]},
                {"process_path_regex":crate::app_rules::process_patterns(&prefs.sniff_excluded_apps),"invert":true}
            ]);
        }
        rules.push(sniff);
    }
    if !prefs.bypass_domains.is_empty() {
        rules.push(
            json!({"domain_suffix":prefs.bypass_domains,"action":"route","outbound":"direct"}),
        );
    }
    crate::advanced_routing::append_rules(&mut rules, prefs);
    let final_ = match prefs.route_mode {
        RouteMode::Direct => "direct",
        RouteMode::Global => "proxy",
        RouteMode::Whitelist => {
            if !prefs.process_paths().is_empty() {
                rules.push(
                    json!({"process_path_regex":crate::app_rules::process_patterns(prefs.process_paths()),"action":"route","outbound":"proxy"}),
                );
            }
            "direct"
        }
        RouteMode::Blacklist => {
            if !prefs.process_paths().is_empty() {
                rules.push(
                    json!({"process_path_regex":crate::app_rules::process_patterns(prefs.process_paths()),"action":"route","outbound":"direct"}),
                );
            }
            "proxy"
        }
    };
    let mut inbounds = vec![
        json!({"type":"mixed","tag":"local-proxy","listen":"127.0.0.1","listen_port":prefs.proxy_port,"users":[{"username":"netpulse","password":proxy_secret}]}),
    ];
    if prefs.system_proxy || prefs.effective_tun() {
        // Never turn the public Windows proxy endpoint into an authenticated
        // diagnostic endpoint when switching to TUN. Apps can cache its address.
        // The dedicated diagnostic listeners retain their random credentials.
        inbounds[0]["users"] = json!([]);
    }
    if prefs.effective_tun() {
        inbounds.push(json!({"type":"tun","tag":"tun-in","interface_name":"NetPulse","address":["172.31.255.1/30","fdfe:dcba:9876::1/126"],"mtu":prefs.mtu,"auto_route":true,"strict_route":true,"stack":if prefs.network_preset == "compatibility" { "mixed" } else { "system" }}));
    }
    let mut config = json!({"log":{"level":match prefs.logging_level.as_str(){"DEBUG"=>"debug","WARNING"=>"warn","ERROR"=>"error",_=>"info"},"disabled":false,"timestamp":true},"dns":{"servers":[prefs.dns_server(),{"type":"local","tag":"bootstrap"}],"final":"resolver","strategy":"prefer_ipv4"},"inbounds":inbounds,"outbounds":[outbound,{"type":"direct","tag":"direct"}],"route":{"rules":rules,"final":final_,"auto_detect_interface":true,"default_domain_resolver":"bootstrap"},"experimental":{"clash_api":{"external_controller":format!("127.0.0.1:{api_port}"),"secret":api_secret}}});
    if !prefs.bypass_domains.is_empty() {
        config["dns"]["reverse_mapping"] = json!(true);
        config["dns"]["rules"] =
            json!([{"domain_suffix":prefs.bypass_domains,"action":"route","server":"bootstrap"}]);
    }
    crate::advanced_routing::apply(&mut config, prefs);
    Ok(config)
}

#[cfg(windows)]
struct Job(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
unsafe impl Send for Job {}
#[cfg(windows)]
impl Job {
    fn attach(child: &Child) -> Result<Self> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::{Foundation::CloseHandle, System::JobObjects::*};
        let h = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if h.is_null() {
            return Err(Error::io("engine-job", std::io::Error::last_os_error()));
        }
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let ok = unsafe {
            SetInformationJobObject(
                h,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as _,
                std::mem::size_of_val(&limits) as u32,
            )
        };
        if ok == 0 || unsafe { AssignProcessToJobObject(h, child.as_raw_handle()) } == 0 {
            let e = std::io::Error::last_os_error();
            unsafe { CloseHandle(h) };
            return Err(Error::io("engine-job", e));
        }
        Ok(Self(h))
    }
}
#[cfg(windows)]
impl Drop for Job {
    fn drop(&mut self) {
        unsafe { windows_sys::Win32::Foundation::CloseHandle(self.0) };
    }
}

pub struct Engine {
    child: Child,
    #[cfg(windows)]
    job: Option<Job>,
    diagnostics: Arc<Mutex<Zeroizing<Vec<u8>>>>,
    secrets: Zeroizing<Vec<String>>,
    readers: Vec<std::thread::JoinHandle<()>>,
}
impl Engine {
    pub fn spawn(path: &Path, config: &Value, check: bool) -> Result<Self> {
        Self::spawn_arguments(
            path,
            config,
            &[
                if check { "check" } else { "run" },
                "--disable-color",
                "-c",
                "stdin",
            ],
        )
    }
    pub fn spawn_arguments(path: &Path, config: &Value, args: &[&str]) -> Result<Self> {
        let mut cmd = Command::new(path);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        cmd.current_dir(path.parent().unwrap_or(Path::new(".")));
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        let until = Instant::now() + Duration::from_secs(3);
        let mut child = loop {
            match cmd.spawn() {
                Ok(child) => break child,
                Err(e) if e.raw_os_error() == Some(32) && Instant::now() < until => {
                    std::thread::sleep(Duration::from_millis(60))
                }
                Err(e) => return Err(component_access_error(&path.to_string_lossy(), e)),
            }
        };
        // The engine cannot parse a config/open sockets until stdin is supplied.
        // Attach its job first; death before attachment closes stdin and exits parsing.
        #[cfg(windows)]
        let job = match Job::attach(&child) {
            Ok(job) => Some(job),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e);
            }
        };
        let diagnostics = Arc::new(Mutex::new(Zeroizing::new(Vec::new())));
        fn capture(
            mut stream: impl Read + Send + 'static,
            captured: Arc<Mutex<Zeroizing<Vec<u8>>>>,
        ) -> std::thread::JoinHandle<()> {
            std::thread::spawn(move || {
                let mut buf = Zeroizing::new([0u8; 4096]);
                while let Ok(n) = stream.read(&mut *buf) {
                    if n == 0 {
                        break;
                    }
                    if let Ok(mut bytes) = captured.lock() {
                        bytes.extend_from_slice(&buf[..n]);
                        let excess = bytes.len().saturating_sub(16384);
                        if excess > 0 {
                            // Discard the remainder of the first truncated line too.
                            let end = bytes[excess..]
                                .iter()
                                .position(|b| *b == b'\n')
                                .map_or(bytes.len(), |n| excess + n + 1);
                            bytes.drain(..end);
                        }
                    }
                }
            })
        }
        let mut readers = vec![];
        if let Some(stderr) = child.stderr.take() {
            readers.push(capture(stderr, diagnostics.clone()));
        }
        if let Some(stdout) = child.stdout.take() {
            readers.push(capture(stdout, diagnostics.clone()));
        }
        let payload = Zeroizing::new(serde_json::to_vec(config).map_err(|_| {
            Error::new(
                "ConfigError",
                "engine-config",
                "Could not serialize engine configuration",
            )
        })?);
        let mut managed = Self {
            child,
            #[cfg(windows)]
            job,
            diagnostics,
            secrets: {
                let mut values = Vec::new();
                sensitive(config, "", &mut values);
                Zeroizing::new(values)
            },
            readers,
        };
        let write = managed
            .child
            .stdin
            .take()
            .ok_or_else(|| Error::new("EngineError", "engine-stdin", "Engine stdin unavailable"))?
            .write_all(&payload);
        write.map_err(|e| Error::io("engine-config-pipe", e))?;
        Ok(managed)
    }
    pub fn validate(path: &Path, config: &Value) -> Result<()> {
        let mut process = Self::spawn(path, config, true)?;
        process.wait_checked(config)
    }
    pub fn wait_checked(&mut self, config: &Value) -> Result<()> {
        let process = self;
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            if let Some(code) = process
                .child
                .try_wait()
                .map_err(|e| Error::io("engine-check", e))?
            {
                process.stop()?;
                return if code.success() {
                    Ok(())
                } else {
                    Err(Error::new(
                        "EngineConfigRejected",
                        "engine-check",
                        format!(
                            "sing-box rejected configuration (exit {:?}): {}",
                            code.code(),
                            process.diagnostic(config)
                        ),
                    ))
                };
            }
            if Instant::now() > deadline {
                return Err(Error::new(
                    "EngineTimeout",
                    "engine-check",
                    "Engine configuration check exceeded 8 seconds",
                ));
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    pub fn alive(&mut self) -> Result<bool> {
        Ok(self
            .child
            .try_wait()
            .map_err(|e| Error::io("engine-status", e))?
            .is_none())
    }
    pub fn pid(&self) -> u32 {
        self.child.id()
    }
    pub fn diagnostic(&self, config: &Value) -> String {
        let mut secrets = Zeroizing::new(self.secrets.to_vec());
        sensitive(config, "", &mut secrets);
        secrets.sort_by_key(|s| std::cmp::Reverse(s.len()));
        let mut text = self
            .diagnostics
            .lock()
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default();
        for s in secrets.iter() {
            text = text.replace(s, "[redacted]");
        }
        // Keep complete lines only. A rolling byte tail used to turn the same
        // old error into a different INFO fragment on every snapshot.
        // Each engine contributes at most 6 KB, leaving headroom in the 64 KB
        // authenticated IPC response even after JSON escaping.
        crate::runtime_log::complete_tail(&text, 6000).replace("://", "[:]//")
    }
    pub fn stop(&mut self) -> Result<()> {
        if self.alive()? {
            self.child.kill().map_err(|e| Error::io("engine-stop", e))?;
        }
        #[cfg(windows)]
        {
            self.job.take();
        }
        let deadline = Instant::now() + Duration::from_secs(4);
        while self.alive()? {
            if Instant::now() >= deadline {
                return Err(Error::new(
                    "EngineStopTimeout",
                    "engine-stop",
                    "Owned engine did not exit within 4 seconds",
                ));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        // A delayed pipe reader must never hold the TUN controller mutex forever.
        // Dropping an unfinished JoinHandle detaches it; no process is abandoned.
        let deadline = Instant::now() + Duration::from_millis(250);
        while self.readers.iter().any(|r| !r.is_finished()) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        for reader in self.readers.drain(..) {
            if reader.is_finished() {
                let _ = reader.join();
            }
        }
        Ok(())
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
pub fn installed_engine(data_dir: &Path) -> PathBuf {
    data_dir.join("engine/sing-box.exe")
}

pub fn install(data_dir: &Path) -> Result<()> {
    let dest = data_dir.join("engine");
    std::fs::create_dir_all(&dest).map_err(|e| Error::io("engine-directory", e))?;
    let url = "https://github.com/SagerNet/sing-box/releases/download/v1.14.3/sing-box-1.14.3-windows-amd64.zip";
    let response = crate::network::client(Duration::from_secs(120))?
        .get(url)
        .send()
        .map_err(|_| {
            Error::new(
                "EngineDownloadFailed",
                "engine-download",
                "Official engine download failed",
            )
        })?;
    if !response.status().is_success() {
        return Err(Error::new(
            "EngineDownloadFailed",
            "engine-download",
            format!(
                "Official download returned HTTP {}",
                response.status().as_u16()
            ),
        ));
    }
    let mut bytes = Vec::new();
    response
        .take(150 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| Error::io("engine-download", e))?;
    if bytes.len() > 150 * 1024 * 1024 {
        return Err(Error::new(
            "EngineSizeLimit",
            "engine-download",
            "Engine archive exceeds 150 MiB",
        ));
    }
    let hash = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    if hash != "d8b86ef58936dba9230ec6a57ffa4243933f3c14ef4809828b6c553440ed1518" {
        return Err(Error::new(
            "EngineHashMismatch",
            "engine-download",
            "Official archive SHA-256 does not match the pinned release",
        ));
    }
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| {
        Error::new(
            "EngineArchiveInvalid",
            "engine-extract",
            "Invalid engine ZIP",
        )
    })?;
    let staging = dest.join(format!("staging-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&staging).map_err(|e| Error::io("engine-staging", e))?;
    let result = (|| {
        for name in ["sing-box.exe", "libcronet.dll"] {
            let entry = format!("sing-box-1.14.3-windows-amd64/{name}");
            let file = archive.by_name(&entry).map_err(|_| {
                Error::new(
                    "EngineArchiveInvalid",
                    "engine-extract",
                    "Engine component missing from archive",
                )
            })?;
            let mut out = std::fs::File::create(staging.join(name))
                .map_err(|e| Error::io("engine-extract", e))?;
            std::io::copy(&mut file.take(120 * 1024 * 1024 + 1), &mut out)
                .map_err(|e| Error::io("engine-extract", e))?;
            out.sync_all().map_err(|e| Error::io("engine-extract", e))?;
        }
        verify_engine(&staging.join("sing-box.exe"))?;
        for name in ["sing-box.exe", "libcronet.dll"] {
            std::fs::rename(staging.join(name), dest.join(name))
                .map_err(|e| Error::io("engine-install", e))?;
        }
        Ok(())
    })();
    // Only this newly-created UUID directory and its two known files are removed.
    for name in ["sing-box.exe", "libcronet.dll"] {
        let _ = std::fs::remove_file(staging.join(name));
    }
    let _ = std::fs::remove_dir(staging);
    result
}

#[cfg(test)]
mod tests {
    #[test]
    fn missing_and_denied_components_report_the_actual_file_and_distinct_reason() {
        let missing=super::component_access_error(r"C:\NetPulse\data\engine\sing-box.exe",std::io::Error::from_raw_os_error(2));
        let denied=super::component_access_error(r"C:\NetPulse\cores\libcronet.dll",std::io::Error::from_raw_os_error(5));
        assert!(missing.message.contains("Файл не найден") && missing.message.contains("sing-box.exe"));
        assert!(denied.message.contains("отказала в доступе") && denied.message.contains("libcronet.dll"));
    }
    #[test]
    #[cfg(windows)]
    #[ignore = "Pinned core, owned loopback HTTP only; never creates TUN or changes Windows proxy"]
    fn tun_public_proxy_does_not_challenge_cached_clients_but_diagnostics_stay_private() {
        use std::{
            io::{Read, Write},
            net::{TcpListener, TcpStream},
            time::Duration,
        };
        let path = std::path::PathBuf::from(std::env::var_os("NETPULSE_TEST_ENGINE").unwrap());
        let profile = crate::imports::parse_uri("socks5://127.0.0.1:9").unwrap();
        let public = crate::runtime::free_port().unwrap();
        let diag = crate::runtime::free_port().unwrap();
        let mut config = configuration(
            &profile,
            &Preferences {
                tun: true,
                system_proxy: false,
                proxy_port: public,
                ..Default::default()
            },
            crate::runtime::free_port().unwrap(),
            "fixture",
            "private-fixture",
        )
        .unwrap();
        assert_eq!(config["inbounds"][0]["listen"], "127.0.0.1");
        assert_eq!(config["inbounds"][0]["users"], json!([]));
        // Exercise the generated public listener without ever opening a TUN.
        config["inbounds"]
            .as_array_mut()
            .unwrap()
            .retain(|i| i["type"] != "tun");
        config["inbounds"].as_array_mut().unwrap().push(json!({"type":"mixed","tag":"diagnostic","listen":"127.0.0.1","listen_port":diag,"users":[{"username":"netpulse","password":"private-fixture"}]}));
        config["route"]["final"] = json!("direct");
        let origin = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/fixture", origin.local_addr().unwrap());
        origin.set_nonblocking(true).unwrap();
        let responder = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + Duration::from_secs(8);
            loop {
                match origin.accept() {
                    Ok((mut socket, _)) => {
                        socket.set_nonblocking(false).unwrap();
                        socket
                            .set_read_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        let mut request=Vec::new();let mut buf=[0;1024];
                        while request.len()<4096&&!request.windows(4).any(|s|s==b"\r\n\r\n") {
                            let n=socket.read(&mut buf).unwrap();assert!(n>0);request.extend_from_slice(&buf[..n]);
                        }
                        assert!(request.windows(4).any(|s|s==b"\r\n\r\n"));
                        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\nfixture").unwrap();
                        break;
                    }
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            && std::time::Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(20))
                    }
                    Err(e) => panic!("Owned fixture: {e}"),
                }
            }
        });
        let mut engine = Engine::spawn(&path, &config, false).unwrap();
        for _ in 0..100 {
            if TcpStream::connect(("127.0.0.1", public)).is_ok() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let client = |port| {
            reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(3))
                .proxy(reqwest::Proxy::http(format!("http://127.0.0.1:{port}")).unwrap())
                .build()
                .unwrap()
        };
        let response = client(public).get(&url).send().unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.text().unwrap(), "fixture");
        assert_eq!(client(diag).get(&url).send().unwrap().status(), 407);
        engine.stop().unwrap();
        responder.join().unwrap();
        let standalone = configuration(
            &profile,
            &Preferences::default(),
            28000,
            "fixture",
            "private-fixture",
        )
        .unwrap();
        assert_eq!(
            standalone["inbounds"][0]["users"][0]["password"],
            "private-fixture"
        );
    }
    #[test]
    fn stop_does_not_wait_forever_for_a_stalled_log_reader() {
        use std::os::windows::process::CommandExt;
        let mut child = Command::new(r"C:\Windows\System32\cmd.exe")
            .args(["/d", "/c", "exit 0"])
            .creation_flags(0x08000000)
            .spawn()
            .unwrap();
        child.wait().unwrap();
        let (release, wait) = std::sync::mpsc::channel::<()>();
        let reader = std::thread::spawn(move || {
            let _ = wait.recv();
        });
        let mut engine = Engine {
            child,
            job: None,
            diagnostics: Arc::new(Mutex::new(Zeroizing::new(vec![]))),
            secrets: Zeroizing::new(vec![]),
            readers: vec![reader],
        };
        let started = Instant::now();
        engine.stop().unwrap();
        assert!(started.elapsed() < Duration::from_secs(1));
        release.send(()).unwrap();
        assert!(!engine.alive().unwrap());
    }

    use super::*;
    #[cfg(windows)]
    #[test]
    #[ignore = "Requires owned job_owner fixture and pinned sing-box"]
    fn job_kills_engine_when_owner_crashes() {
        use std::io::{BufRead, BufReader};
        use windows_sys::Win32::{Foundation::CloseHandle, System::Threading::*};
        let fixture = std::env::var_os("NETPULSE_JOB_OWNER").expect("Set NETPULSE_JOB_OWNER");
        let mut owner = Command::new(fixture)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut line = String::new();
        BufReader::new(owner.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let v: Value = serde_json::from_str(&line).unwrap();
        let pid = v["pid"].as_u64().unwrap() as u32;
        let handle = unsafe {
            OpenProcess(
                PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
                0,
                pid,
            )
        };
        assert!(!handle.is_null());
        owner.kill().unwrap();
        owner.wait().unwrap();
        assert_eq!(
            unsafe { WaitForSingleObject(handle, 5000) },
            0,
            "Owned engine survived a hard parent crash"
        );
        unsafe { CloseHandle(handle) };
    }
    #[test]
    fn rules_use_executable_names_and_no_plain_config() {
        let p = crate::imports::parse_uri("socks5://127.0.0.1:1080").unwrap();
        let prefs = Preferences {
            route_mode: RouteMode::Whitelist,
            tun: true,
            whitelist: vec![r"C:\Apps\Telegram.exe".into()],
            ..Preferences::default()
        };
        let c = configuration(&p, &prefs, 9999, "API-SECRET", "PROXY-SECRET").unwrap();
        assert_eq!(c["route"]["final"], "direct");
        assert_eq!(
            c["route"]["rules"].as_array().unwrap().iter().find(|rule|rule["outbound"]=="proxy").unwrap()["process_path_regex"][0],
            r"(?i)(?:^|[\\/])telegram\.exe$"
        );
        assert_eq!(c["inbounds"][1]["type"], "tun");
    }
}
