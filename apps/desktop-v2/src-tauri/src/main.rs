#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod self_test;
fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|a| a == "--installer-verify") {
        let verified = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("cores")))
            .is_some_and(|root| {
                netpulse_core::engine::verify_engine(&root.join("sing-box.exe"))
                    .and_then(|()| netpulse_core::runtime::verify_xray(&root.join("xray.exe")))
                    .is_ok()
            });
        std::process::exit(if verified { 0 } else { 1 });
    }
    #[cfg(windows)]
    if args
        .get(1)
        .is_some_and(|a| a == "--installer-service" || a == "--installer-service-remove")
    {
        let result = (|| {
            let root = std::env::current_exe()
                .map_err(|e| netpulse_core::Error::io("installer-service", e))?
                .parent()
                .unwrap()
                .to_path_buf();
            let install = args[1] == "--installer-service";
            let installed = netpulse_core::service::installation_status()?.installed;
            if !install && !installed {
                return Ok(());
            }
            let upgrade = install && installed;
            netpulse_core::service::elevated_install(
                &root.join("NetPulseService.exe"),
                &root.join("cores"),
                install,
                upgrade,
            )
        })();
        std::process::exit(if result.is_ok() { 0 } else { 1 });
    }
    #[cfg(windows)]
    if args.get(1).is_some_and(|a| a == "--proxy-recover") {
        let Some(path) = args.get(2) else {
            std::process::exit(2)
        };
        std::process::exit(
            if netpulse_core::proxy_recovery::run(std::path::Path::new(path)).is_ok() {
                0
            } else {
                1
            },
        );
    }
    if args.get(1).is_some_and(|a| a == "--self-test") {
        let Some(directory) = args.get(2) else {
            std::process::exit(2)
        };
        if let Err(e) = self_test::run(std::path::Path::new(directory)) {
            eprintln!("Packaging check failed: {e}");
            std::process::exit(1);
        }
        return;
    }
    netpulse_desktop_v2::run()
}
