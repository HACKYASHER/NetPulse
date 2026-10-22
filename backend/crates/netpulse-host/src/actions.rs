use super::*;
use netpulse_core::{diagnostics, selection};
use std::sync::atomic::Ordering;

pub fn subscription_monitor(app: host::AppHandle, core: Arc<Mutex<NetworkCore>>) {
    std::thread::spawn(move || {
        let mut attempts = std::collections::HashMap::<String, std::time::Instant>::new();
        let mut first = true;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(if first { 100 } else { 60_000 }));
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
                        (first || s.due(netpulse_core::store::now()))
                            && attempts
                                .get(&s.id)
                                .is_none_or(|at| at.elapsed().as_secs() >= 15 * 60)
                    })
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
                    v.updated_at == s.updated_at && (first || v.due(netpulse_core::store::now()))
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
            first = false;
        }
    });
}

pub async fn ui_event(
    state: host::State<'_, AppState>,
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
pub async fn open_log_folder(state: host::State<'_, AppState>) -> Result<()> {
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



type Session = Option<(u32, String)>;
pub async fn start_mtu(state: host::State<'_, AppState>, target: String) -> Result<String> {
    let explicit_target=if target.trim().is_empty(){None}else{Some(target.trim().parse::<std::net::Ipv4Addr>().map_err(|_|Error::new("MtuTarget","mtu","Неверный IPv4 адрес"))?)};
    let (route, profile, prefs, path, tun, server) = with_core(&state, "mtu-start", |c| {
        let route = c.diagnostic_route();
        let snapshot = c.snapshot();
        let profile = snapshot
            .active
            .as_ref()
            .map(|p| c.store.profile(&p.id))
            .transpose()?;
        let prefs=c.store.preferences()?;
        let server=snapshot.active.as_ref().map(|p|p.host.clone()).or_else(||c.store.profiles().ok()?.into_iter().find(|p|Some(&p.id)==prefs.selected_profile.as_ref()).map(|p|p.host));
        Ok((
            route,
            profile,
            prefs,
            c.engine_path.clone(),
            snapshot.tun_active,
            server,
        ))
    })
    .await?;
    let jobs = state.diagnostics.clone();
    let core = state.core.clone();
    let (id, cancel) = jobs.begin("mtu", 1)?;
    let returned = id.clone();
    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<()> {
            let (target,target_source)=explicit_target.map(|ip|(ip,"explicit")).unwrap_or_else(||netpulse_core::mtu::target(server.as_deref()));
            if target.is_unspecified()||target.is_multicast()||target.is_broadcast(){return Err(Error::new("MtuTarget","mtu","Нужен адрес одного узла"));}
            let mut report = netpulse_core::mtu::analyze(target, tun, &cancel)?;
            if let Some(profile) = profile {
                let latency = diagnostics::latency_samples_on_route(
                    profile,
                    prefs.clone(),
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
            let mut value = serde_json::to_value(report).unwrap_or_default();
            value["timestamp"]=serde_json::json!(netpulse_core::store::now());
            value["target_source"]=serde_json::json!(target_source);
            value["max_icmp_payload"]=serde_json::json!(value["path"]["confirmed_mtu"].as_u64().map(|n|n.saturating_sub(28)));
            value["current_mtu"]=serde_json::json!(prefs.mtu);
            jobs.append(&id, value.clone());
            if let Ok(c) = core.lock() { let _ = c.store.add_measurement("mtu", &value); }
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
pub async fn start_best_server(state: host::State<'_, AppState>, samples: u8) -> Result<String> {
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
    host::async_runtime::spawn_blocking(f).await.map_err(|_| {
        Error::new(
            "SelectionWorkerFailed",
            "selection",
            "Server selection could not start",
        )
    })?
}

pub fn selection_monitor(
    app: host::AppHandle,
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



pub async fn start_mtu_tuning(state:host::State<'_,AppState>,request:netpulse_core::mtu_tuning::Request)->Result<String> {
    use netpulse_core::mtu_tuning::{Trial, Probe, compare, confirm};
    request.validate()?;
    let (original,mut expected_session)=with_core(&state,"mtu-tuning",|c|{
        let snapshot=c.snapshot();
        if !snapshot.tun_active||snapshot.engine_pid.is_none(){return Err(Error::new("TunRequired","mtu-tuning","Для сравнения MTU подключитесь в режиме TUN"))}
        Ok((c.store.preferences()?,session(&snapshot)))
    }).await?;
    let values=request.candidates();
    let jobs=state.diagnostics.clone();let core=state.core.clone();
    let (id,cancel)=jobs.begin("mtu_tuning",values.len()+4)?;let returned=id.clone();
    std::thread::spawn(move || {
        let mut runtime_mtu=original.mtu;
        let mut report=None;
        let work=std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<()> {
            let probe=Probe::new(&request)?;
            // Runtime candidates are never persisted. This record also handles
            // migration from the older implementation that saved each candidate.
            {let c=core.lock().map_err(|_|Error::new("CoreUnavailable","mtu-tuning","Core unavailable"))?;
                netpulse_core::mtu_tuning::save_recovery(c.store.data_directory(),&original,&original)?;}
            let mut measure=|mtu:u16,phase:&str|->Result<Trial>{
                {
                    let mut c=core.lock().map_err(|_|Error::new("CoreUnavailable","mtu-tuning","Core unavailable"))?;
                    if cancel.load(Ordering::SeqCst){return Err(Error::new("Cancelled","mtu-tuning","Тест отменён"))}
                    if session(&c.snapshot())!=expected_session||c.store.preferences()?!=original {
                        return Err(Error::new("ConnectionChanged","mtu-tuning","Подключение или настройки изменились; тест остановлен"))
                    }
                    if runtime_mtu!=mtu {
                        // connect_with can replace the process even when a later
                        // preflight fails, so capture the resulting owned session.
                        let changed=c.guarded(|c|c.set_session_mtu(mtu));
                        let snapshot=c.snapshot();expected_session=session(&snapshot);
                        runtime_mtu=snapshot.applied_preferences.as_ref().map_or(original.mtu,|p|p.mtu);
                        changed?;
                    }
                }
                let mut samples=Vec::new();let mut last_error=None;
                for _ in 0..request.samples {
                    if cancel.load(Ordering::SeqCst){return Err(Error::new("Cancelled","mtu-tuning","Тест отменён"))}
                    let measured=probe.sample(&cancel);
                    {
                        let mut c=core.lock().map_err(|_|Error::new("CoreUnavailable","mtu-tuning","Core unavailable"))?;
                        if session(&c.snapshot())!=expected_session||c.store.preferences()?!=original {
                            return Err(Error::new("ConnectionChanged","mtu-tuning","Подключение или настройки изменились; тест остановлен"))
                        }
                    }
                    match measured {
                        Ok(ms)=>samples.push(ms),
                        Err(e) if ["Cancelled","MtuRouteChanged","PlatformUnsupported"].contains(&e.code.as_str())=>return Err(e),
                        Err(e)=>last_error=Some(e.message),
                    }
                }
                let trial=Trial::from_samples(mtu,phase,samples,request.samples as usize,last_error);
                jobs.append(&id,serde_json::to_value(&trial).unwrap_or_default());Ok(trial)
            };
            // Fail before changing the MTU if this route/protocol cannot carry
            // the requested probe (notably ICMP over VLESS/Hysteria2/SOCKS).
            let baseline=measure(original.mtu,"baseline")?;
            if baseline.successes==0 {
                return Err(Error::new("MtuProbeUnavailable","mtu-tuning",baseline.error.clone().unwrap_or_else(||"Тестовая цель не ответила. Проверьте URL; для VPN выберите HTTPS.".into())));
            }
            let mut trials=Vec::new();
            for mtu in values {trials.push(measure(mtu,"candidate")?)}
            let provisional=compare(&request,&trials,&baseline,&baseline);
            let verify=measure(provisional.suggested_mtu.unwrap_or(original.mtu),"verify")?;
            let control=measure(original.mtu,"control")?;
            let comparison=confirm(compare(&request,&trials,&baseline,&control),&request,&trials,&baseline,&control,&verify);
            report=Some(serde_json::json!({"kind":"comparison","timestamp":netpulse_core::store::now(),"comparison":comparison,"baseline":baseline,"trials":trials,"control":control,"verification":verify,"resolved_target":probe.address().to_string(),"route":"current_tun","restored":false}));
            Ok(())
        }));
        let mut error=match work {Ok(result)=>result.err(),Err(_)=>Some(Error::new("MtuTuningFailed","mtu-tuning","Тест прерван"))};
        let mut restored=false;
        if let Ok(mut c)=core.lock() {
            let snapshot=c.snapshot();
            if let Ok(saved)=c.store.preferences() {
                // Keep user's new profile, routing settings, and manual MTU. Only
                // restore a runtime that still belongs to this diagnostic.
                let owned=session(&snapshot)==expected_session;
                if owned && snapshot.applied_preferences.as_ref().is_some_and(|p|p.mtu!=saved.mtu) {
                    if let Err(e)=c.guarded(|c|c.apply_preferences(saved.clone())) {error=Some(e)} else {restored=true;}
                } else {
                    restored=snapshot.engine_pid.is_none() || snapshot.applied_preferences.as_ref().is_some_and(|p|p.mtu==saved.mtu);
                }
                if restored {
                    let _=std::fs::remove_file(c.store.data_directory().join("mtu-tuning-recovery.json"));
                }
            }
            if let Some(mut value)=report {
                value["restored"]=serde_json::json!(restored);
                // A failed restoration cannot expose an actionable suggestion.
                if !restored {value["comparison"]["suggested_mtu"]=serde_json::Value::Null;}
                jobs.append(&id,value.clone());let _=c.store.add_measurement("mtu_tuning",&value);
            }
        }
        let _=host::AppHandle.emit("mtu-tuning-finished",());jobs.finish(&id,error);
    });
    Ok(returned)
}