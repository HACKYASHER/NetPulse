use crate::{
    Error, Result, engine,
    imports::ImportReport,
    model::{Preferences, Profile, ProfileSummary},
    proxy::ProxyLease,
    runtime::{EngineGroup, RuntimePlan},
    store::Store,
};
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    path::PathBuf,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Disconnected,
    Validating,
    Connecting,
    Connected,
    Switching,
    Disconnecting,
    Error,
}
#[derive(Serialize, Clone, Debug, Default)]
pub struct Metrics {
    pub upload_total: u64,
    pub download_total: u64,
    pub upload_bps: u64,
    pub download_bps: u64,
    pub connections: usize,
}
#[derive(Serialize, Clone)]
pub struct Snapshot {
    pub state: ConnectionState,
    pub active: Option<ProfileSummary>,
    pub engine_pid: Option<u32>,
    pub metrics: Option<Metrics>,
    pub metrics_error: Option<ErrorView>,
    pub error: Option<ErrorView>,
    pub probe_ms: Option<u64>,
    pub system_proxy_active: bool,
    pub tun_active: bool,
    pub applied_preferences: Option<Preferences>,
    pub recovery_pid: Option<u32>,
}
#[derive(Serialize, Clone, Debug)]
pub struct ErrorView {
    pub code: String,
    pub stage: String,
    pub message: String,
}
impl From<&Error> for ErrorView {
    fn from(e: &Error) -> Self {
        Self {
            code: e.code.clone(),
            stage: e.stage.clone(),
            message: e.message.clone(),
        }
    }
}
pub struct NetworkCore {
    pub store: Store,
    pub engine_path: PathBuf,
    state: ConnectionState,
    active: Option<Profile>,
    engine: Option<ManagedEngine>,
    proxy: Option<ProxyLease>,
    prefs: Preferences,
    api_port: u16,
    probe_port: u16,
    direct_probe_port: u16,
    api_secret: Zeroizing<String>,
    proxy_secret: Zeroizing<String>,
    error: Option<ErrorView>,
    probe_ms: Option<u64>,
    metrics: Option<Metrics>,
    connection_monitor: Option<crate::connection_monitor::Monitor>,
    metrics_error: Option<ErrorView>,
    recent_runtime_lines: std::collections::VecDeque<String>,
    #[cfg(test)]
    probe_certificate: Option<reqwest::Certificate>,
    observer: Option<Box<dyn Fn(ConnectionState) + Send + Sync>>,
}
enum ManagedEngine {
    #[cfg(test)]
    FaultFixture {
        monitor_error: bool,
        stop_error: bool,
    },
    Local(Box<EngineGroup>),
    #[cfg(windows)]
    Service(crate::service::ServiceEngine),
}
impl ManagedEngine {
    fn pid(&self) -> u32 {
        match self {
            Self::Local(e) => e.pid(),
            #[cfg(test)]
            Self::FaultFixture { .. } => 12345,
            #[cfg(windows)]
            Self::Service(e) => e.pid(),
        }
    }
    fn alive(&mut self) -> Result<bool> {
        match self {
            Self::Local(e) => e.alive(),
            #[cfg(test)]
            Self::FaultFixture { monitor_error, .. } => {
                if *monitor_error {
                    Err(Error::new("IpcTimeout", "service-read", "Fixture timeout"))
                } else {
                    Ok(true)
                }
            }
            #[cfg(windows)]
            Self::Service(e) => e.alive(),
        }
    }
    fn stop(&mut self) -> Result<()> {
        match self {
            Self::Local(e) => e.stop(),
            #[cfg(test)]
            Self::FaultFixture { stop_error, .. } => {
                if *stop_error {
                    Err(Error::new("IpcTimeout", "service-stop", "Fixture timeout"))
                } else {
                    Ok(())
                }
            }
            #[cfg(windows)]
            Self::Service(e) => e.stop(),
        }
    }
    fn recent_diagnostic(&self) -> String {
        match self {
            Self::Local(e) => e.recent_diagnostic(),
            #[cfg(windows)]
            Self::Service(e) => e.diagnostic(),
            #[cfg(test)]
            Self::FaultFixture { .. } => String::new(),
        }
    }
    fn diagnostic(&self, p: &RuntimePlan) -> String {
        match self {
            Self::Local(e) => e.diagnostic(p),
            #[cfg(test)]
            Self::FaultFixture { .. } => String::new(),
            #[cfg(windows)]
            Self::Service(e) => e.diagnostic(),
        }
    }
}
impl NetworkCore {
    /// Catch command panics while the caller still owns the mutex guard. This
    /// allows cleanup and a later disconnect/quit instead of poisoning the core.
    pub fn guarded<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(self))) {
            Ok(result) => result,
            Err(_) => {
                let cleaned =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.disconnect()))
                        .is_ok_and(|r| r.is_ok());
                let e = Error::new(
                    "CoreOperationFailed",
                    "command",
                    if cleaned {
                        "Внутренняя операция прервана; подключение остановлено. Повторите действие."
                    } else {
                        "Внутренняя операция прервана; очистка соединения не завершена. Повторите отключение."
                    },
                );
                self.log_error(&e);
                Err(e)
            }
        }
    }
    pub fn new(store: Store, engine_path: PathBuf) -> Result<Self> {
        let prefs = store.preferences()?;
        let _ = store.log(
            "INFO",
            "Core",
            "core-ready",
            "Idle",
            "Core controller ready; runtime starts on connection",
        );
        #[cfg(windows)]
        if let Ok(status) = crate::service::installation_status() {
            let _ = store.log(
                "INFO",
                "Service",
                "service-state",
                "StartupStatus",
                &format!(
                    "Windows service installed={} running={}",
                    status.installed, status.running
                ),
            );
        }
        Ok(Self {
            store,
            engine_path,
            state: ConnectionState::Disconnected,
            active: None,
            engine: None,
            proxy: None,
            prefs,
            api_port: 0,
            probe_port: 0,
            direct_probe_port: 0,
            api_secret: Zeroizing::new(String::new()),
            proxy_secret: Zeroizing::new(String::new()),
            error: None,
            probe_ms: None,
            metrics: None,
            recent_runtime_lines: Default::default(),
            connection_monitor: None,
            metrics_error: None,
            observer: None,
            #[cfg(test)]
            probe_certificate: None,
        })
    }
    pub fn observe(&mut self, callback: impl Fn(ConnectionState) + Send + Sync + 'static) {
        self.observer = Some(Box::new(callback));
    }
    pub fn diagnostic_route(&mut self) -> crate::diagnostics::DiagnosticRoute {
        let s = self.snapshot();
        crate::diagnostics::DiagnosticRoute {
            direct_port: s.engine_pid.map(|_| self.direct_probe_port),
            vpn_port: s.engine_pid.map(|_| self.probe_port),
            password: Zeroizing::new(self.proxy_secret.to_string()),
            engine_pid: s.engine_pid,
        }
    }
    fn transition(&mut self, state: ConnectionState) {
        let _ = self.store.log(
            "INFO",
            "Connection",
            "state",
            "Transition",
            &format!("{:?}", state),
        );
        self.state = state.clone();
        if let Some(observer) = &self.observer {
            observer(state);
        }
    }
    fn log_error(&mut self, e: &Error) {
        self.error = Some(e.into());
        let _ = self
            .store
            .log("ERROR", "Connection", &e.stage, &e.code, &e.message);
    }
    pub fn snapshot(&mut self) -> Snapshot {
        if let Some(engine) = &self.engine {
            let text = engine.recent_diagnostic();
            for line in text.lines().filter(|s| !s.trim().is_empty()) {
                let line = line.chars().take(500).collect::<String>();
                if !self.recent_runtime_lines.contains(&line) {
                    let level = crate::runtime_log::level(&line);
                    let _ = self
                        .store
                        .log(level, "Core", "runtime-output", "RuntimeOutput", &line);
                    self.recent_runtime_lines.push_back(line);
                    if self.recent_runtime_lines.len() > 64 {
                        self.recent_runtime_lines.pop_front();
                    }
                }
            }
        }
        let status = self.engine.as_mut().map(ManagedEngine::alive);
        if let Some(Err(e)) = &status
            && self.error.as_ref().is_none_or(|old| old.code != e.code)
        {
            self.log_error(e);
        } else if matches!(status, Some(Ok(true)))
            && self.error.as_ref().is_some_and(|e| {
                matches!(
                    e.code.as_str(),
                    "IpcTimeout"
                        | "IoError"
                        | "ServiceTimeout"
                        | "ServiceBusy"
                        | "ServiceMonitorUnavailable"
                )
            })
        {
            self.error = None;
        }

        if matches!(status, Some(Ok(false))) {
            let e = Error::new(
                "EngineExited",
                "engine-monitor",
                "Network engine exited unexpectedly; local proxy has been stopped",
            );
            let _ = self.disconnect();
            self.transition(ConnectionState::Error);
            self.log_error(&e);
        }
        Snapshot {
            state: self.state.clone(),
            active: self.active.as_ref().map(Profile::summary),
            engine_pid: self.engine.as_ref().map(ManagedEngine::pid),
            metrics: self.metrics.clone(),
            metrics_error: self.metrics_error.clone(),
            error: self.error.clone(),
            probe_ms: self.probe_ms,
            system_proxy_active: self
                .proxy
                .as_ref()
                .is_some_and(|p| p.is_applied().unwrap_or(false)),
            tun_active: self.engine.is_some() && self.prefs.effective_tun(),
            applied_preferences: self.engine.as_ref().map(|_| self.prefs.clone()),
            recovery_pid: self.proxy.as_ref().and_then(ProxyLease::recovery_pid),
        }
    }
    pub fn select_profile(&mut self, id: &str) -> Result<()> {
        self.store.profile(id)?;
        let mut p = self.store.preferences()?;
        p.selected_profile = Some(id.into());
        self.store.save_preferences(&p)
    }
    pub fn connect(&mut self, id: &str) -> Result<Snapshot> {
        let previous = self.active.clone();
        let applied = self.prefs.clone();
        let next = self.store.profile(id)?;
        let mut prefs = self.store.preferences()?.normalized();
        prefs.selected_profile = Some(id.into());
        self.connect_with(next, &prefs)?;
        if let Err(e) = self.store.save_preferences(&prefs) {
            let _ = self.disconnect();
            if let Some(previous) = previous {
                let _ = self.connect_with(previous, &applied);
            }
            return Err(e);
        }
        Ok(self.snapshot())
    }
    pub fn apply_preferences(&mut self, prefs: Preferences) -> Result<()> {
        let mut prefs = prefs.normalized();
        // Window geometry is saved by the native window, not by stale form drafts.
        prefs.window = self.store.preferences()?.window;
        prefs.selected_profile = self.store.preferences()?.selected_profile;
        prefs.validate()?;
        let previous = self.store.preferences()?;
        let routing_changed = prefs.tun != self.prefs.tun
            || prefs.system_proxy != self.prefs.system_proxy
            || prefs.route_mode != self.prefs.route_mode
            || prefs.process_paths() != self.prefs.process_paths()
            || prefs.bypass_domains != self.prefs.bypass_domains
            || prefs.dns != self.prefs.dns
            || prefs.dns_route != self.prefs.dns_route
            || prefs.mtu != self.prefs.mtu
            || prefs.network_preset != self.prefs.network_preset
            || prefs.fake_ip != self.prefs.fake_ip
            || prefs.routing_rules != self.prefs.routing_rules
            || prefs.sniff_excluded_apps != self.prefs.sniff_excluded_apps
            || prefs.fragmentation != self.prefs.fragmentation
            || prefs.multiplex != self.prefs.multiplex
            || prefs.tls_defaults != self.prefs.tls_defaults
            || prefs.transport_engine != self.prefs.transport_engine
            || prefs.proxy_port != self.prefs.proxy_port
            || prefs.probe_url != self.prefs.probe_url;
        let active = self.active.clone();
        let applied = self.prefs.clone();
        if routing_changed && let Some(p) = &active {
            self.connect_with(p.clone(), &prefs)?;
        }
        if let Err(e) = self.store.save_preferences(&prefs) {
            if routing_changed && let Some(p) = active {
                let _ = self.connect_with(p, &applied);
            }
            return Err(e);
        }
        if self.engine.is_none() || !routing_changed {
            self.prefs = prefs;
        }
        let _ = self.store.log(
            "INFO",
            "Settings",
            "settings-save",
            "Saved",
            if routing_changed && self.engine.is_some() {
                "Settings saved and applied after verified runtime restart"
            } else if prefs_network_equal(&previous, &self.prefs) {
                "Settings saved"
            } else {
                "Settings saved; apply on next connection"
            },
        );
        Ok(())
    }
    fn connect_with(&mut self, next: Profile, prefs: &Preferences) -> Result<()> {
        let previous = self.active.clone();
        let old_prefs = self.prefs.clone();
        self.transition(if previous.is_some() {
            ConnectionState::Switching
        } else {
            ConnectionState::Validating
        });
        self.error = None;
        let result = (|| {
            engine::verify_engine(&self.engine_path)?;
            if prefs.system_proxy && self.proxy.is_none() {
                ProxyLease::check_conflict()?;
            }
            #[cfg(windows)]
            if prefs.effective_tun() {
                crate::service::ensure_started()?;
            }
            #[cfg(not(windows))]
            if prefs.effective_tun() {
                return Err(Error::new(
                    "PlatformUnsupported",
                    "tun-prepare",
                    "Windows service is required",
                ));
            }
            let port = free_port()?;
            let probe_port = free_port()?;
            let a = uuid::Uuid::new_v4().to_string();
            let b = uuid::Uuid::new_v4().to_string();
            let plan = RuntimePlan::new(
                &self.engine_path,
                &next,
                prefs,
                port,
                &a,
                &b,
                [probe_port, free_port()?],
            )?;
            plan.validate(&self.engine_path)?;
            if previous.is_some() && self.engine.is_some() {
                self.preflight(&next, prefs)?;
            }
            self.disconnect()?;
            self.start(next, prefs, port, a, b, &plan)
        })();
        if let Err(e) = result {
            if self.engine.is_none()
                && let Some(old) = previous
            {
                let rollback = (|| {
                    let port = free_port()?;
                    let probe_port = free_port()?;
                    let a = uuid::Uuid::new_v4().to_string();
                    let b = uuid::Uuid::new_v4().to_string();
                    let plan = RuntimePlan::new(
                        &self.engine_path,
                        &old,
                        &old_prefs,
                        port,
                        &a,
                        &b,
                        [probe_port, free_port()?],
                    )?;
                    self.start(old, &old_prefs, port, a, b, &plan)
                })();
                if let Err(rollback) = rollback {
                    let _ = self.store.log(
                        "ERROR",
                        "Connection",
                        &rollback.stage,
                        &rollback.code,
                        "Previous connection could not be restored; connection is stopped",
                    );
                }
            }
            self.transition(if self.engine.is_some() {
                ConnectionState::Connected
            } else {
                ConnectionState::Error
            });
            self.log_error(&e);
            return Err(e);
        }
        Ok(())
    }
    fn log_start_diagnostic(&self, text: &str) {
        for line in text.lines().filter(|line| !line.trim().is_empty()).take(24) {
            let safe: String = line.chars().take(500).collect();
            let _ = self.store.log(
                crate::runtime_log::level(&safe),
                "Core",
                "candidate-probe",
                "RuntimeOutput",
                &safe,
            );
        }
    }
    /// Test a candidate with private ports while the current session stays alive.
    /// Failed candidates never tear down a healthy TUN just to roll it back.
    fn preflight(&self, profile: &Profile, prefs: &Preferences) -> Result<()> {
        let mut isolated = prefs.clone();
        isolated.tun = false;
        isolated.system_proxy = false;
        isolated.route_mode = crate::model::RouteMode::Global;
        isolated.proxy_port = free_port()?;
        let secret = uuid::Uuid::new_v4().to_string();
        let plan = RuntimePlan::new(
            &self.engine_path,
            profile,
            &isolated,
            free_port()?,
            &uuid::Uuid::new_v4().to_string(),
            &secret,
            [free_port()?, free_port()?],
        )?;
        plan.validate(&self.engine_path)?;
        let mut candidate = plan.start(&self.engine_path)?;
        #[cfg(not(test))]
        let certificate = None;
        #[cfg(test)]
        let certificate = self.probe_certificate.as_ref();
        let checked = probe(plan.probe_port, &secret, &prefs.probe_url, certificate);
        if checked.is_err() {
            self.log_start_diagnostic(&candidate.diagnostic(&plan));
        }
        candidate.stop()?;
        checked.map(|_|()).map_err(|mut e| {
            e.message="Проверка нового сервера или ядра не прошла. Текущее подключение сохранено; подробности в журнале ядра.".into();e
        })
    }
    fn start(
        &mut self,
        profile: Profile,
        prefs: &Preferences,
        port: u16,
        a: String,
        b: String,
        plan: &RuntimePlan,
    ) -> Result<()> {
        self.transition(ConnectionState::Connecting);
        let mut engine = if prefs.effective_tun() {
            #[cfg(windows)]
            {
                ManagedEngine::Service(crate::service::ServiceEngine::start(
                    &profile,
                    prefs,
                    port,
                    [plan.probe_port, plan.direct_probe_port],
                    &a,
                    &b,
                )?)
            }
            #[cfg(not(windows))]
            {
                return Err(Error::new(
                    "PlatformUnsupported",
                    "tun-start",
                    "Windows service is required",
                ));
            }
        } else {
            ManagedEngine::Local(Box::new(plan.start(&self.engine_path)?))
        };
        #[cfg(not(test))]
        let certificate = None;
        #[cfg(test)]
        let certificate = self.probe_certificate.as_ref();
        let elapsed =
            probe(plan.probe_port, &b, &prefs.probe_url, certificate).inspect_err(|_| {
                self.log_start_diagnostic(&engine.diagnostic(plan));
            })?;
        if !engine.alive()? {
            return Err(Error::new(
                "EngineExited",
                "connection-probe",
                "Engine exited during the selected outbound HTTPS probe",
            ));
        }
        let connection_monitor =
            crate::connection_monitor::Monitor::new(port, Zeroizing::new(a.clone()))?;
        let proxy = if prefs.system_proxy {
            Some(ProxyLease::acquire(
                prefs.proxy_port,
                &self.store.data_directory().join("recovery"),
            )?)
        } else {
            None
        };
        self.proxy = proxy;
        self.connection_monitor = Some(connection_monitor);
        self.engine = Some(engine);
        self.active = Some(profile);
        self.prefs = prefs.clone();
        self.api_port = port;
        self.probe_port = plan.probe_port;
        self.direct_probe_port = plan.direct_probe_port;
        self.api_secret = Zeroizing::new(a);
        self.proxy_secret = Zeroizing::new(b);
        self.probe_ms = Some(elapsed);
        self.transition(ConnectionState::Connected);
        let _ = self.store.log(
            "INFO",
            "Core",
            "core-start",
            "Started",
            &format!(
                "Verified network runtime ready (transport: {})",
                self.prefs.transport_engine
            ),
        );
        if self.prefs.effective_tun() {
            let _ = self.store.log(
                "INFO",
                "Service",
                "service-start",
                "SessionReady",
                "Windows service owns the TUN session; IPC monitoring enabled",
            );
        }
        self.metrics = None;
        self.metrics_error = None;
        let _ = self.store.log(
            "INFO",
            "Connection",
            "connected",
            "Connected",
            "Selected outbound HTTPS probe succeeded; runtime is ready",
        );
        Ok(())
    }
    pub fn disconnect(&mut self) -> Result<Snapshot> {
        self.transition(ConnectionState::Disconnecting);
        let mut failure = None;
        // Keep the recovery lease until Windows confirms restoration. A transient
        // WinINET failure must not strand the user on a stopped local proxy.
        if let Some(proxy) = self.proxy.as_mut() {
            for attempt in 0..4 {
                match proxy.restore() {
                    Ok(()) => {
                        failure = None;
                        break;
                    }
                    Err(e) if e.code == "ProxyOwnershipLost" => {
                        failure = Some(e);
                        break;
                    }
                    Err(e) => failure = Some(e),
                }
                if attempt < 3 {
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
            if failure
                .as_ref()
                .is_some_and(|e| e.code != "ProxyOwnershipLost")
            {
                match proxy.recover_now() {
                    Ok(()) => failure = None,
                    Err(e) => failure = Some(e),
                }
            }
            if failure.is_none()
                || failure
                    .as_ref()
                    .is_some_and(|e| e.code == "ProxyOwnershipLost")
            {
                self.proxy = None;
            }
        }
        if let Some(e) = failure.as_ref().filter(|e| e.code != "ProxyOwnershipLost") {
            // Keep the listening engine available while restoration is retried.
            let e = Error::new(&e.code, &e.stage, &e.message);
            self.transition(ConnectionState::Error);
            self.log_error(&e);
            return Err(e);
        }
        if let Some(engine) = self.engine.as_mut()
            && let Err(e) = engine.stop()
        {
            self.transition(ConnectionState::Error);
            self.log_error(&e);
            return Err(e);
        }
        if self.engine.is_some() {
            let _ = self.store.log(
                "INFO",
                "Core",
                "core-stop",
                "Stopped",
                "Owned network runtime stopped",
            );
            if self.prefs.effective_tun() {
                let _ = self.store.log(
                    "INFO",
                    "Service",
                    "service-stop",
                    "SessionReleased",
                    "Windows service confirmed session cleanup",
                );
            }
        }
        self.engine = None;
        self.recent_runtime_lines.clear();
        self.active = None;
        self.metrics = None;
        self.probe_ms = None;
        self.connection_monitor = None;
        self.metrics_error = None;
        self.error = None;
        self.transition(ConnectionState::Disconnected);
        if let Some(e) = failure {
            // Another proxy client's settings are deliberately preserved. This
            // is not a failed VPN disconnect and must not trigger reconnect.
            let _ = self
                .store
                .log("WARNING", "Connection", &e.stage, &e.code, &e.message);
        }
        Ok(self.snapshot())
    }
    pub fn set_profile_transport(
        &mut self,
        id: &str,
        value: Option<crate::transport_options::ProfileTransport>,
    ) -> Result<()> {
        self.store.ensure_profile_editable(id)?;
        let previous = self.store.profile(id)?;
        let mut next = previous.clone();
        next.transport_override = value;
        self.commit_profile_change(previous, next)
    }
    pub fn set_profile_json(&mut self, id: &str, json: &str) -> Result<()> {
        self.store.ensure_profile_editable(id)?;
        if json.len() > 65536 {
            return Err(Error::new(
                "ProfileLimit",
                "profile-json",
                "Конфигурация превышает 64 КиБ",
            ));
        }
        let outbound = serde_json::from_str(json).map_err(|_| {
            Error::new(
                "InvalidJson",
                "profile-json",
                "Некорректный JSON. Проверьте запятые, кавычки и скобки.",
            )
        })?;
        let previous = self.store.profile(id)?;
        let mut next = previous.clone();
        next.outbound_override = Some(outbound);
        self.commit_profile_change(previous, next)
    }
    fn commit_profile_change(&mut self, previous: Profile, next: Profile) -> Result<()> {
        let id = next.id.clone();
        next.validate()?;
        let prefs = self.store.preferences()?;
        let plan = RuntimePlan::new(
            &self.engine_path,
            &next,
            &prefs,
            self.api_port,
            "validation",
            "validation",
            [self.probe_port, self.direct_probe_port],
        )?;
        plan.validate(&self.engine_path)?;
        let active = self.active.as_ref().is_some_and(|p| p.id == id);
        if active {
            self.connect_with(next.clone(), &prefs)?;
        }
        if let Err(error) = self.store.save_profile_transport(&next) {
            if active {
                let _ = self.connect_with(previous, &prefs);
            }
            return Err(error);
        }
        let _ = self.store.log(
            "INFO",
            "Profile",
            "profile-transport",
            "TransportChanged",
            "Profile SNI / Host settings updated",
        );
        Ok(())
    }
    pub fn analysis_request(&self, pid: Option<u32>, path: String) -> Result<AnalysisRequest> {
        if path.len() > 32768 {
            return Err(Error::new(
                "InvalidProcess",
                "playground",
                "Invalid process path",
            ));
        }
        Ok(AnalysisRequest {
            reader: self.connection_monitor.as_ref().map(|m| m.reader.clone()),
            diagnostic: self
                .engine
                .as_ref()
                .map(ManagedEngine::recent_diagnostic)
                .unwrap_or_default(),
            pid,
            path,
        })
    }
    pub fn refresh_metrics(&mut self) -> Result<Snapshot> {
        if let Some(monitor) = &self.connection_monitor {
            monitor.reader.request();
            let (metrics, error) = monitor.reader.cached_metrics();
            if let Some(metrics) = metrics {
                self.metrics = Some(metrics);
            }
            self.metrics_error = error.as_ref().map(ErrorView::from);
        }
        Ok(self.snapshot())
    }
    pub fn update_subscription(&mut self, id: &str) -> Result<ImportReport> {
        let result = (|| {
            let url = self.store.subscription_url(id)?;
            self.store
                .import(fetch_subscription(url.as_str())?, Some(id))
        })();
        match &result {
            Ok(r) => {
                let _ = self.store.log(
                    "INFO",
                    "Subscriptions",
                    "subscription-update",
                    "Imported",
                    &format!(
                        "Imported {} profiles; {} incompatible entries",
                        r.imported,
                        r.issues.len()
                    ),
                );
            }
            Err(e) => self.log_error(e),
        };
        result
    }
    pub fn public_ip(&self) -> Result<String> {
        if self.engine.is_none() {
            return Err(Error::new(
                "NotConnected",
                "public-ip",
                "Connect before checking the routed public IP",
            ));
        }
        let proxy = reqwest::Proxy::all(format!("http://127.0.0.1:{}", self.probe_port))
            .map_err(|_| Error::new("ProxyError", "public-ip", "Invalid local endpoint"))?
            .basic_auth("netpulse", &self.proxy_secret);
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .proxy(proxy)
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| Error::new("TlsError", "public-ip", "Could not initialize TLS client"))?;
        let response = client.get("https://api.ipify.org").send().map_err(|_| {
            Error::new(
                "PublicIpFailed",
                "public-ip",
                "Public IP check failed through the active route",
            )
        })?;
        let mut text = String::new();
        response
            .take(128)
            .read_to_string(&mut text)
            .map_err(|e| Error::io("public-ip", e))?;
        text.trim()
            .parse::<std::net::IpAddr>()
            .map(|ip| ip.to_string())
            .map_err(|_| {
                Error::new(
                    "PublicIpInvalid",
                    "public-ip",
                    "Public IP service returned an invalid address",
                )
            })
    }
    pub fn local_proxy_uri(&self) -> Result<Zeroizing<String>> {
        if self.engine.is_none() {
            return Err(Error::new(
                "NotConnected",
                "proxy-copy",
                "Connect before copying the local proxy endpoint",
            ));
        }
        if self.prefs.system_proxy {
            return Ok(Zeroizing::new(format!(
                "http://127.0.0.1:{}",
                self.prefs.proxy_port
            )));
        }
        Ok(Zeroizing::new(format!(
            "http://netpulse:{}@127.0.0.1:{}",
            self.proxy_secret.as_str(),
            self.prefs.proxy_port
        )))
    }
}
/// Prepared under the controller lock, executed on a worker without that lock.
pub struct AnalysisRequest {
    reader: Option<crate::connection_monitor::Reader>,
    diagnostic: String,
    pid: Option<u32>,
    path: String,
}
impl AnalysisRequest {
    pub fn run(self) -> Result<crate::playground::Analysis> {
        let mut rows = Vec::new();
        let mut errors = Vec::new();
        let mut alive = true;
        if let Some(pid) = self.pid {
            let result = crate::playground::sockets(pid, &self.path)?;
            alive = result.0;
            rows = result.1;
        }
        // A collapsed EXE group has no single PID: inspect every live instance.
        if self.pid.is_none() && !self.path.is_empty() {
            let name=crate::app_rules::executable_name(&self.path);
            alive=false;
            for p in crate::playground::processes()?.into_iter().filter(|p|crate::app_rules::executable_name(&p.path)==name).take(128) {
                if let Ok((running,mut sockets))=crate::playground::sockets(p.pid,&p.path){alive|=running;rows.append(&mut sockets);}
                if rows.len()>=300{rows.truncate(300);break;}
            }
        }
        let mut truncated = rows.len() >= 300;
        let mut core_available = false;
        if let Some(reader) = self.reader {
            match reader.fresh() {
                Ok(sample) => {
                    core_available = true;
                    truncated |= sample.metrics.connections > sample.rows.len();
                    let mut core_rows = sample.rows.clone();
                    if let Some(pid) = self.pid {
                        core_rows.retain(|c| {
                            rows.iter()
                                .any(|os| os.network == c.network && os.local == c.local)
                        });
                        for c in &mut core_rows {
                            c.pid = Some(pid);
                            c.process = self.path.clone();
                        }
                    }
                    if self.pid.is_none() && !self.path.is_empty() {
                        let name=crate::app_rules::executable_name(&self.path);
                        core_rows.retain(|c|crate::app_rules::executable_name(&c.process)==name || rows.iter().any(|os|os.network==c.network&&os.local==c.local));
                    }
                    // A matched Windows socket and core row describe the same flow.
                    rows.retain(|os|!core_rows.iter().any(|c|os.network==c.network&&os.local==c.local));
                    rows.splice(0..0, core_rows);
                }
                Err(e) => errors.push(format!("{}: {}", e.code, e.message)),
            }
        }
        errors.extend(
            self.diagnostic
                .lines()
                .filter(|line| matches!(crate::runtime_log::level(line), "ERROR" | "WARNING"))
                .take(20)
                .map(str::to_owned),
        );
        rows.truncate(600);
        Ok(crate::playground::Analysis {
            connections: rows,
            process_alive: alive,
            errors,
            core_available,
            truncated,
        })
    }
}
fn prefs_network_equal(a: &Preferences, b: &Preferences) -> bool {
    a.tun == b.tun
        && a.system_proxy == b.system_proxy
        && a.route_mode == b.route_mode
        && a.process_paths() == b.process_paths()
        && a.bypass_domains == b.bypass_domains
        && a.proxy_port == b.proxy_port
        && a.dns == b.dns
        && a.dns_route == b.dns_route
        && a.mtu == b.mtu
        && a.network_preset == b.network_preset
        && a.fake_ip == b.fake_ip
        && a.routing_rules == b.routing_rules
        && a.sniff_excluded_apps == b.sniff_excluded_apps
        && a.fragmentation == b.fragmentation
        && a.multiplex == b.multiplex
        && a.transport_engine == b.transport_engine
        && a.probe_url == b.probe_url
}
#[derive(serde::Serialize)]
pub struct PublicIps {
    pub vpn: Option<String>,
    pub direct: Option<String>,
    pub vpn_error: Option<Error>,
    pub direct_error: Option<Error>,
}
pub fn public_ips(route: crate::diagnostics::DiagnosticRoute) -> PublicIps {
    fn fetch(port: Option<u16>, password: &str) -> Result<String> {
        let mut builder = reqwest::blocking::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none());
        if let Some(port) = port {
            builder = builder.proxy(
                reqwest::Proxy::all(format!("http://127.0.0.1:{port}"))
                    .map_err(|_| Error::new("ProxyError", "public-ip", "Неверный endpoint"))?
                    .basic_auth("netpulse", password),
            );
        }
        let response = builder
            .build()
            .map_err(|_| Error::new("TlsError", "public-ip", "Не удалось создать TLS клиент"))?
            .get("https://api.ipify.org")
            .send()
            .map_err(|_| {
                Error::new(
                    "PublicIpFailed",
                    "public-ip",
                    "Сервис IP не отвечает по выбранному маршруту",
                )
            })?;
        if !response.status().is_success() {
            return Err(Error::new(
                "PublicIpFailed",
                "public-ip",
                "Сервис IP вернул ошибку",
            ));
        }
        let mut text = String::new();
        response
            .take(128)
            .read_to_string(&mut text)
            .map_err(|e| Error::io("public-ip", e))?;
        text.trim()
            .parse::<std::net::IpAddr>()
            .map(|p| p.to_string())
            .map_err(|_| {
                Error::new(
                    "PublicIpInvalid",
                    "public-ip",
                    "Сервис вернул неверный IP адрес",
                )
            })
    }
    let (direct,vpn)=std::thread::scope(|scope| {
        let direct=scope.spawn(||fetch(route.direct_port,&route.password));
        let vpn=route.vpn_port.map(|port|fetch(Some(port),&route.password));
        (direct.join().unwrap_or_else(|_|Err(Error::new("PublicIpFailed","public-ip","Direct IP check failed"))),vpn)
    });
    PublicIps {
        direct: direct.as_ref().ok().cloned(),
        direct_error: direct.err(),
        vpn: vpn.as_ref().and_then(|r| r.as_ref().ok().cloned()),
        vpn_error: vpn.and_then(Result::err),
    }
}
fn free_port() -> Result<u16> {
    Ok(std::net::TcpListener::bind("127.0.0.1:0")
        .map_err(|e| Error::io("api-port", e))?
        .local_addr()
        .map_err(|e| Error::io("api-port", e))?
        .port())
}
pub fn client(timeout: Duration) -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() > 5 || attempt.url().scheme() != "https" {
                attempt.stop()
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|_| {
            Error::new(
                "TlsError",
                "http-client",
                "Could not initialize Windows TLS client",
            )
        })
}
fn probe(
    port: u16,
    password: &str,
    url: &str,
    certificate: Option<&reqwest::Certificate>,
) -> Result<u64> {
    let proxy = reqwest::Proxy::all(format!("http://127.0.0.1:{port}"))
        .map_err(|_| Error::new("ProxyError", "connection-probe", "Invalid local endpoint"))?
        .basic_auth("netpulse", password);
    let mut builder = reqwest::blocking::Client::builder()
        .no_proxy()
        .proxy(proxy)
        .timeout(Duration::from_secs(12))
        .redirect(reqwest::redirect::Policy::none());
    if let Some(cert) = certificate {
        builder = builder.add_root_certificate(cert.clone());
    }
    let client = builder.build().map_err(|_| {
        Error::new(
            "TlsError",
            "connection-probe",
            "Could not initialize Windows TLS client",
        )
    })?;
    let start = Instant::now();
    let response=client.get(url).send().map_err(|e|{
        Error::new(if e.is_timeout(){"ProbeTimeout"}else if e.is_connect(){"ProbeConnectionFailed"}else{"ProbeTlsOrTransportFailed"},"connection-probe","HTTPS probe through the selected profile failed; check server, credentials, TLS and transport")})?;
    if !response.status().is_success() {
        return Err(Error::new(
            "ProbeHttpError",
            "connection-probe",
            format!("HTTPS probe returned HTTP {}", response.status().as_u16()),
        ));
    }
    Ok(start.elapsed().as_millis() as u64)
}
impl Drop for NetworkCore {
    fn drop(&mut self) {
        let _ = self.disconnect();
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::engine::Engine;
    use serde_json::json;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };
    fn path() -> PathBuf {
        std::env::var_os("NETPULSE_TEST_ENGINE")
            .map(PathBuf::from)
            .expect("Set NETPULSE_TEST_ENGINE to the pinned official executable")
    }
    fn upstream() -> (Engine, u16) {
        let port = free_port().unwrap();
        let config = json!({"log":{"disabled":true},"inbounds":[{"type":"mixed","listen":"127.0.0.1","listen_port":port,"users":[{"username":"fixture","password":"fixture-only"}]}],"outbounds":[{"type":"direct","tag":"direct"}]});
        Engine::validate(&path(), &config).unwrap();
        let mut e = Engine::spawn(&path(), &config, false).unwrap();
        for _ in 0..100 {
            if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
                return (e, port);
            }
            assert!(e.alive().unwrap());
            std::thread::sleep(Duration::from_millis(30));
        }
        panic!("Owned upstream failed")
    }
    struct Origin {
        port: u16,
        stop: Arc<AtomicBool>,
        join: Option<std::thread::JoinHandle<()>>,
    }
    impl Origin {
        fn new() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            listener.set_nonblocking(true).unwrap();
            let identity = native_tls::Identity::from_pkcs12(
                include_bytes!("../tests/fixtures/owned-localhost.pfx"),
                "fixture-only",
            )
            .unwrap();
            let acceptor = native_tls::TlsAcceptor::new(identity).unwrap();
            let stop = Arc::new(AtomicBool::new(false));
            let s = stop.clone();
            let join = std::thread::spawn(move || {
                while !s.load(Ordering::Relaxed) {
                    if let Ok((socket, _)) = listener.accept() {
                        socket.set_nonblocking(false).unwrap();
                        socket
                            .set_read_timeout(Some(Duration::from_secs(3)))
                            .unwrap();
                        if let Ok(mut tls) = acceptor.accept(socket) {
                            let mut bytes = [0u8; 8192];
                            if let Ok(n) = tls.read(&mut bytes) {
                                let request = String::from_utf8_lossy(&bytes[..n]);
                                if request.starts_with("GET /download") {
                                    let _=tls.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2097152\r\nConnection: close\r\n\r\n");
                                    for _ in 0..256 {
                                        if tls.write_all(&[7u8; 8192]).is_err() {
                                            break;
                                        }
                                    }
                                } else if request.starts_with("HEAD ") {
                                    let _=tls.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                                } else if request.starts_with("POST ") {
                                    let size = request
                                        .lines()
                                        .find_map(|line| {
                                            line.split_once(':')
                                                .filter(|(k, _)| {
                                                    k.eq_ignore_ascii_case("content-length")
                                                })
                                                .and_then(|(_, v)| v.trim().parse::<usize>().ok())
                                        })
                                        .unwrap_or(0);
                                    let head = bytes[..n]
                                        .windows(4)
                                        .position(|v| v == b"\r\n\r\n")
                                        .map(|i| i + 4)
                                        .unwrap_or(n);
                                    let mut read = n - head;
                                    while read < size {
                                        match tls.read(&mut bytes) {
                                            Ok(0) | Err(_) => break,
                                            Ok(n) => read += n,
                                        }
                                    }
                                    let _=tls.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                                } else {
                                    let _=tls.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\nNETPULSE");
                                }
                                let _ = tls.flush();
                            }
                        }
                    } else {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                }
            });
            Self {
                port,
                stop,
                join: Some(join),
            }
        }
    }
    impl Drop for Origin {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            if let Some(t) = self.join.take() {
                let _ = t.join();
            }
        }
    }
    #[test]
    fn owned_https_speed_caps_upload_and_cancel() {
        use crate::diagnostics::{DiagnosticRoute, SpeedRequest, speed};
        let origin = Origin::new();
        let request = SpeedRequest {
            mode: "direct".into(),
            download_url: format!("https://127.0.0.1:{}/download", origin.port),
            upload_url: Some(format!("https://127.0.0.1:{}/upload", origin.port)),
            upload_endpoint_confirmed: true,
            duration_seconds: 2,
            max_payload_mib: 1,
        };
        let route = DiagnosticRoute {
            direct_port: None,
            vpn_port: None,
            password: Zeroizing::new(String::new()),
            engine_pid: None,
        };
        let result = speed(
            &request,
            &route,
            "direct",
            128 * 1024,
            &AtomicBool::new(false),
        );
        assert!(result.error.is_none(), "{:?}", result.error);
        assert_eq!(result.download_bytes, 64 * 1024);
        assert_eq!(result.upload_bytes, 64 * 1024);
        assert!(result.download_mbps.is_some_and(|v| v > 0.0));
        assert!(result.upload_mbps.is_some_and(|v| v > 0.0));
        assert!(result.idle_https_ms.is_some());
        let result = speed(
            &request,
            &route,
            "direct",
            128 * 1024,
            &AtomicBool::new(true),
        );
        assert!(result.cancelled);
        assert!(result.download_mbps.is_none());
        assert_eq!(result.download_bytes, 0);
    }
    #[test]
    #[ignore = "Requires pinned sing-box; owned loopback fixtures only"]
    fn real_connection_switch_rollback_metrics_and_cleanup() {
        engine::verify_engine(&path()).unwrap();
        let origin = Origin::new();
        let (_first, first_port) = upstream();
        let (_second, second_port) = upstream();
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(&dir.path().join("test.db")).unwrap();
        let prefs = Preferences {
            proxy_port: free_port().unwrap(),
            probe_url: format!("https://127.0.0.1:{}/", origin.port),
            ..Preferences::default()
        };
        store.save_preferences(&prefs).unwrap();
        let bad_port = free_port().unwrap();
        let text = format!(
            "socks5://fixture:fixture-only@127.0.0.1:{first_port}#First\nsocks5://fixture:fixture-only@127.0.0.1:{second_port}#Second\nsocks5://fixture:fixture-only@127.0.0.1:{bad_port}#Unavailable"
        );
        store
            .import(crate::imports::parse(&text).unwrap(), None)
            .unwrap();
        let profiles = store.profiles().unwrap();
        let mut core = NetworkCore::new(store, path()).unwrap();
        core.probe_certificate = Some(
            reqwest::Certificate::from_pem(include_bytes!("../tests/fixtures/owned-ca.pem"))
                .unwrap(),
        );
        let a = core.connect(&profiles[0].id).unwrap();
        assert_eq!(a.state, ConnectionState::Connected);
        let first_pid = a.engine_pid.unwrap();
        let active_latency = crate::diagnostics::latency_on_route(
            core.store.profile(&profiles[0].id).unwrap(),
            core.store.preferences().unwrap(),
            path(),
            "proxy_https_rtt",
            &AtomicBool::new(false),
            Some(&core.diagnostic_route()),
        );
        assert_eq!(active_latency.successes, 2, "{:?}", active_latency.error);
        assert_eq!(core.snapshot().engine_pid, Some(first_pid));
        let mut draft = core.store.preferences().unwrap();
        draft.proxy_port = free_port().unwrap();
        core.store.test_read_only(true);
        assert!(core.apply_preferences(draft.clone()).is_err());
        core.store.test_read_only(false);
        assert_eq!(
            core.store.preferences().unwrap().proxy_port,
            prefs.proxy_port
        );
        assert_eq!(core.snapshot().active.unwrap().id, profiles[0].id);
        // A rejected settings transaction must restore the previous working runtime.
        assert!(core.snapshot().engine_pid.is_some());
        assert!(std::net::TcpStream::connect(("127.0.0.1", draft.proxy_port)).is_err());
        let endpoint = core.local_proxy_uri().unwrap();
        let user_client = reqwest::blocking::Client::builder()
            .no_proxy()
            .proxy(reqwest::Proxy::all(endpoint.as_str()).unwrap())
            .add_root_certificate(
                reqwest::Certificate::from_pem(include_bytes!("../tests/fixtures/owned-ca.pem"))
                    .unwrap(),
            )
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        assert_eq!(
            user_client
                .get(&prefs.probe_url)
                .send()
                .unwrap()
                .text()
                .unwrap(),
            "NETPULSE"
        );
        let b = core.connect(&profiles[1].id).unwrap();
        assert_eq!(b.active.unwrap().id, profiles[1].id);
        assert_ne!(b.engine_pid.unwrap(), first_pid);
        assert!(core.connect(&profiles[2].id).is_err());
        let rolled = core.snapshot();
        assert_eq!(
            rolled.engine_pid, b.engine_pid,
            "failed preflight must preserve the current runtime and its connections"
        );
        assert_eq!(rolled.state, ConnectionState::Connected);
        assert_eq!(rolled.active.unwrap().id, profiles[1].id);
        let until = Instant::now() + Duration::from_secs(3);
        let metrics = loop {
            if let Some(metrics) = core.refresh_metrics().unwrap().metrics {
                break metrics;
            }
            assert!(Instant::now() < until, "background metrics did not arrive");
            std::thread::sleep(Duration::from_millis(20));
        };
        assert!(metrics.download_total > 0);
        assert!(metrics.upload_total > 0);
        let saved = core.store.backup().unwrap();
        core.disconnect().unwrap();
        assert!(std::net::TcpStream::connect(("127.0.0.1", prefs.proxy_port)).is_err());
        core.store.clear_all().unwrap();
        core.store.restore(&saved).unwrap();
        assert_eq!(core.store.profiles().unwrap().len(), 3);
        assert_eq!(
            core.store.preferences().unwrap().proxy_port,
            prefs.proxy_port
        );
        core.connect(&profiles[0].id).unwrap();
        core.engine.as_mut().unwrap().stop().unwrap();
        let crashed = core.refresh_metrics().unwrap();
        assert_eq!(crashed.state, ConnectionState::Error);
        assert!(crashed.engine_pid.is_none());
        assert_eq!(crashed.error.unwrap().code, "EngineExited");
    }
    #[test]
    #[ignore = "Requires pinned sing-box; validates configurations without applying routes"]
    fn real_engine_accepts_all_supported_protocols_and_route_modes() {
        let key =
            base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, [1u8; 32]);
        let uris=vec!["socks5://user:pass@example.org:1080".into(),"https://user:pass@example.org:443".into(),"trojan://pass@example.org:443?type=ws&path=%2Fproxy".into(),"hy2://pass@example.org:443?obfs=salamander&obfs-password=test".into(),"ss://YWVzLTEyOC1nY206cGFzcw@example.org:443".into(),format!("vless://15bc786a-b01b-4a8a-962e-cb52e089e500@example.org:443?security=reality&pbk={key}&sid=abcd&flow=xtls-rprx-vision"),"vless://15bc786a-b01b-4a8a-962e-cb52e089e500@example.org:443?type=grpc&serviceName=test".into()];
        let mut uris = uris;
        let fm = percent_encoding::utf8_percent_encode(
            r#"{"quicParams":{"debug":false,"congestion":"bbr"}}"#,
            percent_encoding::NON_ALPHANUMERIC,
        );
        uris.push(format!(
            "hysteria2://fixture@example.org:443/?sni=example.org&fm={fm}"
        ));
        uris.push(format!("vless://15bc786a-b01b-4a8a-962e-cb52e089e500@example.org:443?encryption=none&type=grpc&serviceName=fixture_grpc&mode=gun&security=reality&sni=example.org&fp=firefox&pbk={key}&sid=0123456789abcdef"));
        for uri in uris {
            let p = crate::imports::parse_uri(&uri).unwrap();
            for mode in [
                crate::model::RouteMode::Global,
                crate::model::RouteMode::Direct,
                crate::model::RouteMode::Whitelist,
                crate::model::RouteMode::Blacklist,
            ] {
                let prefs = Preferences {
                    tun: matches!(
                        mode,
                        crate::model::RouteMode::Whitelist | crate::model::RouteMode::Blacklist
                    ),
                    route_mode: mode,
                    whitelist: vec![r"C:\Apps\Telegram.exe".into()],
                    blacklist: vec![r"C:\Apps\Telegram.exe".into()],
                    ..Preferences::default()
                };
                let c = engine::configuration(
                    &p,
                    &prefs,
                    free_port().unwrap(),
                    "api-secret",
                    "proxy-secret",
                )
                .unwrap();
                Engine::validate(&path(), &c).unwrap();
            }
        }
    }
    #[test]
    #[ignore = "Requires bundled sing-box; checks only, no system changes"]
    fn real_engine_accepts_dns_routes_mtu_and_shared_app_rules() {
        use crate::model::{Dns, RouteMode};
        let profile = crate::imports::parse_uri("socks5://fixture:pass@example.org:1080").unwrap();
        for dns in [
            Dns::Udp {
                server: "1.1.1.1".into(),
            },
            Dns::Tls {
                server: "dns.example.org".into(),
            },
            Dns::Https {
                server: "dns.example.org".into(),
                path: "/dns-query".into(),
            },
        ] {
            for dns_route in ["direct", "vpn"] {
                for route_mode in [
                    RouteMode::Global,
                    RouteMode::Whitelist,
                    RouteMode::Blacklist,
                ] {
                    let p = Preferences {
                        dns: dns.clone(),
                        dns_route: dns_route.into(),
                        mtu: 1400,
                        route_mode,
                        tun: true,
                        applications: vec![r"C:\Apps\fixture.exe".into()],
                        applications_migrated: true,
                        ..Preferences::default()
                    };
                    let c =
                        engine::configuration(&profile, &p, free_port().unwrap(), "api", "proxy")
                            .unwrap();
                    Engine::validate(&path(), &c).unwrap();
                }
            }
        }
    }
    #[test]
    fn ipc_timeout_preserves_connection_until_stop_is_confirmed() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("state.db")).unwrap();
        let mut core = NetworkCore::new(store, dir.path().join("sing-box.exe")).unwrap();
        core.active = Some(crate::imports::parse_uri("socks5://127.0.0.1:1080#Fixture").unwrap());
        core.engine = Some(ManagedEngine::FaultFixture {
            monitor_error: true,
            stop_error: true,
        });
        core.state = ConnectionState::Connected;
        for _ in 0..3 {
            let snapshot = core.snapshot();
            assert_eq!(snapshot.engine_pid, Some(12345));
            assert_eq!(snapshot.state, ConnectionState::Connected);
            assert!(snapshot.active.is_some());
        }
        assert!(core.disconnect().is_err());
        assert_eq!(core.snapshot().engine_pid, Some(12345));
        if let Some(ManagedEngine::FaultFixture {
            monitor_error,
            stop_error,
        }) = core.engine.as_mut()
        {
            *monitor_error = false;
            *stop_error = false;
        }
        assert_eq!(
            core.disconnect().unwrap().state,
            ConnectionState::Disconnected
        );
        assert!(core.snapshot().engine_pid.is_none());
    }
    #[test]
    fn command_panic_does_not_poison_core_and_disconnect_still_works() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("state.db")).unwrap();
        let mut core = NetworkCore::new(store, dir.path().join("sing-box.exe")).unwrap();
        core.active = Some(crate::imports::parse_uri("socks5://127.0.0.1:1080#Fixture").unwrap());
        core.engine = Some(ManagedEngine::FaultFixture {
            monitor_error: false,
            stop_error: false,
        });
        core.state = ConnectionState::Connected;
        let mutex = std::sync::Mutex::new(core);
        {
            let mut core = mutex.lock().unwrap();
            let error = core
                .guarded::<()>(|_| panic!("owned panic fixture"))
                .unwrap_err();
            assert_eq!(error.code, "CoreOperationFailed");
        }
        assert!(!mutex.is_poisoned());
        let mut core = mutex.lock().unwrap();
        assert!(core.snapshot().engine_pid.is_none());
        assert_eq!(
            core.disconnect().unwrap().state,
            ConnectionState::Disconnected
        );
    }
    #[test]
    #[ignore = "Requires bundled sing-box; owned HTTPS keepalive fixtures only"]
    fn latency_get_head_double_and_keepalive_use_real_requests() {
        use std::sync::atomic::AtomicUsize;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let identity = native_tls::Identity::from_pkcs12(
            include_bytes!("../tests/fixtures/owned-localhost.pfx"),
            "fixture-only",
        )
        .unwrap();
        let acceptor = native_tls::TlsAcceptor::new(identity).unwrap();
        let connections = Arc::new(AtomicUsize::new(0));
        let requests = Arc::new(std::sync::Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let server = {
            let connections = connections.clone();
            let requests = requests.clone();
            let stop = stop.clone();
            std::thread::spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    if let Ok((socket, _)) = listener.accept() {
                        let acceptor = acceptor.clone();
                        let connections = connections.clone();
                        let requests = requests.clone();
                        std::thread::spawn(move || {
                            socket.set_nonblocking(false).unwrap();
                            socket
                                .set_read_timeout(Some(Duration::from_secs(2)))
                                .unwrap();
                            if let Ok(mut stream) = acceptor.accept(socket) {
                                connections.fetch_add(1, Ordering::SeqCst);
                                loop {
                                    let mut header = Vec::new();
                                    let mut chunk = [0u8; 2048];
                                    while !header.ends_with(b"\r\n\r\n") {
                                        match stream.read(&mut chunk) {
                                            Ok(0) | Err(_) => return,
                                            Ok(n) => header.extend_from_slice(&chunk[..n]),
                                        }
                                        if header.len() > 16384 {
                                            return;
                                        }
                                    }
                                    requests.lock().unwrap().push(
                                        String::from_utf8_lossy(&header)
                                            .split_whitespace()
                                            .next()
                                            .unwrap()
                                            .to_owned(),
                                    );
                                    if stream.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: keep-alive\r\n\r\n").is_err(){return;}
                                }
                            }
                        });
                    } else {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                }
            })
        };
        let (_upstream, upstream_port) = upstream();
        let profile = crate::imports::parse_uri(&format!(
            "socks5://fixture:fixture-only@127.0.0.1:{upstream_port}#OwnedProxy"
        ))
        .unwrap();
        for method in ["proxy_https_rtt", "proxy_https_head_rtt"] {
            for mode in ["double", "keepalive"] {
                let before = connections.load(Ordering::SeqCst);
                let request_count = requests.lock().unwrap().len();
                let prefs = Preferences {
                    probe_url: format!("https://127.0.0.1:{port}/"),
                    ping_proxy_mode: mode.into(),
                    ..Preferences::default()
                };
                let result = crate::diagnostics::latency(
                    profile.clone(),
                    prefs,
                    path(),
                    method,
                    &AtomicBool::new(false),
                );
                assert_eq!(result.successes, 2, "{:?}", result.error);
                assert!(result.min_ms.is_some());
                assert_eq!(
                    connections.load(Ordering::SeqCst) - before,
                    if mode == "keepalive" { 1 } else { 2 }
                );
                let measured = requests.lock().unwrap();
                assert_eq!(measured.len() - request_count, 2);
                assert!(measured[request_count..].iter().all(|m| m
                    == if method == "proxy_https_head_rtt" {
                        "HEAD"
                    } else {
                        "GET"
                    }));
            }
        }
        stop.store(true, Ordering::SeqCst);
        server.join().unwrap();
    }
}

/// Downloads and parses outside the connection mutex; callers commit atomically.
pub fn fetch_subscription(url: &str) -> Result<crate::imports::Parsed> {
    let parsed_url = url::Url::parse(url).map_err(|_| {
        Error::new(
            "InvalidSubscription",
            "subscription-download",
            "Invalid HTTPS subscription",
        )
    })?;
    if parsed_url.scheme() != "https"
        || parsed_url.host_str().is_none()
        || !parsed_url.username().is_empty()
        || parsed_url.password().is_some()
    {
        return Err(Error::new(
            "InvalidSubscription",
            "subscription-download",
            "Subscription requires HTTPS without userinfo",
        ));
    }
    let c = client(Duration::from_secs(20))?;
    let response = c
        .get(url)
        .header("User-Agent", "NetPulse/3.0")
        .send()
        .map_err(|_| {
            Error::new(
                "SubscriptionDownloadFailed",
                "subscription-download",
                "Subscription HTTPS request failed; verify address, certificate and connectivity",
            )
        })?;
    if !response.status().is_success() {
        return Err(Error::new(
            "SubscriptionHttpError",
            "subscription-download",
            format!(
                "Subscription server returned HTTP {}",
                response.status().as_u16()
            ),
        ));
    }
    if response.url().scheme() != "https" {
        return Err(Error::new(
            "InsecureRedirect",
            "subscription-download",
            "Subscription redirected away from HTTPS",
        ));
    }
    let usage = response
        .headers()
        .get("subscription-userinfo")
        .and_then(|h| h.to_str().ok())
        .and_then(crate::store::SubscriptionUsage::parse);
    let mut bytes = Zeroizing::new(Vec::new());
    response
        .take(crate::imports::MAX_IMPORT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| Error::io("subscription-download", e))?;
    if bytes.len() > crate::imports::MAX_IMPORT {
        return Err(Error::new(
            "ImportLimit",
            "subscription-download",
            "Subscription exceeds 8 MiB",
        ));
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| {
        Error::new(
            "ImportError",
            "subscription-parse",
            "Subscription must be UTF-8",
        )
    })?;
    let mut parsed = crate::imports::parse(text)?;
    parsed.usage = usage;
    Ok(parsed)
}
