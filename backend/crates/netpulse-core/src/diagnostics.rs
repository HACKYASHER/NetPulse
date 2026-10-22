//! Explicit, cancellable diagnostics. Live counters are never speed-test results.
use crate::{
    Error, Result,
    model::{Preferences, Profile},
    runtime::{RuntimePlan, free_port},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::Read,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
#[derive(Clone, Serialize)]
pub struct JobStatus {
    pub id: String,
    pub kind: String,
    pub running: bool,
    pub cancelled: bool,
    pub completed: usize,
    pub total: usize,
    pub results: Vec<serde_json::Value>,
    pub error: Option<Error>,
    pub progress: Option<SpeedSample>,
}
struct Job {
    status: JobStatus,
    cancel: Arc<AtomicBool>,
}
#[derive(Default)]
pub struct Jobs {
    jobs: Mutex<HashMap<String, Job>>,
}
impl Jobs {
    pub fn begin(&self, kind: &str, total: usize) -> Result<(String, Arc<AtomicBool>)> {
        let mut jobs = self.jobs.lock().map_err(|_| {
            Error::new(
                "DiagnosticsUnavailable",
                "diagnostics-start",
                "Diagnostic controller unavailable",
            )
        })?;
        if jobs.values().any(|j| j.status.running) {
            return Err(Error::new(
                "DiagnosticInProgress",
                "diagnostics-start",
                "Cancel or finish the current diagnostic first",
            ));
        }
        if jobs.len() > 10 {
            jobs.clear();
        }
        let id = uuid::Uuid::new_v4().to_string();
        let cancel = Arc::new(AtomicBool::new(false));
        jobs.insert(
            id.clone(),
            Job {
                status: JobStatus {
                    id: id.clone(),
                    kind: kind.into(),
                    running: true,
                    cancelled: false,
                    completed: 0,
                    total,
                    results: vec![],
                    error: None,
                    progress: None,
                },
                cancel: cancel.clone(),
            },
        );
        Ok((id, cancel))
    }
    pub fn active(&self) -> Result<Option<JobStatus>> {
        self.jobs
            .lock()
            .map(|jobs| {
                jobs.values()
                    .find(|j| j.status.running)
                    .map(|j| j.status.clone())
            })
            .map_err(|_| {
                Error::new(
                    "DiagnosticsUnavailable",
                    "diagnostics-status",
                    "Diagnostic controller unavailable",
                )
            })
    }
    pub fn status(&self, id: &str) -> Result<JobStatus> {
        self.jobs
            .lock()
            .map_err(|_| {
                Error::new(
                    "DiagnosticsUnavailable",
                    "diagnostics-status",
                    "Diagnostic controller unavailable",
                )
            })?
            .get(id)
            .map(|j| j.status.clone())
            .ok_or_else(|| {
                Error::new(
                    "DiagnosticNotFound",
                    "diagnostics-status",
                    "Diagnostic job not found",
                )
            })
    }
    pub fn cancel(&self, id: &str) -> Result<()> {
        let jobs = self.jobs.lock().map_err(|_| {
            Error::new(
                "DiagnosticsUnavailable",
                "diagnostics-cancel",
                "Diagnostic controller unavailable",
            )
        })?;
        let j = jobs.get(id).ok_or_else(|| {
            Error::new(
                "DiagnosticNotFound",
                "diagnostics-cancel",
                "Diagnostic job not found",
            )
        })?;
        j.cancel.store(true, Ordering::SeqCst);
        Ok(())
    }
    pub fn cancel_all(&self) {
        if let Ok(jobs) = self.jobs.lock() {
            for j in jobs.values() {
                j.cancel.store(true, Ordering::SeqCst);
            }
        }
    }
    pub fn append(&self, id: &str, result: serde_json::Value) {
        if let Ok(mut jobs) = self.jobs.lock()
            && let Some(j) = jobs.get_mut(id)
        {
            j.status.results.push(result);
            j.status.completed += 1;
        }
    }
    pub fn progress(&self, id: &str, sample: &SpeedSample) {
        if let Ok(mut jobs) = self.jobs.lock()
            && let Some(j) = jobs.get_mut(id)
        {
            j.status.progress = Some(sample.clone());
        }
    }
    pub fn finish(&self, id: &str, error: Option<Error>) {
        if let Ok(mut jobs) = self.jobs.lock()
            && let Some(j) = jobs.get_mut(id)
        {
            j.status.running = false;
            j.status.cancelled = j.cancel.load(Ordering::SeqCst);
            j.status.error = error;
        }
    }
}
fn cancelled(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::SeqCst) {
        Err(Error::new(
            "Cancelled",
            "diagnostics",
            "Diagnostic cancelled by user",
        ))
    } else {
        Ok(())
    }
}
fn endpoint(url: &str) -> Result<url::Url> {
    let u = url::Url::parse(url).map_err(|_| {
        Error::new(
            "InvalidEndpoint",
            "diagnostics-validate",
            "Invalid HTTPS endpoint",
        )
    })?;
    if u.scheme() != "https"
        || u.host_str().is_none()
        || !u.username().is_empty()
        || u.password().is_some()
        || u.fragment().is_some()
    {
        return Err(Error::new(
            "InvalidEndpoint",
            "diagnostics-validate",
            "Use HTTPS without credentials or fragments",
        ));
    }
    Ok(u)
}
fn http(port: Option<u16>, secret: &str, timeout: Duration) -> Result<reqwest::blocking::Client> {
    let mut builder = reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::none());
    #[cfg(test)]
    {
        builder = builder.add_root_certificate(
            reqwest::Certificate::from_pem(include_bytes!("../tests/fixtures/owned-ca.pem"))
                .unwrap(),
        );
    }
    if let Some(port) = port {
        builder = builder.proxy(
            reqwest::Proxy::all(format!("http://127.0.0.1:{port}"))
                .map_err(|_| Error::new("ProxyError", "diagnostics", "Invalid diagnostic proxy"))?
                .basic_auth("netpulse", secret),
        );
    }
    builder.build().map_err(|_| {
        Error::new(
            "TlsError",
            "diagnostics",
            "Could not initialize verified TLS client",
        )
    })
}
#[derive(Serialize, Clone, Debug)]
pub struct LatencyResult {
    pub profile_id: String,
    pub method: String,
    pub protocol: String,
    pub timestamp: u64,
    pub destination: String,
    pub min_ms: Option<f64>,
    pub avg_ms: Option<f64>,
    pub max_ms: Option<f64>,
    pub successes: usize,
    pub timeouts: usize,
    pub failures: usize,
    pub samples: Vec<f64>,
    pub error: Option<Error>,
    pub note: String,
}
pub fn latency(
    profile: Profile,
    prefs: Preferences,
    path: PathBuf,
    method: &str,
    cancel: &AtomicBool,
) -> LatencyResult {
    latency_on_route(profile, prefs, path, method, cancel, None)
}
pub fn latency_on_route(
    profile: Profile,
    prefs: Preferences,
    path: PathBuf,
    method: &str,
    cancel: &AtomicBool,
    route: Option<&DiagnosticRoute>,
) -> LatencyResult {
    latency_samples_on_route(profile, prefs, path, method, cancel, route, 2)
}
pub fn latency_samples_on_route(
    profile: Profile,
    prefs: Preferences,
    path: PathBuf,
    method: &str,
    cancel: &AtomicBool,
    route: Option<&DiagnosticRoute>,
    samples: usize,
) -> LatencyResult {
    let mut result = LatencyResult {
        profile_id: profile.id.clone(),
        protocol: profile.summary().protocol,
        method: method.into(),
        timestamp: crate::store::now(),
        destination: if ["tcp_connect_rtt", "icmp_rtt"].contains(&method) {
            format!("{}:{}", profile.summary().host, profile.summary().port)
        } else {
            prefs.probe_url.clone()
        },
        min_ms: None,
        avg_ms: None,
        max_ms: None,
        successes: 0,
        timeouts: 0,
        failures: 0,
        samples: vec![],
        error: None,
        note: if method == "icmp_rtt" {
            "ICMP echo: direct host reachability, not authentication or VPN transport".into()
        } else if method == "tcp_connect_rtt" {
            "TCP handshake outside NetPulse TUN; does not prove authentication, TLS, UDP or Hysteria2 connectivity"
                .into()
        } else {
            "HTTPS request through an isolated authenticated proxy; DNS/TLS may be included".into()
        },
    };
    let run = (|| {
        cancelled(cancel)?;
        if ![
            "tcp_connect_rtt",
            "proxy_https_rtt",
            "proxy_https_head_rtt",
            "icmp_rtt",
        ]
        .contains(&method)
        {
            return Err(Error::new(
                "LatencyMethod",
                "latency-start",
                "Choose Proxy GET/HEAD, TCP or ICMP",
            ));
        }
        let mut isolated = prefs.clone();
        isolated.tun = false;
        isolated.system_proxy = false;
        isolated.route_mode = crate::model::RouteMode::Global;
        isolated.proxy_port = free_port()?;
        let password = route
            .map(|r| r.password.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let group = if method.starts_with("proxy_https") && route.is_none() {
            let plan = RuntimePlan::new(
                &path,
                &profile,
                &isolated,
                free_port()?,
                &uuid::Uuid::new_v4().to_string(),
                &password,
                [free_port()?, free_port()?],
            )?;
            plan.validate(&path)?;
            Some((plan.start(&path)?, plan.probe_port))
        } else {
            None
        };
        let proxy_port = route
            .and_then(|r| r.vpn_port)
            .or_else(|| group.as_ref().map(|(_, port)| *port));
        let shared_client =
            if let Some(port) = proxy_port.filter(|_| method.starts_with("proxy_https")) {
                Some(http(Some(port), &password, Duration::from_secs(4))?)
            } else {
                None
            };
        for _ in 0..samples.clamp(1, 15) {
            cancelled(cancel)?;
            let mut started = Instant::now();
            let sample: Result<()> = if let Some(port) =
                proxy_port.filter(|_| method.starts_with("proxy_https"))
            {
                let fresh;
                let client = if prefs.ping_proxy_mode == "keepalive" {
                    shared_client.as_ref().unwrap()
                } else {
                    fresh = http(Some(port), &password, Duration::from_secs(4))?;
                    &fresh
                };
                let url = endpoint(&prefs.probe_url)?;
                let request = if method == "proxy_https_head_rtt" {
                    client.head(url)
                } else {
                    client.get(url)
                };
                started = Instant::now();
                request.send().map_err(|e|Error::new(if e.is_timeout(){"LatencyTimeout"}else{"LatencyFailed"},"latency-probe","HTTPS через выбранный профиль не отвечает")).and_then(|response|{
                    if !response.status().is_success(){return Err(Error::new("LatencyHttpError","latency-probe",format!("HTTP {}",response.status().as_u16())));}
                    // Drain only a bounded test response so keepalive can reuse TLS.
                    let mut body=Vec::new();response.take(65537).read_to_end(&mut body).map_err(|e|Error::io("latency-body",e))?;
                    if body.len()>65536{return Err(Error::new("LatencyResponseLimit","latency-body","Для проверки выберите небольшой HTTPS ответ, например generate_204"));}Ok(())
                })
            } else if method == "icmp_rtt" {
                match crate::icmp::sample(&profile.summary().host) {
                    Ok(ms) => {
                        result.samples.push(ms);
                        continue;
                    }
                    Err(error) => Err(error),
                }
            } else {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| Error::io("latency-dns", e))?;
                let host = profile.summary().host;
                let port = profile.summary().port;
                rt.block_on(async {
                    tokio::time::timeout(Duration::from_secs(4), async {
                        let addresses = tokio::net::lookup_host((host.as_str(), port)).await?;
                        started = Instant::now();
                        let mut last = None;
                        for address in addresses {
                            let socket = if address.is_ipv4() {
                                tokio::net::TcpSocket::new_v4()?
                            } else {
                                tokio::net::TcpSocket::new_v6()?
                            };
                            #[cfg(windows)]
                            if !address.ip().is_loopback()
                                && let Err(e) =
                                    crate::routes::bind_outside_tun(&socket, address.ip())
                            {
                                last = Some(e);
                                continue;
                            }
                            // Measure the remote handshake, not route lookup or a local TUN accept.
                            started = Instant::now();
                            match socket.connect(address).await {
                                Ok(_) => return Ok(()),
                                Err(e) => last = Some(e),
                            }
                        }
                        Err(last.unwrap_or_else(|| {
                            std::io::Error::new(std::io::ErrorKind::NotFound, "No address")
                        }))
                    })
                    .await
                    .map_err(|_| {
                        Error::new("LatencyTimeout", "latency-connect", "TCP connect timed out")
                    })?
                    .map_err(|e| Error::io("latency-connect", e))
                })
            };
            match sample {
                Ok(()) => result
                    .samples
                    .push(started.elapsed().as_secs_f64() * 1000.0),
                Err(e) => {
                    if e.code == "LatencyTimeout" {
                        result.timeouts += 1;
                    } else {
                        result.failures += 1;
                    }
                    result.error = Some(e);
                }
            }
        }
        drop(group);
        Ok(())
    })();
    if let Err(e) = run {
        result.error = Some(e);
    }
    result.successes = result.samples.len();
    if !result.samples.is_empty() {
        result.min_ms = result.samples.iter().copied().reduce(f64::min);
        result.max_ms = result.samples.iter().copied().reduce(f64::max);
        result.avg_ms = Some(result.samples.iter().sum::<f64>() / result.successes as f64);
    }
    result
}
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct SpeedRequest {
    pub mode: String,
    pub download_url: String,
    pub upload_url: Option<String>,
    pub upload_endpoint_confirmed: bool,
    pub duration_seconds: u64,
    pub max_payload_mib: u64,
}
impl SpeedRequest {
    pub fn validate(&self) -> Result<()> {
        if !["direct", "vpn", "compare"].contains(&self.mode.as_str())
            || !(2..=30).contains(&self.duration_seconds)
            || !(1..=100).contains(&self.max_payload_mib)
        {
            return Err(Error::new(
                "InvalidSpeedTest",
                "speed-validate",
                "Choose Direct/VPN/Compare, 2–30 seconds and 1–100 MiB total payload",
            ));
        }
        endpoint(&self.download_url)?;
        if let Some(u) = &self.upload_url {
            endpoint(u)?;
            if !self.upload_endpoint_confirmed {
                return Err(Error::new(
                    "UploadNotConfirmed",
                    "speed-validate",
                    "Confirm that this endpoint is designed to accept test uploads",
                ));
            }
        }
        Ok(())
    }
}
#[derive(Clone)]
pub struct DiagnosticRoute {
    pub direct_port: Option<u16>,
    pub vpn_port: Option<u16>,
    pub password: zeroize::Zeroizing<String>,
    pub engine_pid: Option<u32>,
}
#[derive(Serialize, Clone, Debug)]
pub struct SpeedResult {
    pub mode: String,
    pub timestamp: u64,
    pub route: String,
    pub engine_pid: Option<u32>,
    pub download_bytes: u64,
    pub upload_bytes: u64,
    pub download_mbps: Option<f64>,
    pub upload_mbps: Option<f64>,
    pub duration_ms: u64,
    pub samples: Vec<SpeedSample>,
    pub cancelled: bool,
    pub error: Option<Error>,
    pub idle_https_ms: Option<f64>,
    pub loaded_latency_ms: Option<f64>,
    pub jitter_ms: Option<f64>,
}
#[derive(Serialize, Clone, Debug)]
pub struct SpeedSample {
    pub elapsed_ms: u64,
    pub bytes: u64,
    pub direction: String,
}
pub fn speed(
    request: &SpeedRequest,
    route: &DiagnosticRoute,
    mode: &str,
    budget: u64,
    cancel: &AtomicBool,
) -> SpeedResult {
    speed_observed(request, route, mode, budget, cancel, &|_| {})
}
pub fn speed_observed(
    request: &SpeedRequest,
    route: &DiagnosticRoute,
    mode: &str,
    budget: u64,
    cancel: &AtomicBool,
    progress: &dyn Fn(&SpeedSample),
) -> SpeedResult {
    let started = Instant::now();
    let port = if mode == "vpn" {
        route.vpn_port
    } else {
        route.direct_port
    };
    let mut result = SpeedResult {
        mode: mode.into(),
        timestamp: crate::store::now(),
        route: if port.is_some() {
            format!("Owned diagnostic inbound forced to {mode}")
        } else {
            "Windows default route; external VPNs are not bypassed".into()
        },
        engine_pid: route.engine_pid,
        download_bytes: 0,
        upload_bytes: 0,
        download_mbps: None,
        upload_mbps: None,
        duration_ms: 0,
        samples: vec![],
        cancelled: false,
        error: None,
        idle_https_ms: None,
        loaded_latency_ms: None,
        jitter_ms: None,
    };
    let run = (|| {
        request.validate()?;
        cancelled(cancel)?;
        if mode == "vpn" && port.is_none() {
            return Err(Error::new(
                "NotConnected",
                "speed-route",
                "VPN speed test requires an active verified connection",
            ));
        }
        let timeout = Duration::from_secs(request.duration_seconds);
        let c = http(port, &route.password, timeout)?;
        let mut idle = vec![];
        for _ in 0..3 {
            cancelled(cancel)?;
            let t = Instant::now();
            if c.head(endpoint(&request.download_url)?)
                .timeout(Duration::from_secs(1))
                .send()
                .is_ok_and(|r| r.status().is_success())
            {
                idle.push(t.elapsed().as_secs_f64() * 1000.0);
            }
        }
        if !idle.is_empty() {
            result.idle_https_ms = Some(idle.iter().sum::<f64>() / idle.len() as f64);
        }
        let download_budget = if request.upload_url.is_some() {
            budget / 2
        } else {
            budget
        };
        let down_start = Instant::now();
        let mut response = c
            .get(endpoint(&request.download_url)?)
            .header("Cache-Control", "no-cache")
            .header(
                "Range",
                format!("bytes=0-{}", download_budget.saturating_sub(1)),
            )
            .send()
            .map_err(|e| {
                Error::new(
                    if e.is_timeout() {
                        "SpeedTimeout"
                    } else {
                        "SpeedDownloadFailed"
                    },
                    "speed-download",
                    "Download request failed; partial data is retained",
                )
            })?;
        if !response.status().is_success() {
            return Err(Error::new(
                "SpeedHttpError",
                "speed-download",
                format!("HTTP {}", response.status().as_u16()),
            ));
        }
        let mut buf = [0u8; 32768];
        let mut load = LoadProbe::start(c.clone(), request.download_url.clone());
        let mut last_sample = Instant::now();
        while result.download_bytes < download_budget && down_start.elapsed() < timeout {
            cancelled(cancel)?;
            let limit = (download_budget - result.download_bytes).min(buf.len() as u64) as usize;
            let n = response.read(&mut buf[..limit]).map_err(|_| {
                Error::new(
                    "SpeedReadFailed",
                    "speed-download",
                    "Download stream failed or reached its timeout; partial data is retained",
                )
            })?;
            if n == 0 {
                break;
            }
            result.download_bytes += n as u64;
            if last_sample.elapsed() >= Duration::from_millis(200) {
                result.samples.push(SpeedSample {
                    elapsed_ms: started.elapsed().as_millis() as u64,
                    bytes: result.download_bytes,
                    direction: "download".into(),
                });
                progress(result.samples.last().unwrap());
                last_sample = Instant::now();
            }
        }
        if result.download_bytes > 0 {
            result.download_mbps = Some(
                result.download_bytes as f64 * 8.0
                    / down_start.elapsed().as_secs_f64()
                    / 1_000_000.0,
            );
        }
        let loaded = load.finish();
        if !loaded.is_empty() {
            result.loaded_latency_ms = Some(loaded.iter().sum::<f64>() / loaded.len() as f64);
        }
        if loaded.len() > 1 {
            result.jitter_ms = Some(
                loaded.windows(2).map(|v| (v[1] - v[0]).abs()).sum::<f64>()
                    / (loaded.len() - 1) as f64,
            );
        }
        result.samples.push(SpeedSample {
            elapsed_ms: started.elapsed().as_millis() as u64,
            bytes: result.download_bytes,
            direction: "download".into(),
        });
        progress(result.samples.last().unwrap());
        drop(response);
        cancelled(cancel)?;
        if let Some(upload) = &request.upload_url {
            let n = (budget - result.download_bytes).min(4 * 1024 * 1024);
            let body = vec![0u8; n as usize];
            let up_start = Instant::now();
            let response = c
                .post(endpoint(upload)?)
                .header("Content-Type", "application/octet-stream")
                .body(body)
                .send()
                .map_err(|_| {
                    Error::new(
                        "SpeedUploadFailed",
                        "speed-upload",
                        "Upload failed; uploaded amount is unknown and throughput was not recorded",
                    )
                })?;
            if !response.status().is_success() {
                return Err(Error::new(
                    "SpeedHttpError",
                    "speed-upload",
                    format!("HTTP {}", response.status().as_u16()),
                ));
            }
            result.upload_bytes = n;
            result.upload_mbps =
                Some(n as f64 * 8.0 / up_start.elapsed().as_secs_f64() / 1_000_000.0);
            result.samples.push(SpeedSample {
                elapsed_ms: started.elapsed().as_millis() as u64,
                bytes: n,
                direction: "upload".into(),
            });
            progress(result.samples.last().unwrap());
        }
        Ok(())
    })();
    if let Err(e) = run {
        result.error = Some(e);
    }
    result.cancelled = cancel.load(Ordering::SeqCst);
    result.duration_ms = started.elapsed().as_millis() as u64;
    result
}
struct LoadProbe {
    stop: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<Vec<f64>>>,
}
impl LoadProbe {
    fn start(client: reqwest::blocking::Client, url: String) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let join = std::thread::spawn(move || {
            let mut samples = vec![];
            for _ in 0..60 {
                for _ in 0..6 {
                    if flag.load(Ordering::SeqCst) {
                        return samples;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                let t = Instant::now();
                if client
                    .head(&url)
                    .timeout(Duration::from_secs(1))
                    .send()
                    .is_ok_and(|r| r.status().is_success())
                    && !flag.load(Ordering::SeqCst)
                {
                    samples.push(t.elapsed().as_secs_f64() * 1000.0);
                }
            }
            samples
        });
        Self {
            stop,
            join: Some(join),
        }
    }
    fn finish(&mut self) -> Vec<f64> {
        self.stop.store(true, Ordering::SeqCst);
        self.join
            .take()
            .and_then(|j| j.join().ok())
            .unwrap_or_default()
    }
}
impl Drop for LoadProbe {
    fn drop(&mut self) {
        self.finish();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn jobs_cancel_and_serialize_state() {
        let jobs = Jobs::default();
        let (id, cancel) = jobs.begin("latency", 2).unwrap();
        assert!(jobs.begin("speed", 1).is_err());
        jobs.append(&id, serde_json::json!({"avg_ms":null}));
        jobs.cancel(&id).unwrap();
        assert!(cancel.load(Ordering::SeqCst));
        jobs.finish(&id, None);
        let s = jobs.status(&id).unwrap();
        assert_eq!(s.completed, 1);
        assert!(s.cancelled);
        assert!(!s.running);
        assert!(jobs.begin("speed", 1).is_ok());
    }
    #[test]
    fn speed_validation_and_cancel_without_network() {
        let mut r = SpeedRequest {
            mode: "direct".into(),
            download_url: "https://example.org/download".into(),
            upload_url: Some("https://example.org/upload".into()),
            upload_endpoint_confirmed: false,
            duration_seconds: 2,
            max_payload_mib: 1,
        };
        assert!(r.validate().is_err());
        r.upload_endpoint_confirmed = true;
        assert!(r.validate().is_ok());
        let route = DiagnosticRoute {
            direct_port: None,
            vpn_port: None,
            password: zeroize::Zeroizing::new(String::new()),
            engine_pid: None,
        };
        let result = speed(&r, &route, "vpn", 1024, &AtomicBool::new(false));
        assert!(result.download_mbps.is_none());
        assert_eq!(result.error.unwrap().code, "NotConnected");
        let result = speed(&r, &route, "direct", 1024, &AtomicBool::new(true));
        assert!(result.cancelled);
        assert_eq!(result.download_bytes, 0);
    }
}
