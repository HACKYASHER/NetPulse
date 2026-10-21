use super::*;
use netpulse_core::{client_updates, diagnostics, selection};
use std::sync::atomic::Ordering;
type UpdateResult = Result<Option<client_updates::Release>>;
#[derive(Default)]
pub struct UpdateCheck {
    last: Mutex<Option<(std::time::Instant, UpdateResult)>>,
}
impl UpdateCheck {
    fn cached(&self, force: bool, check: impl FnOnce() -> UpdateResult) -> UpdateResult {
        let mut last = self.last.lock().map_err(|_| {
            Error::new(
                "ClientUpdateUnavailable",
                "client-check",
                "Update worker unavailable",
            )
        })?;
        if !force
            && let Some((at, result)) = &*last
            && at.elapsed().as_secs() < 60
        {
            return result.clone();
        }
        let result = check();
        *last = Some((std::time::Instant::now(), result.clone()));
        result
    }
    fn check(&self, force: bool) -> UpdateResult {
        self.cached(force, || client_updates::check(env!("CARGO_PKG_VERSION")))
    }
}
pub fn start_update_monitor(app: tauri::AppHandle, directory: PathBuf, updates: Arc<UpdateCheck>) {
    // Native worker runs even when the autostart WebView is hidden/throttled.
    // No automatic download or installer launch, and no connection mutex held.
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(5));
        loop {
            let result = updates.check(false);
            let (level, code, message) = match &result {
                Ok(Some(_)) => ("INFO", "Available", "Client update available"),
                Ok(None) => ("INFO", "UpToDate", "Client is up to date"),
                Err(_) => (
                    "WARNING",
                    "CheckUnavailable",
                    "Could not check client update; will retry later",
                ),
            };
            let _ = netpulse_core::audit::append(
                &directory,
                &LogEvent {
                    id: uuid::Uuid::new_v4().to_string(),
                    timestamp: netpulse_core::store::now(),
                    level: level.into(),
                    subsystem: "Update".into(),
                    stage: "client-check".into(),
                    code: code.into(),
                    message: message.into(),
                },
            );
            if let Ok(release) = result {
                let _ = app.emit("client-release", release);
            }
            std::thread::sleep(std::time::Duration::from_secs(6 * 60 * 60));
        }
    });
}
pub fn subscription_monitor(app: tauri::AppHandle, core: Arc<Mutex<NetworkCore>>) {
    std::thread::spawn(move || {
        let mut attempts = std::collections::HashMap::<String, std::time::Instant>::new();
        loop {
            std::thread::sleep(std::time::Duration::from_secs(60));
            let due = {
                let Ok(c) = core.try_lock() else {
                    continue;
                };
                let Ok(subscriptions) = c.store.subscriptions() else {
                    continue;
                };
                attempts.retain(|id, _| subscriptions.iter().any(|s| s.id == *id));
                subscriptions
                    .into_iter()
                    .filter(|s| {
                        s.due(netpulse_core::store::now())
                            && attempts
                                .get(&s.id)
                                .is_none_or(|at| at.elapsed().as_secs() >= 15 * 60)
                    })
                    .take(20)
                    .collect::<Vec<_>>()
            };
            for s in due {
                let request = {
                    let Ok(c) = core.try_lock() else {
                        break;
                    };
                    c.store.subscription_url(&s.id)
                };
                let Ok(url) = request else {
                    continue;
                };
                attempts.insert(s.id.clone(), std::time::Instant::now());
                let result = netpulse_core::network::fetch_subscription(url.as_str());
                let Ok(mut c) = core.lock() else {
                    break;
                };
                // Discard responses made obsolete by deletion, manual refresh
                // or an interval change while the request was in flight.
                let current = c
                    .store
                    .subscriptions()
                    .ok()
                    .and_then(|v| v.into_iter().find(|v| v.id == s.id));
                if !current.is_some_and(|v| {
                    v.updated_at == s.updated_at && v.due(netpulse_core::store::now())
                }) {
                    continue;
                }
                let result = result.and_then(|parsed| c.store.import(parsed, Some(&s.id)));
                match result {
                    Ok(_) => {
                        let _ = c.store.log(
                            "INFO",
                            "Subscriptions",
                            "subscription-auto",
                            "Refreshed",
                            "Subscription refreshed automatically",
                        );
                        let _ = app.emit("subscriptions-updated", ());
                    }
                    Err(e) => {
                        let _ =
                            c.store
                                .log("WARNING", "Subscriptions", &e.stage, &e.code, &e.message);
                    }
                }
            }
        }
    });
}

#[tauri::command]
pub async fn ui_event(
    state: tauri::State<'_, AppState>,
    action: String,
    target: String,
) -> Result<()> {
    if !["click", "change", "navigation", "error"].contains(&action.as_str())
        || ![
            "button",
            "input",
            "select",
            "textarea",
            "a",
            "dashboard",
            "connections",
            "routing",
            "networkSettings",
            "activity",
            "settings",
            "runtime",
        ]
        .contains(&target.as_str())
    {
        return Err(Error::new(
            "InvalidAuditEvent",
            "ui-audit",
            "Unknown UI action",
        ));
    }
    if action != "error" {
        return Ok(());
    }
    with_core(&state, "ui_event", move |c| {
        c.store.log(
            "ERROR",
            "Interface",
            "runtime",
            "UiRuntimeFailure",
            "Unexpected interface runtime failure",
        )
    })
    .await
}
#[tauri::command]
pub async fn open_log_folder(state: tauri::State<'_, AppState>) -> Result<()> {
    let path = state.data_dir.join("logs");
    std::fs::create_dir_all(&path).map_err(|e| Error::io("audit-folder", e))?;
    open_path(&path)
}
fn open_path(path: &Path) -> Result<()> {
    use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
    let verb = netpulse_core::service::wide("open");
    let path = netpulse_core::service::wide(&path.to_string_lossy());
    if unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            path.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    } as usize
        <= 32
    {
        return Err(Error::new(
            "OpenFailed",
            "open",
            "Windows could not open the requested item",
        ));
    }
    Ok(())
}
#[tauri::command]
pub async fn check_client_update(
    state: tauri::State<'_, AppState>,
    force: Option<bool>,
) -> Result<Option<client_updates::Release>> {
    if state.startup_smoke {
        return Ok(None);
    }
    // Do not hold the connection mutex during a slow/offline GitHub request.
    let updates = state.updates.clone();
    let result =
        tauri::async_runtime::spawn_blocking(move || updates.check(force.unwrap_or(false)))
            .await
            .map_err(|_| {
                Error::new(
                    "ClientUpdateUnavailable",
                    "client-update",
                    "Update worker stopped",
                )
            })?;
    with_core(&state, "check_client_update", move |c| {
        let result = result?;
        c.store.log(
            "INFO",
            "Update",
            "client-check",
            if result.is_some() {
                "Available"
            } else {
                "UpToDate"
            },
            "Client update check completed",
        )?;
        Ok(result)
    })
    .await
}
#[tauri::command]
pub async fn download_client_update(
    state: tauri::State<'_, AppState>,
    release: client_updates::Release,
) -> Result<bool> {
    let directory = state.data_dir.clone();
    with_core(&state, "download_client_update", |c| {
        c.store.log(
            "INFO",
            "Update",
            "client-download",
            "Started",
            "Installer download started",
        )
    })
    .await?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        client_updates::download(&directory, &release)
    })
    .await
    .map_err(|_| {
        Error::new(
            "ClientDownloadFailed",
            "client-update",
            "Download worker stopped",
        )
    })?;
    with_core(&state, "download_client_update_complete", move |c| {
        if let Err(error) = result {
            let _ = c
                .store
                .log("ERROR", "Update", &error.stage, &error.code, &error.message);
            return Err(error);
        }
        c.store.log(
            "INFO",
            "Update",
            "client-download",
            "Verified",
            "Installer downloaded and verified against GitHub SHA-256",
        )?;
        Ok(true)
    })
    .await
}
#[tauri::command]
pub async fn install_client_update(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    release: client_updates::Release,
) -> Result<bool> {
    state.diagnostics.cancel_all();
    let dir = state.data_dir.clone();
    with_core(&state, "install_client_update", move |c| {
        client_updates::revalidate(&release)?;
        let installer = client_updates::verify_download(&dir, &release)?;
        c.disconnect()?;
        c.store.log(
            "INFO",
            "Update",
            "client-install",
            "Launching",
            "Launching verified installer; client is exiting",
        )?;
        open_path(&installer)?;
        Ok(())
    })
    .await?;
    app.exit(0);
    Ok(true)
}
type Session = Option<(u32, String)>;
#[tauri::command]
pub async fn start_mtu(state: tauri::State<'_, AppState>, target: String) -> Result<String> {
    let target = target
        .parse::<std::net::Ipv4Addr>()
        .map_err(|_| Error::new("MtuTarget", "mtu", "Укажите IPv4 адрес, отвечающий на ICMP"))?;
    if target.is_unspecified() || target.is_multicast() || target.is_broadcast() {
        return Err(Error::new("MtuTarget", "mtu", "Нужен адрес одного узла"));
    }
    let (route, profile, prefs, path, tun) = with_core(&state, "mtu-start", |c| {
        let route = c.diagnostic_route();
        let snapshot = c.snapshot();
        let profile = snapshot
            .active
            .as_ref()
            .map(|p| c.store.profile(&p.id))
            .transpose()?;
        Ok((
            route,
            profile,
            c.store.preferences()?,
            c.engine_path.clone(),
            snapshot.tun_active,
        ))
    })
    .await?;
    let jobs = state.diagnostics.clone();
    let core = state.core.clone();
    let (id, cancel) = jobs.begin("mtu", 1)?;
    let returned = id.clone();
    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<()> {
            let mut report = netpulse_core::mtu::analyze(target, tun, &cancel)?;
            if let Some(profile) = profile {
                let latency = diagnostics::latency_samples_on_route(
                    profile,
                    prefs,
                    path,
                    "proxy_https_rtt",
                    &cancel,
                    Some(&route),
                    2,
                );
                report.tunnel_https_ok = Some(latency.successes > 0);
                report.tunnel_note = "HTTPS проверяет доступность текущего VPN. Это не измерение MTU или пропускной способности туннеля.".into();
            } else {
                report.tunnel_note =
                    "VPN не подключён; проверен только сетевой путь Windows.".into();
            }
            if cancel.load(Ordering::SeqCst) {
                return Err(Error::new("Cancelled", "mtu", "Проверка отменена"));
            }
            jobs.append(&id, serde_json::to_value(report).unwrap_or_default());
            if let Ok(c) = core.lock() {
                let _ = c.store.log("INFO", "Diagnostics", "mtu", "Completed", "Adapter, DF path and tunnel health analysis completed; recommendations are estimates");
            }
            Ok(())
        }));
        let error = match result {
            Ok(r) => r.err(),
            Err(_) => Some(Error::new("MtuFailed", "mtu", "Анализ MTU не завершён")),
        };
        jobs.finish(&id, error);
    });
    Ok(returned)
}
fn session(snapshot: &Snapshot) -> Session {
    snapshot
        .engine_pid
        .zip(snapshot.active.as_ref().map(|p| p.id.clone()))
}
fn begin_selection(
    core: Arc<Mutex<NetworkCore>>,
    jobs: Arc<diagnostics::Jobs>,
    samples: u8,
    force_reconnect: bool,
) -> Result<String> {
    let (profiles, prefs, path, expected) = {
        let mut c = core
            .lock()
            .map_err(|_| Error::new("CoreUnavailable", "selection", "Core unavailable"))?;
        let summaries = c.store.profiles()?;
        if summaries.is_empty() || summaries.len() > 2000 {
            return Err(Error::new(
                "NoProfiles",
                "selection",
                "Add 1–2000 servers first",
            ));
        }
        let profiles = summaries
            .iter()
            .map(|p| c.store.profile(&p.id))
            .collect::<Result<Vec<_>>>()?;
        let prefs = c.store.preferences()?;
        let expected = c.guarded(|c| Ok(session(&c.snapshot())))?;
        (profiles, prefs, c.engine_path.clone(), expected)
    };
    let (id, cancel) = jobs.begin("best_server", profiles.len())?;
    let returned = id.clone();
    std::thread::spawn(move || {
        let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<()> {
            let queue = Mutex::new(profiles.into_iter());
            let ranked = Mutex::new(Vec::<(f64, String)>::new());
            std::thread::scope(|scope| {
                for _ in 0..2 {
                    let queue = &queue;
                    let ranked = &ranked;
                    let prefs = &prefs;
                    let path = &path;
                    let cancel = &cancel;
                    let jobs = &jobs;
                    let id = &id;
                    let core = &core;
                    scope.spawn(move || {
                        loop {
                            if cancel.load(Ordering::SeqCst) {
                                break;
                            }
                            let Some(profile) = queue.lock().ok().and_then(|mut q| q.next()) else {
                                break;
                            };
                            let result = diagnostics::latency_samples_on_route(
                                profile,
                                prefs.clone(),
                                path.clone(),
                                "proxy_https_rtt",
                                cancel,
                                None,
                                samples as usize,
                            );
                            if let Some(score) = selection::score(&result, samples as usize)
                                && let Ok(mut values) = ranked.lock()
                            {
                                values.push((score, result.profile_id.clone()));
                            }
                            let value = serde_json::to_value(&result).unwrap_or_default();
                            if let Ok(c) = core.lock() {
                                let _ = c.store.add_measurement("latency", &value);
                                let _ = c.store.log(
                                    "INFO",
                                    "Selection",
                                    "profile-tested",
                                    if result.successes > 0 {
                                        "Reachable"
                                    } else {
                                        "Unreachable"
                                    },
                                    &format!(
                                        "{} of {} authenticated HTTPS requests succeeded",
                                        result.successes, samples
                                    ),
                                );
                            }
                            jobs.append(id, value);
                        }
                    });
                }
            });
            if cancel.load(Ordering::SeqCst) {
                return Ok(());
            }
            let mut ranked = ranked
                .into_inner()
                .map_err(|_| Error::new("SelectionFailed", "selection", "Ranking unavailable"))?;
            ranked.sort_by(|a, b| a.0.total_cmp(&b.0));
            if ranked.is_empty() {
                return Err(Error::new(
                    "NoHealthyServer",
                    "selection",
                    "Ни один сервер не прошёл проверку HTTPS; текущее подключение сохранено",
                ));
            }
            let mut c = core
                .lock()
                .map_err(|_| Error::new("CoreUnavailable", "selection", "Core unavailable"))?;
            if cancel.load(Ordering::SeqCst)
                || c.guarded(|c| Ok(session(&c.snapshot())))? != expected
            {
                return Ok(());
            }
            // Try the next tested candidate if the best fails during activation.
            let mut failure = None;
            for (_, id) in ranked {
                if cancel.load(Ordering::SeqCst) {
                    return Ok(());
                }
                let result = if expected.is_some() {
                    if !force_reconnect
                        && expected.as_ref().is_some_and(|(_, active)| active == &id)
                    {
                        // The already healthy winner does not need a runtime restart.
                        Ok(())
                    } else {
                        c.guarded(|c| c.connect(&id).map(|_| ()))
                    }
                } else {
                    c.select_profile(&id)
                };
                match result {
                    Ok(()) => {
                        c.store.log(
                            "INFO",
                            "Selection",
                            "selected",
                            "BestServer",
                            "Best healthy server selected using median, jitter and packet loss",
                        )?;
                        return Ok(());
                    }
                    Err(e) => failure = Some(e),
                }
            }
            Err(failure.unwrap_or_else(|| {
                Error::new(
                    "SelectionFailed",
                    "selection",
                    "Could not activate a tested server",
                )
            }))
        }));
        let error = match run {
            Ok(r) => r.err(),
            Err(_) => Some(Error::new(
                "SelectionWorkerFailed",
                "selection",
                "Server selection worker stopped",
            )),
        };
        if let Some(e) = &error
            && let Ok(c) = core.lock()
        {
            let _ = c
                .store
                .log("ERROR", "Selection", &e.stage, &e.code, &e.message);
        }
        jobs.finish(&id, error);
    });
    Ok(returned)
}
#[tauri::command]
pub async fn start_best_server(state: tauri::State<'_, AppState>, samples: u8) -> Result<String> {
    if !(5..=15).contains(&samples) {
        return Err(Error::new(
            "SelectionSamples",
            "selection",
            "Use 5–15 samples",
        ));
    }
    // snapshot() may synchronously query the TUN service using its own Tokio
    // runtime. Never execute it on Tauri's async executor (nested runtime panic).
    let core = state.core.clone();
    let jobs = state.diagnostics.clone();
    selection_start_worker(move || begin_selection(core, jobs, samples, false)).await
}
async fn selection_start_worker(
    f: impl FnOnce() -> Result<String> + Send + 'static,
) -> Result<String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|_| {
        Error::new(
            "SelectionWorkerFailed",
            "selection",
            "Server selection could not start",
        )
    })?
}

pub fn selection_monitor(
    app: tauri::AppHandle,
    core: Arc<Mutex<NetworkCore>>,
    jobs: Arc<diagnostics::Jobs>,
) {
    std::thread::spawn(move || {
        let mut failures = 0;
        let mut watched = None;
        loop {
            std::thread::sleep(std::time::Duration::from_secs(10));
            let request = core.try_lock().ok().and_then(|mut c| {
                let prefs = c.store.preferences().ok()?;
                let snapshot = c.snapshot();
                if !prefs.auto_failover
                    || snapshot.engine_pid.is_none()
                    || jobs.active().ok().flatten().is_some()
                {
                    failures = 0;
                    return None;
                }
                let identity = session(&snapshot);
                if watched != identity {
                    failures = 0;
                    watched = identity;
                }
                let profile = c.store.profile(&snapshot.active?.id).ok()?;
                Some((profile, prefs, c.engine_path.clone(), c.diagnostic_route()))
            });
            let Some((profile, prefs, path, route)) = request else {
                continue;
            };
            let result = diagnostics::latency_samples_on_route(
                profile,
                prefs,
                path,
                "proxy_https_rtt",
                &std::sync::atomic::AtomicBool::new(false),
                Some(&route),
                1,
            );
            // A manual switch/disconnect while the request was in flight cancels recovery.
            let still_current = core.lock().is_ok_and(|mut c| {
                session(&c.snapshot()) == watched
                    && c.store.preferences().is_ok_and(|p| p.auto_failover)
            });
            if !still_current {
                failures = 0;
                continue;
            }
            if result.successes > 0 {
                failures = 0;
                continue;
            }
            failures += 1;
            if let Ok(c) = core.lock() {
                let _ = c.store.log(
                    "WARNING",
                    "Selection",
                    "health-check",
                    "NoResponse",
                    &format!("Active session HTTPS health check failed ({failures}/3)"),
                );
            }
            if failures >= 3 {
                failures = 0;
                if begin_selection(core.clone(), jobs.clone(), 5, true).is_ok() {
                    let _ = app.emit("automatic-selection", ());
                }
            }
        }
    });
}

#[cfg(test)]
mod selection_tests {
    use super::*;
    #[test]
    fn startup_and_hidden_webview_share_one_check_and_offline_errors_can_retry() {
        let cache = Arc::new(UpdateCheck::default());
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        std::thread::scope(|scope| {
            for _ in 0..3 {
                let cache = cache.clone();
                let calls = calls.clone();
                scope.spawn(move || {
                    assert!(
                        cache
                            .cached(false, || {
                                calls.fetch_add(1, Ordering::SeqCst);
                                std::thread::sleep(std::time::Duration::from_millis(15));
                                Ok(None)
                            })
                            .unwrap()
                            .is_none()
                    );
                });
            }
        });
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(
            cache
                .cached(true, || Err(Error::new(
                    "Offline",
                    "test",
                    "Fixture offline"
                )))
                .is_err()
        );
        assert!(cache.cached(true, || Ok(None)).is_ok());
    }
    #[test]
    fn async_selection_can_use_the_synchronous_service_runtime() {
        // The IPC client does exactly this: creates and blocks on a private
        // runtime. Running this closure directly in the async command panics.
        let value = tauri::async_runtime::block_on(async {
            selection_start_worker(|| {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                rt.block_on(async { Ok("started".into()) })
            })
            .await
        })
        .unwrap();
        assert_eq!(value, "started");
    }
}
