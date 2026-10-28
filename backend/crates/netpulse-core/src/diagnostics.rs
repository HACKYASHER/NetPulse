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
        .no_gzip() // Speed measures transferred payload, never decompressed size.
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
            || !(2..=120).contains(&self.duration_seconds)
            || !(1..=1024).contains(&self.max_payload_mib)
        {
            return Err(Error::new(
                "InvalidSpeedTest",
                "speed-validate",
                "Choose Direct/VPN/Compare, 2–120 seconds per route and 1–1024 MiB total payload",
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
    pub requested_duration_seconds: u64,
    pub payload_limit_bytes: u64,
    pub download_duration_ms: u64,
    pub upload_duration_ms: u64,
    pub download_requests: u32,
    pub upload_requests: u32,
    pub payload_limit_reached: bool,
}
#[derive(Serialize, Clone, Debug)]
pub struct SpeedSample {
    pub elapsed_ms: u64,
    pub bytes: u64,
    pub direction: String,
    pub mode: String,
    pub download_bytes: u64,
    pub upload_bytes: u64,
    pub payload_limit_bytes: u64,
    pub phase_elapsed_ms: u64,
    pub phase_duration_ms: u64,
}
fn speed_sample(
    result: &mut SpeedResult,
    started: Instant,
    phase_started: Instant,
    phase_duration: Duration,
    direction: &str,
    progress: &dyn Fn(&SpeedSample),
) {
    let sample = SpeedSample {
        elapsed_ms: started.elapsed().as_millis() as u64,
        bytes: if direction == "upload" { result.upload_bytes } else { result.download_bytes },
        direction: direction.into(),
        mode: result.mode.clone(),
        download_bytes: result.download_bytes,
        upload_bytes: result.upload_bytes,
        payload_limit_bytes: result.payload_limit_bytes,
        phase_elapsed_ms: phase_started.elapsed().as_millis() as u64,
        phase_duration_ms: phase_duration.as_millis() as u64,
    };
    progress(&sample);
    result.samples.push(sample);
}
fn download_endpoint(url: &str, bytes: u64) -> Result<url::Url> {
    let mut endpoint = endpoint(url)?;
    // Cloudflare accepts an explicit size; cap each response as well as the
    // local reader so a short test does not request an oversized body.
    if endpoint.host_str() == Some("speed.cloudflare.com") && endpoint.path() == "/__down" {
        let params: Vec<(String, String)> = endpoint.query_pairs()
            .filter(|(key, _)| key != "bytes")
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect();
        endpoint.query_pairs_mut().clear().extend_pairs(params)
            .append_pair("bytes", &bytes.to_string());
    }
    Ok(endpoint)
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
        requested_duration_seconds: request.duration_seconds,
        payload_limit_bytes: budget.min(request.max_payload_mib.saturating_mul(1024 * 1024)),
        download_duration_ms: 0,
        upload_duration_ms: 0,
        download_requests: 0,
        upload_requests: 0,
        payload_limit_reached: false,
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
        let budget = result.payload_limit_bytes;
        if budget == 0 {
            return Err(Error::new("InvalidSpeedTest", "speed-validate", "The speed-test payload limit must be positive"));
        }
        let timeout = Duration::from_secs(request.duration_seconds);
        // Individual transfers are bounded so cancellation does not have to
        // wait for an entire 120-second test when an endpoint stops replying.
        let transfer_timeout = Duration::from_secs(5);
        let c = http(port, &route.password, transfer_timeout)?;
        speed_sample(&mut result, started, started, Duration::ZERO, "idle", progress);
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
        // Download and optional upload share the requested time on each route.
        // Repeat finite responses until that phase expires or its byte budget
        // is exhausted; one small response is not a sustained speed test.
        let phase_duration = if request.upload_url.is_some() { timeout / 2 } else { timeout };
        let down_start = Instant::now();
        let mut buf = [0u8; 32768];
        let mut load = LoadProbe::start(c.clone(), request.download_url.clone());
        let mut last_sample = Instant::now();
        speed_sample(&mut result, started, down_start, phase_duration, "download", progress);
        while result.download_bytes < download_budget && down_start.elapsed() < phase_duration {
            cancelled(cancel)?;
            let remaining = phase_duration.saturating_sub(down_start.elapsed());
            if remaining.is_zero() { break; }
            let chunk_budget = (download_budget - result.download_bytes).min(8 * 1024 * 1024);
            let transfer_start = Instant::now();
            let transfer_time = remaining.min(transfer_timeout);
            let response = c
                .get(download_endpoint(&request.download_url, chunk_budget)?)
                .timeout(transfer_time)
                .header("Cache-Control", "no-cache, no-store")
                .header("Range", format!("bytes=0-{}", chunk_budget.saturating_sub(1)))
                .send();
            let mut response = match response {
                Ok(response) => response,
                Err(error) if error.is_timeout() && down_start.elapsed() >= phase_duration && result.download_bytes > 0 => break,
                Err(error) => return Err(Error::new(
                    if error.is_timeout() { "SpeedTimeout" } else { "SpeedDownloadFailed" },
                    "speed-download", "Download request failed; partial data is retained",
                )),
            };
            if !response.status().is_success() {
                return Err(Error::new("SpeedHttpError", "speed-download", format!("HTTP {}", response.status().as_u16())));
            }
            result.download_requests += 1;
            let before = result.download_bytes;
            while result.download_bytes - before < chunk_budget && down_start.elapsed() < phase_duration {
                cancelled(cancel)?;
                let limit = (chunk_budget - (result.download_bytes - before)).min(buf.len() as u64) as usize;
                let n = match response.read(&mut buf[..limit]) {
                    Ok(n) => n,
                    Err(_) if transfer_start.elapsed() >= transfer_time && result.download_bytes > before => break,
                    Err(_) => return Err(Error::new("SpeedReadFailed", "speed-download", "Download stream failed; partial data is retained")),
                };
                if n == 0 { break; }
                result.download_bytes += n as u64;
                if last_sample.elapsed() >= Duration::from_millis(200) {
                    speed_sample(&mut result, started, down_start, phase_duration, "download", progress);
                    last_sample = Instant::now();
                }
            }
            if result.download_bytes == before {
                return Err(Error::new("SpeedEmptyResponse", "speed-download", "The download endpoint returned no test data"));
            }
        }
        result.download_duration_ms = down_start.elapsed().as_millis() as u64;
        if result.download_bytes > 0 {
            result.download_mbps = Some(
                result.download_bytes as f64 * 8.0
                    / down_start.elapsed().as_secs_f64().max(f64::EPSILON)
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
        speed_sample(&mut result, started, down_start, phase_duration, "download", progress);
        cancelled(cancel)?;
        if let Some(upload) = &request.upload_url {
            let upload_budget = budget - result.download_bytes;
            let upload_endpoint = endpoint(upload)?;
            let up_start = Instant::now();
            speed_sample(&mut result, started, up_start, phase_duration, "upload", progress);
            while result.upload_bytes < upload_budget && up_start.elapsed() < phase_duration {
                cancelled(cancel)?;
                let remaining = phase_duration.saturating_sub(up_start.elapsed());
                if remaining.is_zero() { break; }
                let n = (upload_budget - result.upload_bytes).min(1024 * 1024);
                let response = c
                    .post(upload_endpoint.clone())
                    .timeout(remaining.min(transfer_timeout))
                    .header("Content-Type", "application/octet-stream")
                    .body(vec![0u8; n as usize])
                    .send()
                    .map_err(|_| Error::new("SpeedUploadFailed", "speed-upload", "Upload failed; the last unconfirmed payload is not included in throughput"))?;
                if !response.status().is_success() {
                    return Err(Error::new("SpeedHttpError", "speed-upload", format!("HTTP {}", response.status().as_u16())));
                }
                result.upload_requests += 1;
                result.upload_bytes += n;
                if last_sample.elapsed() >= Duration::from_millis(200) {
                    speed_sample(&mut result, started, up_start, phase_duration, "upload", progress);
                    last_sample = Instant::now();
                }
            }
            result.upload_duration_ms = up_start.elapsed().as_millis() as u64;
            if result.upload_bytes > 0 {
                result.upload_mbps = Some(result.upload_bytes as f64 * 8.0 / up_start.elapsed().as_secs_f64().max(f64::EPSILON) / 1_000_000.0);
            }
            speed_sample(&mut result, started, up_start, phase_duration, "upload", progress);
        }
        result.payload_limit_reached = result.download_bytes + result.upload_bytes >= budget;
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
            while !flag.load(Ordering::SeqCst) {
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
        r.duration_seconds = 120;
        r.max_payload_mib = 1024;
        assert!(r.validate().is_ok());
        r.duration_seconds = 121;
        assert!(r.validate().is_err());
        r.duration_seconds = 30;
        r.max_payload_mib = 1025;
        assert!(r.validate().is_err());
    }
    #[test]
    fn cloudflare_response_size_is_bounded_without_changing_custom_endpoints() {
        let url = download_endpoint("https://speed.cloudflare.com/__down?bytes=100000000&test=one", 4096).unwrap();
        let pairs: Vec<_> = url.query_pairs().collect();
        assert_eq!(pairs.iter().filter(|(key,_)| key == "bytes").count(), 1);
        assert!(pairs.iter().any(|(key,value)| key == "bytes" && value == "4096"));
        assert!(pairs.iter().any(|(key,value)| key == "test" && value == "one"));
        let custom = "https://example.org/download?bytes=10";
        assert_eq!(download_endpoint(custom, 4096).unwrap().as_str(), custom);
    }
    /// Owned TLS fixture only. A short finite response exercises the repeated
    /// transfer loop without relying on an external speed-test service.
    struct SpeedOrigin {
        port: u16,
        stop: Arc<AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }
    impl SpeedOrigin {
        fn new() -> Self {
            use std::{io::Write, net::TcpListener};
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            listener.set_nonblocking(true).unwrap();
            let identity = native_tls::Identity::from_pkcs12(
                include_bytes!("../tests/fixtures/owned-localhost.pfx"), "fixture-only",
            ).unwrap();
            let acceptor = native_tls::TlsAcceptor::new(identity).unwrap();
            let stop = Arc::new(AtomicBool::new(false));
            let flag = stop.clone();
            let thread = std::thread::spawn(move || {
                while !flag.load(Ordering::SeqCst) {
                    if let Ok((socket, _)) = listener.accept() {
                        socket.set_nonblocking(false).unwrap();
                        socket.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                        socket.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
                        if let Ok(mut stream) = acceptor.accept(socket) {
                            let mut request = [0u8; 4096];
                            if let Ok(n) = stream.read(&mut request) {
                                if request[..n].starts_with(b"HEAD ") {
                                    let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                                } else {
                                    std::thread::sleep(Duration::from_millis(20));
                                    let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4096\r\nConnection: close\r\n\r\n");
                                    let _ = stream.write_all(&[7u8; 4096]);
                                }
                                let _ = stream.flush();
                            }
                        }
                    } else {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                }
            });
            Self { port, stop, thread:Some(thread) }
        }
        fn request(&self) -> SpeedRequest {
            SpeedRequest { mode:"direct".into(),download_url:format!("https://127.0.0.1:{}/download",self.port),upload_url:None,upload_endpoint_confirmed:false,duration_seconds:2,max_payload_mib:1 }
        }
    }
    impl Drop for SpeedOrigin {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
            if let Some(thread) = self.thread.take() { thread.join().unwrap(); }
        }
    }
    fn direct_route() -> DiagnosticRoute {
        DiagnosticRoute { direct_port:None,vpn_port:None,password:zeroize::Zeroizing::new(String::new()),engine_pid:None }
    }
    #[test]
    fn speed_repeats_finite_responses_and_reports_the_payload_limit() {
        let origin = SpeedOrigin::new();
        let result = speed(&origin.request(), &direct_route(), "direct", 32*1024, &AtomicBool::new(false));
        assert!(result.error.is_none(), "{:?}",result.error);
        assert_eq!(result.download_requests, 8);
        assert_eq!(result.download_bytes, 32*1024);
        assert!(result.payload_limit_reached);
        assert!(result.download_mbps.is_some_and(|value|value>0.0));
        let progress = result.samples.last().unwrap();
        assert_eq!(progress.mode, "direct");
        assert_eq!(progress.download_bytes, 32*1024);
        assert_eq!(progress.payload_limit_bytes, 32*1024);
    }
    #[test]
    fn speed_runs_until_the_time_window_and_can_cancel_partial_transfers() {
        let origin = SpeedOrigin::new();
        let result = speed(&origin.request(), &direct_route(), "direct", 1024*1024, &AtomicBool::new(false));
        assert!(result.error.is_none(), "{:?}", result.error);
        assert!(result.download_requests>1);
        assert!(result.download_bytes>4096 && result.download_bytes<1024*1024);
        assert!(result.download_duration_ms>=1900 && result.download_duration_ms<5000);
        assert!(!result.payload_limit_reached);
        let cancel = AtomicBool::new(false);
        let result = speed_observed(&origin.request(), &direct_route(), "direct", 1024*1024, &cancel, &|sample| {
            if sample.download_bytes >= 4096 { cancel.store(true, Ordering::SeqCst); }
        });
        assert!(result.cancelled);
        assert_eq!(result.error.as_ref().unwrap().code,"Cancelled");
        assert!(result.download_bytes>0 && result.download_bytes<1024*1024);
        assert!(result.duration_ms<2000);
    }
}
