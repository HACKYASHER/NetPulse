//! Read-only Clash snapshots, outside the connection-controller mutex.
//! Requests coalesce; there is no timer, connection eviction or DELETE request.
use crate::{Error, Result, network::Metrics, playground::Connection};
use serde::{
    Deserialize, Deserializer,
    de::{SeqAccess, Visitor},
};
use std::{
    fmt,
    io::Read,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{SyncSender, sync_channel},
    },
    time::{Duration, Instant},
};
use zeroize::Zeroizing;
const LIMIT: u64 = 4 * 1024 * 1024;
const NORMAL_INTERVAL: Duration = Duration::from_secs(5);
const MIN_INTERVAL: Duration = Duration::from_secs(1);
pub struct Sample {
    pub metrics: Metrics,
    pub rows: Vec<Connection>,
    at: Instant,
}
#[derive(Default)]
struct State {
    sample: Option<Arc<Sample>>,
    error: Option<Error>,
    sequence: u64,
    attempted: Option<Instant>,
    fetching: bool,
}
struct Shared {
    state: Mutex<State>,
    changed: Condvar,
    urgent: AtomicBool,
    stop: AtomicBool,
}
#[derive(Clone)]
pub struct Reader {
    shared: Arc<Shared>,
    requests: SyncSender<()>,
}
pub struct Monitor {
    pub reader: Reader,
}
impl Drop for Monitor {
    fn drop(&mut self) {
        self.reader.shared.stop.store(true, Ordering::Release);
        let _ = self.reader.requests.try_send(());
        self.reader.shared.changed.notify_all();
    }
}
struct Connections {
    count: usize,
    retained: Vec<serde_json::Value>,
}
impl<'de> Deserialize<'de> for Connections {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct Rows;
        impl<'de> Visitor<'de> for Rows {
            type Value = Connections;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("connection array or null")
            }
            fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<Self::Value, E> {
                Ok(Connections {
                    count: 0,
                    retained: vec![],
                })
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut result = Connections {
                    count: 0,
                    retained: Vec::with_capacity(300),
                };
                loop {
                    if result.count < 300 {
                        let Some(row) = seq.next_element()? else {
                            break;
                        };
                        result.retained.push(row);
                    } else if seq.next_element::<serde::de::IgnoredAny>()?.is_none() {
                        break;
                    }
                    result.count += 1;
                }
                Ok(result)
            }
        }
        d.deserialize_any(Rows)
    }
}
#[derive(Deserialize)]
struct Document {
    #[serde(rename = "uploadTotal")]
    upload: u64,
    #[serde(rename = "downloadTotal")]
    download: u64,
    connections: Connections,
}
fn fetch(
    client: &reqwest::blocking::Client,
    port: u16,
    secret: &str,
    previous: Option<&Sample>,
) -> Result<Sample> {
    let response = client
        .get(format!("http://127.0.0.1:{port}/connections"))
        .bearer_auth(secret)
        .send()
        .map_err(|_| {
            Error::new(
                "MetricsUnavailable",
                "metrics-read",
                "Список соединений ядра временно недоступен",
            )
        })?;
    if !response.status().is_success() {
        return Err(Error::new(
            "MetricsUnavailable",
            "metrics-read",
            "Ядро отклонило чтение статистики",
        ));
    }
    let mut bytes = Vec::new();
    response
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| Error::io("metrics-read", e))?;
    if bytes.len() as u64 > LIMIT {
        return Err(Error::new(
            "MetricsLimit",
            "metrics-read",
            "Статистика превысила лимит 4 МиБ; подключение сохранено",
        ));
    }
    let doc: Document = serde_json::from_slice(&bytes).map_err(|_| {
        Error::new(
            "MetricsInvalid",
            "metrics-read",
            "Неверный ответ статистики ядра",
        )
    })?;
    let mut metrics = Metrics {
        upload_total: doc.upload,
        download_total: doc.download,
        connections: doc.connections.count,
        ..Default::default()
    };
    if let Some(old) = previous {
        let seconds = old.at.elapsed().as_secs_f64().max(0.001);
        metrics.upload_bps =
            (doc.upload.saturating_sub(old.metrics.upload_total) as f64 / seconds) as u64;
        metrics.download_bps =
            (doc.download.saturating_sub(old.metrics.download_total) as f64 / seconds) as u64;
    }
    let rows = crate::playground::core_connections(
        &serde_json::json!({"connections":doc.connections.retained}),
        "",
    );
    Ok(Sample {
        metrics,
        rows,
        at: Instant::now(),
    })
}
impl Monitor {
    pub fn new(port: u16, secret: Zeroizing<String>) -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .timeout(Duration::from_millis(1500))
            .redirect(reqwest::redirect::Policy::none())
            .pool_max_idle_per_host(1)
            .pool_idle_timeout(Duration::from_secs(10))
            .build()
            .map_err(|_| {
                Error::new(
                    "MetricsClient",
                    "metrics-read",
                    "Не удалось создать локальный клиент статистики",
                )
            })?;
        let shared = Arc::new(Shared {
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
            urgent: AtomicBool::new(false),
            stop: AtomicBool::new(false),
        });
        let (requests, receiver) = sync_channel(1);
        let worker = shared.clone();
        std::thread::spawn(move || {
            while receiver.recv().is_ok() && !worker.stop.load(Ordering::Acquire) {
                let urgent = worker.urgent.swap(false, Ordering::AcqRel);
                let previous = {
                    let Ok(mut state) = worker.state.lock() else {
                        break;
                    };
                    if state.attempted.is_some_and(|at| {
                        at.elapsed()
                            < if urgent {
                                MIN_INTERVAL
                            } else {
                                NORMAL_INTERVAL
                            }
                    }) {
                        continue;
                    }
                    state.attempted = Some(Instant::now());
                    state.fetching = true;
                    state.sample.clone()
                };
                let result = fetch(&client, port, &secret, previous.as_deref());
                if worker.stop.load(Ordering::Acquire) {
                    break;
                }
                if let Ok(mut state) = worker.state.lock() {
                    match result {
                        Ok(sample) => {
                            state.sample = Some(Arc::new(sample));
                            state.error = None;
                        }
                        Err(e) => state.error = Some(e),
                    }
                    state.sequence += 1;
                    state.fetching = false;
                    worker.changed.notify_all();
                }
            }
        });
        Ok(Self {
            reader: Reader { shared, requests },
        })
    }
}
impl Reader {
    pub fn request(&self) {
        let _ = self.requests.try_send(());
    }
    pub fn cached_metrics(&self) -> (Option<Metrics>, Option<Error>) {
        let Ok(state) = self.shared.state.try_lock() else {
            return (None, None);
        };
        (
            state.sample.as_ref().map(|s| s.metrics.clone()),
            state.error.clone(),
        )
    }
    pub fn fresh(&self) -> Result<Arc<Sample>> {
        if self.shared.stop.load(Ordering::Acquire) {
            return Err(Error::new(
                "SessionChanged",
                "playground",
                "Подключение изменилось; обновите наблюдение",
            ));
        }
        let mut state = self.shared.state.lock().map_err(|_| {
            Error::new(
                "MetricsUnavailable",
                "metrics-read",
                "Монитор статистики недоступен",
            )
        })?;
        if !state.fetching
            && state
                .attempted
                .is_some_and(|at| at.elapsed() < MIN_INTERVAL)
        {
            if let Some(error) = &state.error {
                return Err(error.clone());
            }
            if let Some(sample) = &state.sample {
                return Ok(sample.clone());
            }
        }
        let sequence = state.sequence;
        self.shared.urgent.store(true, Ordering::Release);
        self.request();
        let deadline = Instant::now() + Duration::from_millis(2500);
        while state.sequence == sequence && !self.shared.stop.load(Ordering::Acquire) {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            state = self
                .shared
                .changed
                .wait_timeout(state, left)
                .map_err(|_| {
                    Error::new(
                        "MetricsUnavailable",
                        "metrics-read",
                        "Монитор статистики недоступен",
                    )
                })?
                .0;
        }
        if self.shared.stop.load(Ordering::Acquire) {
            return Err(Error::new(
                "SessionChanged",
                "playground",
                "Подключение изменилось; обновите наблюдение",
            ));
        }
        if state.sequence == sequence {
            return Err(Error::new(
                "MetricsPending",
                "metrics-read",
                "Чтение статистики ещё не завершено",
            ));
        }
        if let Some(error) = &state.error {
            return Err(error.clone());
        }
        state.sample.clone().ok_or_else(|| {
            Error::new(
                "MetricsPending",
                "metrics-read",
                "Статистика ещё не получена",
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader, Write},
        net::TcpListener,
        sync::atomic::AtomicUsize,
    };
    struct Api {
        port: u16,
        count: Arc<AtomicUsize>,
        stop: Arc<AtomicBool>,
        worker: Option<std::thread::JoinHandle<()>>,
    }
    impl Api {
        fn new(response: impl Fn(usize) -> (Duration, Vec<u8>) + Send + 'static) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            listener.set_nonblocking(true).unwrap();
            let count = Arc::new(AtomicUsize::new(0));
            let stop = Arc::new(AtomicBool::new(false));
            let calls = count.clone();
            let done = stop.clone();
            let worker = std::thread::spawn(move || {
                while !done.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((mut socket, _)) => {
                            socket
                                .set_read_timeout(Some(Duration::from_secs(2)))
                                .unwrap();
                            let mut input = BufReader::new(socket.try_clone().unwrap());
                            let mut line = String::new();
                            input.read_line(&mut line).unwrap();
                            assert!(
                                line.starts_with("GET /connections HTTP/1.1"),
                                "monitor must never close connections through the API"
                            );
                            let mut authorized = false;
                            loop {
                                line.clear();
                                if input.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                                    break;
                                }
                                if line
                                    .to_ascii_lowercase()
                                    .starts_with("authorization: bearer fixture-secret")
                                {
                                    authorized = true;
                                }
                            }
                            assert!(authorized);
                            let n = calls.fetch_add(1, Ordering::SeqCst);
                            let (delay, body) = response(n);
                            std::thread::sleep(delay);
                            let header = format!(
                                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                body.len()
                            );
                            let _ = socket
                                .write_all(header.as_bytes())
                                .and_then(|()| socket.write_all(&body));
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(5))
                        }
                        Err(e) => panic!("Owned metadata fixture: {e}"),
                    }
                }
            });
            Self {
                port,
                count,
                stop,
                worker: Some(worker),
            }
        }
    }
    impl Drop for Api {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Release);
            self.worker.take().unwrap().join().unwrap();
        }
    }
    fn document(count: usize) -> Vec<u8> {
        let rows:Vec<_>=(0..count).map(|n|serde_json::json!({"id":n.to_string(),"metadata":{"network":"tcp","sourceIP":"127.0.0.1","sourcePort":"1234","host":"fixture.invalid","destinationPort":"443"},"upload":1,"download":2})).collect();
        serde_json::to_vec(
            &serde_json::json!({"uploadTotal":123,"downloadTotal":456,"connections":rows}),
        )
        .unwrap()
    }
    #[test]
    #[cfg(windows)]
    #[ignore = "Pinned engine; owned loopback sockets only, no TUN or system proxy"]
    fn real_engine_releases_closed_flows_without_restarting_or_mass_closing_sessions() {
        use crate::{engine::Engine, imports, model::Preferences, runtime::free_port};
        use std::net::TcpStream;
        let origin = TcpListener::bind("127.0.0.1:0").unwrap();
        origin.set_nonblocking(true).unwrap();
        let target = origin.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let finish = stop.clone();
        let responder = std::thread::spawn(move || {
            let mut clients = Vec::new();
            while !finish.load(Ordering::Acquire) {
                match origin.accept() {
                    Ok((mut socket, _)) => {
                        let stop = finish.clone();
                        clients.push(std::thread::spawn(move || {
                            socket
                                .set_read_timeout(Some(Duration::from_millis(100)))
                                .unwrap();
                            while !stop.load(Ordering::Acquire) {
                                let mut byte = [0];
                                match socket.read(&mut byte) {
                                    Ok(0) => break,
                                    Ok(_) => {
                                        if socket.write_all(&byte).is_err() {
                                            break;
                                        }
                                    }
                                    Err(e)
                                        if matches!(
                                            e.kind(),
                                            std::io::ErrorKind::TimedOut
                                                | std::io::ErrorKind::WouldBlock
                                        ) => {}
                                    Err(_) => break,
                                }
                            }
                        }));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5))
                    }
                    Err(e) => panic!("Owned origin: {e}"),
                }
            }
            for client in clients {
                client.join().unwrap();
            }
        });
        struct OriginGuard {
            stop: Arc<AtomicBool>,
            worker: Option<std::thread::JoinHandle<()>>,
        }
        impl Drop for OriginGuard {
            fn drop(&mut self) {
                self.stop.store(true, Ordering::Release);
                let _ = self.worker.take().unwrap().join();
            }
        }
        let _origin = OriginGuard {
            stop,
            worker: Some(responder),
        };
        let port = free_port().unwrap();
        let api = free_port().unwrap();
        let profile = imports::parse_uri("socks5://127.0.0.1:9").unwrap();
        let mut config = crate::engine::configuration(
            &profile,
            &Preferences {
                proxy_port: port,
                ..Default::default()
            },
            api,
            "fixture-secret",
            "fixture-only",
        )
        .unwrap();
        config["inbounds"][0]["users"] = serde_json::json!([]);
        config["route"]["final"] = serde_json::json!("direct");
        let path = std::path::PathBuf::from(std::env::var_os("NETPULSE_TEST_ENGINE").unwrap());
        let mut engine = Engine::spawn(&path, &config, false).unwrap();
        let pid = engine.pid();
        let ready = Instant::now() + Duration::from_secs(3);
        while TcpStream::connect(("127.0.0.1", port)).is_err() {
            assert!(Instant::now() < ready);
            std::thread::sleep(Duration::from_millis(10));
        }
        let monitor = Monitor::new(api, Zeroizing::new("fixture-secret".into())).unwrap();
        let expected_count = |count, diagnostic: &str| {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                let sample = monitor.reader.fresh();
                if sample
                    .as_ref()
                    .is_ok_and(|s| s.metrics.connections == count)
                {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "expected {count} connections, got {:?}; engine: {}",
                    sample
                        .as_ref()
                        .map(|s| s.metrics.connections)
                        .map_err(|e| &e.code),
                    diagnostic
                );
                std::thread::sleep(Duration::from_millis(100));
            }
        };
        expected_count(0, &engine.diagnostic(&config));
        for _ in 0..4 {
            let mut clients = Vec::new();
            for _ in 0..32 {
                let mut socket = TcpStream::connect(("127.0.0.1", port)).unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                socket.write_all(&[5, 1, 0]).unwrap();
                let mut greeting = [0; 2];
                socket.read_exact(&mut greeting).unwrap();
                assert_eq!(greeting, [5, 0]);
                let mut request = vec![5, 1, 0, 1, 127, 0, 0, 1];
                request.extend(target.port().to_be_bytes());
                socket.write_all(&request).unwrap();
                let mut reply = [0; 10];
                socket.read_exact(&mut reply).unwrap();
                assert_eq!(reply[1], 0);
                socket.write_all(b"x").unwrap();
                let mut echoed = [0];
                socket.read_exact(&mut echoed).unwrap();
                assert_eq!(&echoed, b"x");
                clients.push(socket);
            }
            expected_count(32, &engine.diagnostic(&config));
            drop(clients);
            expected_count(0, &engine.diagnostic(&config));
            assert!(engine.alive().unwrap());
            assert_eq!(engine.pid(), pid);
        }
        drop(monitor);
        engine.stop().unwrap();
    }
    #[test]
    fn thousands_of_rows_are_counted_without_retaining_all_metadata_or_burst_polling() {
        let api = Api::new(|_| (Duration::from_millis(100), document(10000)));
        let monitor = Monitor::new(api.port, Zeroizing::new("fixture-secret".into())).unwrap();
        for _ in 0..1000 {
            monitor.reader.request();
        }
        let sample = monitor.reader.fresh().unwrap();
        assert_eq!(sample.metrics.connections, 10000);
        assert_eq!(sample.rows.len(), 300);
        for _ in 0..1000 {
            monitor.reader.request();
        }
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(api.count.load(Ordering::SeqCst), 1);
        let reader = monitor.reader.clone();
        drop(monitor);
        assert_eq!(reader.fresh().err().unwrap().code, "SessionChanged");
    }
    #[test]
    fn slow_or_failed_api_never_blocks_cached_reads_or_discards_last_good_counts() {
        let api = Api::new(|n| {
            if n == 0 {
                (Duration::ZERO, document(17))
            } else {
                (Duration::from_millis(1800), document(0))
            }
        });
        let monitor = Monitor::new(api.port, Zeroizing::new("fixture-secret".into())).unwrap();
        assert_eq!(monitor.reader.fresh().unwrap().metrics.connections, 17);
        std::thread::sleep(MIN_INTERVAL + Duration::from_millis(20));
        let reader = monitor.reader.clone();
        let pending = std::thread::spawn(move || reader.fresh());
        let deadline = Instant::now() + Duration::from_secs(1);
        while api.count.load(Ordering::SeqCst) < 2 {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        let start = Instant::now();
        for _ in 0..1000 {
            let _ = monitor.reader.cached_metrics();
        }
        assert!(start.elapsed() < Duration::from_millis(100));
        assert!(pending.join().unwrap().is_err());
        let (cached, error) = monitor.reader.cached_metrics();
        assert_eq!(cached.unwrap().connections, 17);
        assert!(error.is_some());
    }
    #[test]
    fn oversized_or_invalid_responses_are_bounded_and_do_not_become_zero_connections() {
        for body in [
            vec![b' '; LIMIT as usize + 1],
            b"{\"connections\":[]}".to_vec(),
        ] {
            let api = Api::new(move |_| (Duration::ZERO, body.clone()));
            let monitor = Monitor::new(api.port, Zeroizing::new("fixture-secret".into())).unwrap();
            assert!(monitor.reader.fresh().is_err());
            assert!(monitor.reader.cached_metrics().0.is_none());
        }
    }
}
