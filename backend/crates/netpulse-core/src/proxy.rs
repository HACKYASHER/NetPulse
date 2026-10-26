//! Current-user WinINET proxy lease. Compare before restoring another client's state.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ProxyState {
    pub flags: u32,
    pub server: String,
    pub bypass: String,
    pub pac: String,
}
fn loopback_state(port: u16) -> ProxyState {
    ProxyState {
        flags: 3,
        // HTTP and HTTPS CONNECT use the same listener. A single endpoint
        // also lets Windows Settings separate the address and port.
        server: format!("127.0.0.1:{port}"),
        bypass: "<local>".into(),
        pac: String::new(),
    }
}
fn check_available(state: &ProxyState) -> Result<()> {
    if state.flags & 6 == 0 { return Ok(()); }
    let message=if state.flags & 4 != 0 {
        "В Windows включён сценарий автоматической настройки прокси (PAC). NetPulse не заменяет его автоматически. Выберите, какой клиент должен управлять системным прокси, в параметрах прокси Windows."
    } else {
        "В Windows уже включён системный прокси. NetPulse не заменяет его автоматически. Если хотите использовать System Proxy в NetPulse, отключите этот режим в другом VPN-клиенте."
    };
    // Do not log a foreign PAC URL or proxy string: either can contain credentials.
    let endpoint=state.server.parse::<std::net::SocketAddr>().ok().filter(|s|s.ip().is_loopback());
    Err(Error::new("ProxyConflict","proxy-acquire",if let Some(endpoint)=endpoint {format!("{message} Текущий адрес: {endpoint}.")}else{message.into()}))
}
pub struct ProxyLease {
    pub before: ProxyState,
    pub applied: ProxyState,
    active: bool,
    #[cfg(windows)]
    recovery: Option<crate::proxy_recovery::Recovery>,
}
#[cfg(windows)]
pub fn read() -> Result<ProxyState> {
    use windows_sys::Win32::{Foundation::GlobalFree, Networking::WinInet::*};
    let mut opts = [
        INTERNET_PER_CONN_OPTIONW {
            dwOption: INTERNET_PER_CONN_FLAGS,
            ..Default::default()
        },
        INTERNET_PER_CONN_OPTIONW {
            dwOption: INTERNET_PER_CONN_PROXY_SERVER,
            ..Default::default()
        },
        INTERNET_PER_CONN_OPTIONW {
            dwOption: INTERNET_PER_CONN_PROXY_BYPASS,
            ..Default::default()
        },
        INTERNET_PER_CONN_OPTIONW {
            dwOption: INTERNET_PER_CONN_AUTOCONFIG_URL,
            ..Default::default()
        },
    ];
    let mut list = INTERNET_PER_CONN_OPTION_LISTW {
        dwSize: std::mem::size_of::<INTERNET_PER_CONN_OPTION_LISTW>() as u32,
        dwOptionCount: opts.len() as u32,
        pOptions: opts.as_mut_ptr(),
        ..Default::default()
    };
    let mut size = std::mem::size_of_val(&list) as u32;
    if unsafe {
        InternetQueryOptionW(
            std::ptr::null(),
            INTERNET_OPTION_PER_CONNECTION_OPTION,
            &mut list as *mut _ as _,
            &mut size,
        )
    } == 0
    {
        return Err(Error::io("proxy-read", std::io::Error::last_os_error()));
    }
    let flags = unsafe { opts[0].Value.dwValue };
    let mut strings = vec![];
    for option in &opts[1..] {
        let ptr = unsafe { option.Value.pszValue };
        let s = if ptr.is_null() {
            String::new()
        } else {
            let mut n = 0;
            unsafe {
                while *ptr.add(n) != 0 && n < 65536 {
                    n += 1;
                }
                let s = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, n));
                GlobalFree(ptr as _);
                s
            }
        };
        strings.push(s);
    }
    Ok(ProxyState {
        flags,
        server: strings[0].clone(),
        bypass: strings[1].clone(),
        pac: strings[2].clone(),
    })
}
#[cfg(windows)]
pub(crate) fn apply(state: &ProxyState) -> Result<()> {
    use windows_sys::Win32::Networking::WinInet::*;
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let mut server = wide(&state.server);
    let mut bypass = wide(&state.bypass);
    let mut pac = wide(&state.pac);
    let mut opts = [
        INTERNET_PER_CONN_OPTIONW {
            dwOption: INTERNET_PER_CONN_FLAGS,
            Value: INTERNET_PER_CONN_OPTIONW_0 {
                dwValue: state.flags,
            },
        },
        INTERNET_PER_CONN_OPTIONW {
            dwOption: INTERNET_PER_CONN_PROXY_SERVER,
            Value: INTERNET_PER_CONN_OPTIONW_0 {
                pszValue: server.as_mut_ptr(),
            },
        },
        INTERNET_PER_CONN_OPTIONW {
            dwOption: INTERNET_PER_CONN_PROXY_BYPASS,
            Value: INTERNET_PER_CONN_OPTIONW_0 {
                pszValue: bypass.as_mut_ptr(),
            },
        },
        INTERNET_PER_CONN_OPTIONW {
            dwOption: INTERNET_PER_CONN_AUTOCONFIG_URL,
            Value: INTERNET_PER_CONN_OPTIONW_0 {
                pszValue: pac.as_mut_ptr(),
            },
        },
    ];
    let list = INTERNET_PER_CONN_OPTION_LISTW {
        dwSize: std::mem::size_of::<INTERNET_PER_CONN_OPTION_LISTW>() as u32,
        dwOptionCount: opts.len() as u32,
        pOptions: opts.as_mut_ptr(),
        ..Default::default()
    };
    let ok = unsafe {
        InternetSetOptionW(
            std::ptr::null(),
            INTERNET_OPTION_PER_CONNECTION_OPTION,
            &list as *const _ as _,
            std::mem::size_of_val(&list) as u32,
        )
    };
    if ok == 0 {
        return Err(Error::io("proxy-apply", std::io::Error::last_os_error()));
    }
    for option in [INTERNET_OPTION_SETTINGS_CHANGED, INTERNET_OPTION_REFRESH] {
        if unsafe { InternetSetOptionW(std::ptr::null(), option, std::ptr::null(), 0) } == 0 {
            return Err(Error::io("proxy-notify", std::io::Error::last_os_error()));
        }
    }
    if read()? != *state {
        return Err(Error::new(
            "ProxyReadbackMismatch",
            "proxy-verify",
            "Windows proxy readback differs from requested state",
        ));
    }
    Ok(())
}
#[cfg(not(windows))]
pub fn read() -> Result<ProxyState> {
    Err(Error::new(
        "PlatformUnsupported",
        "proxy-read",
        "Windows WinINET is required",
    ))
}
#[cfg(not(windows))]
fn apply(_state: &ProxyState) -> Result<()> {
    Err(Error::new(
        "PlatformUnsupported",
        "proxy-apply",
        "Windows WinINET is required",
    ))
}
impl ProxyLease {
    pub fn is_applied(&self) -> Result<bool> {
        Ok(self.active && read()? == self.applied)
    }
    pub fn check_conflict() -> Result<()> {
        check_available(&read()?)
    }
    pub fn acquire(port: u16, directory: &std::path::Path) -> Result<Self> {
        let before = read()?;
        check_available(&before)?;
        let applied = loopback_state(port);
        let mut lease = Self {
            #[cfg(windows)]
            recovery: Some(crate::proxy_recovery::Recovery::arm(
                directory, &before, &applied,
            )?),
            before,
            applied,
            active: true,
        };
        if let Err(e) = apply(&lease.applied) {
            // Restore only if Windows still contains precisely our applied state.
            if read().is_ok_and(|s| s == lease.applied) {
                let _ = apply(&lease.before);
            }
            let _ = lease.restore();
            return Err(e);
        }
        Ok(lease)
    }
    pub fn restore(&mut self) -> Result<()> {
        if !self.active {
            return Ok(());
        }
        if read()? != self.applied {
            self.active = false;
            #[cfg(windows)]
            if let Some(recovery) = &mut self.recovery {
                recovery.finish()?;
            }
            return Err(Error::new(
                "ProxyOwnershipLost",
                "proxy-restore",
                "Another application changed the proxy; its settings were preserved",
            ));
        }
        apply(&self.before)?;
        self.active = false;
        #[cfg(windows)]
        if let Some(recovery) = &mut self.recovery {
            recovery.finish()?;
        }
        Ok(())
    }
    pub fn recovery_pid(&self) -> Option<u32> {
        #[cfg(windows)]
        {
            self.recovery
                .as_ref()
                .map(crate::proxy_recovery::Recovery::pid)
        }
        #[cfg(not(windows))]
        {
            None
        }
    }
    /// Ask the independent recovery process to restore now, with the same
    /// compare-before-restore policy as crash recovery, while the GUI stays open.
    pub fn recover_now(&mut self) -> Result<()> {
        #[cfg(windows)]
        if let Some(recovery) = &mut self.recovery {
            recovery.finish()?;
        }
        if read()? == self.before {
            self.active = false;
            return Ok(());
        }
        self.restore()
    }
}
impl Drop for ProxyLease {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn foreign_proxy_conflict_is_explained_without_disclosing_credentials() {
        let mut state=loopback_state(2080);
        let error=check_available(&state).unwrap_err();
        assert_eq!(error.code,"ProxyConflict");assert!(error.message.contains("127.0.0.1:2080"));
        state.server="http=private:secret@proxy.example:8080".into();
        assert!(!check_available(&state).unwrap_err().message.contains("secret"));
        state.flags=5;state.pac="https://private.invalid/private-token".into();
        let error=check_available(&state).unwrap_err();assert!(error.message.contains("PAC"));assert!(!error.message.contains("private-token"));
        for flags in [1,9] {state.flags=flags;check_available(&state).unwrap();}
    }
    #[test]
    fn read_only_windows_state() {
        let s = read().unwrap();
        assert!(s.server.len() < 65536);
    }
    #[test]
    fn shared_endpoint_routes_http_and_https_in_isolated_wininet_session() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            time::{Duration, Instant},
        };
        use windows_sys::Win32::Networking::WinInet::*;
        let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        // Per-session settings do not change the user's system proxy. The
        // HTTPS fixture rejects CONNECT before TLS; no certificate bypass.
        for (url, request_line, response) in [
            (
                "http://netpulse-fixture.invalid/check",
                "GET http://netpulse-fixture.invalid/check",
                "HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n",
            ),
            (
                "https://netpulse-fixture.invalid/check",
                "CONNECT netpulse-fixture.invalid:443",
                "HTTP/1.1 502 Fixture Stops Before TLS\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            ),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let state = loopback_state(listener.local_addr().unwrap().port());
            listener.set_nonblocking(true).unwrap();
            let worker = std::thread::spawn(move || {
                let deadline = Instant::now() + Duration::from_secs(5);
                let (mut stream, _) = loop {
                    match listener.accept() {
                        Ok(value) => break value,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(Instant::now() < deadline, "WinINET did not use the proxy");
                            std::thread::sleep(Duration::from_millis(10));
                        }
                        Err(error) => panic!("Fixture accept: {error}"),
                    }
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut headers = Vec::new();
                let mut byte = [0];
                while !headers.ends_with(b"\r\n\r\n") {
                    assert!(headers.len() < 16384);
                    stream.read_exact(&mut byte).unwrap();
                    headers.push(byte[0]);
                }
                stream.write_all(response.as_bytes()).unwrap();
                String::from_utf8(headers).unwrap()
            });
            unsafe {
                let session = InternetOpenW(
                    wide("NetPulseProxyFixture").as_ptr(),
                    INTERNET_OPEN_TYPE_PROXY,
                    wide(&state.server).as_ptr(),
                    wide(&state.bypass).as_ptr(),
                    0,
                );
                assert!(!session.is_null());
                let timeout: u32 = 1500;
                for option in [
                    INTERNET_OPTION_CONNECT_TIMEOUT,
                    INTERNET_OPTION_SEND_TIMEOUT,
                    INTERNET_OPTION_RECEIVE_TIMEOUT,
                ] {
                    assert_ne!(
                        InternetSetOptionW(
                            session,
                            option,
                            &timeout as *const _ as _,
                            std::mem::size_of_val(&timeout) as u32
                        ),
                        0
                    );
                }
                let request = InternetOpenUrlW(
                    session,
                    wide(url).as_ptr(),
                    std::ptr::null(),
                    0,
                    INTERNET_FLAG_NO_CACHE_WRITE
                        | INTERNET_FLAG_RELOAD
                        | INTERNET_FLAG_NO_AUTO_REDIRECT,
                    0,
                );
                if !request.is_null() {
                    InternetCloseHandle(request);
                }
                InternetCloseHandle(session);
            }
            let headers = worker.join().unwrap();
            let (target, version) = headers.lines().next().unwrap().rsplit_once(' ').unwrap();
            assert_eq!(target, request_line);
            assert!(matches!(version, "HTTP/1.0" | "HTTP/1.1"));
        }
    }
}
