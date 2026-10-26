//! NetPulse's retained Rust controller, hosted by the native Qt application.
//! Credentials stay in the controller; profile summaries cross the UI boundary.
include!("commands.rs");
include!("dispatch.rs");
use std::io::BufRead;

const MAX_REQUEST: u64 = 12 * 1024 * 1024;
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Request { id: u64, method: String, #[serde(default)] params: serde_json::Value }

fn prepare(directory: &Path, resources: &Path, isolated: bool) -> Result<AppState> {
    std::fs::create_dir_all(directory).map_err(|e| Error::io("data-directory",e))?;
    if !isolated {
        let _ = netpulse_core::proxy_recovery::recover_stale(&directory.join("recovery"));
    }
    let _=netpulse_core::core_releases::cleanup_stages(directory);
    let _=netpulse_core::core_releases::cleanup_downloads(&directory.join("core-downloads"));
    let _=netpulse_core::client_updates::cleanup(directory,env!("CARGO_PKG_VERSION"));
    let store=Store::open(&directory.join("netpulse.db"))?;
    if let Err(e)=netpulse_core::mtu_tuning::recover(&store){let _=store.log("WARNING","Network",&e.stage,&e.code,&e.message);}
    let preferences=store.preferences()?.for_startup();
    store.save_preferences(&preferences)?;
    let core=Arc::new(Mutex::new(NetworkCore::new(store,engine::installed_engine(directory))?));
    let preparing=Arc::new(std::sync::atomic::AtomicBool::new(true));
    let flag=preparing.clone(); let target=directory.to_path_buf(); let bundle=resources.to_path_buf();
    std::thread::spawn(move || {
        let result=netpulse_core::core_updates::ensure_bundled(&bundle,&target);
        flag.store(false,std::sync::atomic::Ordering::Release);
        let _=host::AppHandle.emit("runtime-prepared",result.err());
    });
    let state=AppState {
        core, data_dir:directory.to_path_buf(), diagnostics:Arc::new(netpulse_core::diagnostics::Jobs::default()),
        runtime_preparing:preparing,startup_smoke:isolated,
    };
    if let Ok(mut c)=state.core.lock() {
        c.observe(|state| { let _=host::AppHandle.emit("connection-state",state); });
    }
    if !isolated {
        actions::subscription_monitor(host::AppHandle,state.core.clone());
        actions::selection_monitor(host::AppHandle,state.core.clone(),state.diagnostics.clone());
        let core=state.core.clone();let preparing=state.runtime_preparing.clone();
        std::thread::spawn(move || {
            while preparing.load(std::sync::atomic::Ordering::Acquire) {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            if let Ok(mut c)=core.lock() {
                let _=c.guarded(|c| {
                    let p=c.store.preferences()?;
                    if p.auto_reconnect && (p.tun || p.system_proxy) && c.snapshot().engine_pid.is_none()
                        && let Some(id)=p.selected_profile { c.connect(&id)?; }
                    Ok(())
                });
            }
        });
    }
    // Statistics are observed separately, as in the original release. Polling a
    // snapshot must never restart, close, or otherwise alter live connections.
    let monitor=state.core.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(5));
        if let Ok(mut c)=monitor.try_lock() { let _=c.guarded(|c|c.refresh_metrics()); }
    });
    Ok(state)
}

fn run() -> Result<()> {
    let args=std::env::args_os().collect::<Vec<_>>();
    if args.get(1).is_some_and(|v|v=="--installer-service-install") {
        let executable=std::env::current_exe().map_err(|e|Error::io("installer",e))?;
        let root=executable.parent().unwrap();
        let installed=netpulse_core::service::installation_status()?.installed;
        return netpulse_core::service::elevated_install(&root.join("NetPulseService.exe"),&root.join("cores"),true,installed);
    }
    if args.get(1).is_some_and(|v|v=="--installer-service-remove") {
        if netpulse_core::service::installation_status()?.installed {
            let executable=std::env::current_exe().map_err(|e|Error::io("installer",e))?;
            let root=executable.parent().unwrap();
            netpulse_core::service::elevated_install(&root.join("NetPulseService.exe"),&root.join("cores"),false,false)?;
        }
        return Ok(());
    }
    if args.get(1).is_some_and(|v|v=="--proxy-recover") {
        return netpulse_core::proxy_recovery::run(Path::new(args.get(2).ok_or_else(||Error::new("Argument","recovery","Missing journal"))?));
    }
    if !args.get(1).is_some_and(|v|v=="--stdio") {
        return Err(Error::new("PrivateHost","startup","Start NetPulse.exe to use this application"));
    }
    let directory=args.get(2).map(PathBuf::from).filter(|p|p.is_absolute())
        .ok_or_else(||Error::new("DataDirectory","startup","Absolute data directory required"))?;
    let resources=std::env::current_exe().map_err(|e|Error::io("startup",e))?.parent().unwrap().to_path_buf();
    let isolated=args.iter().any(|v|v=="--isolated");
    let state=Arc::new(prepare(&directory,&resources,isolated)?);
    let runtime=tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build()
        .map_err(|e|Error::io("async-runtime",e))?;
    let semaphore=Arc::new(tokio::sync::Semaphore::new(8));
    let mut stdin=std::io::stdin().lock();
    loop {
        let mut line=Vec::new();
        let count=(&mut stdin).take(MAX_REQUEST+1).read_until(b'\n',&mut line).map_err(|e|Error::io("ipc-read",e))?;
        if count==0 { break; }
        if count as u64>MAX_REQUEST {
            return Err(Error::new("FrameLimit","ipc","Request exceeds size limit"));
        }
        let request=match serde_json::from_slice::<Request>(&line) {
            Ok(v)=>v,
            Err(_)=> { let _=host::send(&serde_json::json!({"id":null,"error":{"code":"InvalidRequest","stage":"ipc","message":"Invalid request"}}));continue; }
        };
        if request.method=="quit_application" {
            let result=runtime.block_on(dispatch(&state,&request.method,request.params));
            let ready=result.is_ok();
            respond(request.id,result);
            if ready { break; } else { continue; }
        }
        let permit=runtime.block_on(semaphore.clone().acquire_owned())
            .map_err(|_|Error::new("HostClosing","ipc","Controller is closing"))?;
        let state=state.clone();
        runtime.spawn(async move {
            let _permit=permit;
            let result=dispatch(&state,&request.method,request.params).await;
            respond(request.id,result);
        });
    }
    state.diagnostics.cancel_all();
    if let Ok(mut c)=state.core.lock() { c.disconnect()?; }
    runtime.shutdown_timeout(std::time::Duration::from_secs(10));
    Ok(())
}
fn respond(id:u64,result:Result<serde_json::Value>) {
    let value=match result {
        Ok(result)=>serde_json::json!({"id":id,"result":result}),
        Err(error)=>serde_json::json!({"id":id,"error":error}),
    };
    let _=host::send(&value);
}
fn main() {
    // Panic details can contain private data. Return a fixed message to stderr.
    std::panic::set_hook(Box::new(|_|eprintln!("NetPulse controller operation failed")));
    if let Err(error)=run() {
        let installer=std::env::args_os().nth(1).is_some_and(|a|a=="--installer-service-install"||a=="--installer-service-remove");
        let exit_code=if installer {netpulse_core::service::maintenance_exit_code(&error)} else {1};
        let _=host::send(&serde_json::json!({"event":"host-error","data":error}));
        std::process::exit(exit_code);
    }
}
