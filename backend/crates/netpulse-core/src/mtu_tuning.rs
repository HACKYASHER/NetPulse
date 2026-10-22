//! Sequential MTU comparison over the current TUN, never physical-path DF probing.
//! Latency/loss are measured; these small probes do not measure download speed.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{net::{Ipv4Addr, SocketAddr}, sync::atomic::{AtomicBool, Ordering}, time::{Duration, Instant}};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Request {
    pub target: String,
    pub method: String,
    pub minimum: u16,
    pub maximum: u16,
    pub samples: u8,
}
impl Default for Request {
    fn default() -> Self {
        Self { target: "https://www.gstatic.com/generate_204".into(), method: "https".into(), minimum: 1440, maximum: 1472, samples: 5 }
    }
}
impl Request {
    pub fn validate(&self) -> Result<()> {
        if !(1280..=1500).contains(&self.minimum) || self.maximum < self.minimum || self.maximum > 1500
            || self.maximum - self.minimum > 64 || !(1..=5).contains(&self.samples) {
            return Err(Error::new("MtuRange", "mtu-tuning", "Диапазон MTU: 1280–1500, до 65 значений; 1–5 проверок на значение"));
        }
        if !["https", "icmp"].contains(&self.method.as_str()) {
            return Err(Error::new("MtuMethod", "mtu-tuning", "Выберите HTTPS или ICMP"));
        }
        self.endpoint()?;
        Ok(())
    }
    pub fn endpoint(&self) -> Result<url::Url> {
        let target = self.target.trim();
        let invalid = || Error::new("MtuTarget", "mtu-tuning", "Укажите имя сайта, IPv4 или HTTPS URL без пароля и фрагмента");
        if target.is_empty() || target.len() > 2048 || self.target.chars().any(char::is_control) || target.chars().any(char::is_whitespace) { return Err(invalid()); }
        let value = if target.contains("://") { target.to_owned() } else {
            if !target.bytes().all(|b| b.is_ascii_alphanumeric() || b".-".contains(&b)) { return Err(invalid()); }
            format!("https://{target}/generate_204")
        };
        let endpoint = url::Url::parse(&value).map_err(|_| invalid())?;
        if endpoint.scheme() != "https" || endpoint.host_str().is_none() || !endpoint.username().is_empty()
            || endpoint.password().is_some() || endpoint.fragment().is_some() { return Err(invalid()); }
        if self.method == "icmp" && target.contains("://") { return Err(Error::new("MtuTarget", "mtu-tuning", "Для ICMP укажите IPv4 или имя сайта без https:// и пути")); }
        Ok(endpoint)
    }
    pub fn candidates(&self) -> Vec<u16> { (self.minimum..=self.maximum).collect() }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Trial {
    pub mtu: u16,
    pub phase: String,
    pub samples_ms: Vec<f64>,
    pub successes: usize,
    pub failures: usize,
    pub loss_percent: f64,
    pub median_ms: Option<f64>,
    pub jitter_ms: Option<f64>,
    pub error: Option<String>,
}
fn median(values: &[f64]) -> Option<f64> {
    let mut values: Vec<_> = values.iter().copied().filter(|n| n.is_finite() && *n >= 0.0).collect();
    if values.is_empty() { return None; }
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    Some(if values.len() % 2 == 0 { (values[middle-1] + values[middle])/2.0 } else { values[middle] })
}
impl Trial {
    pub fn from_samples(mtu:u16, phase:&str, samples_ms:Vec<f64>, attempts:usize, error:Option<String>) -> Self {
        let samples_ms: Vec<_> = samples_ms.into_iter().filter(|n| n.is_finite() && *n >= 0.0).take(attempts).collect();
        let successes = samples_ms.len();
        let failures = attempts.saturating_sub(successes);
        let differences: Vec<_> = samples_ms.windows(2).map(|pair| (pair[1]-pair[0]).abs()).collect();
        Self { mtu, phase:phase.into(), median_ms:median(&samples_ms), jitter_ms:median(&differences),
            samples_ms, successes, failures, loss_percent:if attempts == 0 { 100.0 } else { failures as f64 * 100.0 / attempts as f64 }, error }
    }
    fn reliable(&self) -> bool { self.successes >= 3 && self.median_ms.is_some() && self.loss_percent <= 40.0 }
    fn cost(&self) -> f64 { self.median_ms.unwrap_or(f64::INFINITY) + self.jitter_ms.unwrap_or(0.0) * 0.5 }
}
#[derive(Serialize)]
pub struct Comparison {
    pub suggested_mtu: Option<u16>,
    pub baseline_mtu: u16,
    pub reason: String,
    pub baseline_stable: bool,
    pub method: String,
    pub target: String,
    pub minimum: u16,
    pub maximum: u16,
    pub samples: u8,
    pub throughput_measured: bool,
}
pub fn compare(request:&Request, trials:&[Trial], baseline:&Trial, control:&Trial) -> Comparison {
    let mut report = Comparison { suggested_mtu:None, baseline_mtu:baseline.mtu,
        reason:"Заметного устойчивого улучшения не найдено. Сохраните исходный MTU.".into(), baseline_stable:false,
        method:request.method.clone(),target:request.target.clone(),minimum:request.minimum,maximum:request.maximum,samples:request.samples,throughput_measured:false };
    if !baseline.reliable() || !control.reliable() {
        report.reason="Недостаточно ответов исходного MTU: нужно не менее трёх успешных проверок в начале и конце. Рекомендации нет.".into();
        return report;
    }
    let a = baseline.median_ms.unwrap(); let b = control.median_ms.unwrap();
    if (a-b).abs() > (a.min(b) * 0.25).max(3.0) || (baseline.loss_percent-control.loss_percent).abs() > 20.01 {
        report.reason="Задержка или потери исходного MTU заметно изменились во время теста. Повторите тест при стабильной нагрузке.".into();
        return report;
    }
    report.baseline_stable = true;
    let reference_cost=baseline.cost().min(control.cost());
    let reference_loss=baseline.loss_percent.min(control.loss_percent);
    let mut best: Option<&Trial> = None;
    for trial in trials.iter().filter(|t| t.reliable() && t.mtu != baseline.mtu) {
        // A whole lost probe dominates small timing fluctuations. Never exchange
        // more packet loss for a superficially faster successful response.
        let improved = trial.loss_percent + 0.01 < reference_loss
            || ((trial.loss_percent-reference_loss).abs() < 0.01
                && reference_cost-trial.cost() >= (reference_cost*0.05).max(2.0));
        if !improved { continue; }
        let replace=best.is_none_or(|current| trial.loss_percent + 0.01 < current.loss_percent
            || ((trial.loss_percent-current.loss_percent).abs()<0.01
                && (trial.cost() + 1.0 < current.cost()
                    || ((trial.cost()-current.cost()).abs()<=1.0 && trial.mtu>current.mtu))));
        if replace { best=Some(trial); }
    }
    if let Some(best)=best {
        report.suggested_mtu=Some(best.mtu);
        report.reason="Лучший результат проверенного диапазона по потерям, медиане задержки и джиттеру. Исходный MTU восстановлен; примените рекомендацию вручную. Скорость загрузки этим тестом не измеряется.".into();
    }
    report
}

/// Reject a lucky winner that does not repeat under the same baseline.
pub fn confirm(mut comparison:Comparison,request:&Request,trials:&[Trial],baseline:&Trial,control:&Trial,verify:&Trial)->Comparison {
    if let Some(mtu)=comparison.suggested_mtu {
        let original=trials.iter().find(|trial|trial.mtu==mtu);
        let repeated=compare(request,std::slice::from_ref(verify),baseline,control);
        let consistent=verify.mtu==mtu && repeated.suggested_mtu==Some(mtu) && original.is_some_and(|original| {
            (original.cost()-verify.cost()).abs() <= (original.cost().min(verify.cost())*0.25).max(3.0)
                && (original.loss_percent-verify.loss_percent).abs()<=20.01
        });
        if !consistent {
            comparison.suggested_mtu=None;
            comparison.reason="Лучший кандидат не подтвердился при повторной проверке. Сохраните исходный MTU и повторите тест при стабильной нагрузке.".into();
        }
    }
    comparison
}

/// Resolve once, then bind every sample to the owned IPv4 TUN source. The
/// route is checked again after every runtime restart and before each request.
pub struct Probe {
    endpoint: url::Url,
    address: Ipv4Addr,
    method: String,
}
impl Probe {
    pub fn new(request:&Request) -> Result<Self> {
        request.validate()?;
        let endpoint=request.endpoint()?;
        let host=endpoint.host_str().unwrap().to_owned();
        let port=endpoint.port_or_known_default().unwrap_or(443);
        let runtime=tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e|Error::io("mtu-dns",e))?;
        let address=runtime.block_on(async {
            let addresses=tokio::time::timeout(Duration::from_secs(4),tokio::net::lookup_host((host.as_str(),port))).await
                .map_err(|_|Error::new("MtuDnsTimeout","mtu-dns","Сайт теста не разрешился за 4 секунды"))?
                .map_err(|e|Error::io("mtu-dns",e))?;
            addresses.filter_map(|a|match a.ip(){std::net::IpAddr::V4(ip)=>Some(ip),_=>None}).next()
                .ok_or_else(||Error::new("MtuTarget","mtu-dns","У сайта нет IPv4 для TUN теста"))
        })?;
        Ok(Self{endpoint,address,method:request.method.clone()})
    }
    pub fn address(&self) -> Ipv4Addr { self.address }
    pub fn sample(&self, cancel:&AtomicBool) -> Result<f64> {
        if cancel.load(Ordering::SeqCst) { return Err(Error::new("Cancelled","mtu-tuning","Тест отменён")); }
        ensure_tun_route(self.address)?;
        if self.method=="icmp" { return tun_echo(self.address); }
        use std::io::Read;
        let address=SocketAddr::new(self.address.into(),self.endpoint.port_or_known_default().unwrap_or(443));
        let client=reqwest::blocking::Client::builder().no_proxy().https_only(true)
            .local_address(std::net::IpAddr::V4(Ipv4Addr::new(172,31,255,1)))
            .resolve(self.endpoint.host_str().unwrap(),address)
            .timeout(Duration::from_secs(4)).connect_timeout(Duration::from_secs(3))
            .redirect(reqwest::redirect::Policy::none()).pool_max_idle_per_host(0)
            .build().map_err(|_|Error::new("MtuHttps","mtu-tuning","Не удалось создать HTTPS тест"))?;
        let started=Instant::now();
        let response=client.get(self.endpoint.clone()).header(reqwest::header::CONNECTION,"close")
            .send().map_err(|e|Error::new(if e.is_timeout(){"MtuTimeout"}else{"MtuHttps"},"mtu-tuning","HTTPS через TUN не получил проверенный ответ"))?;
        if !response.status().is_success() { return Err(Error::new("MtuHttpStatus","mtu-tuning",format!("HTTP {}: выберите короткий HTTPS ответ без перенаправления",response.status().as_u16()))); }
        let mut body=Vec::new(); response.take(65537).read_to_end(&mut body).map_err(|e|Error::io("mtu-response",e))?;
        if body.len()>65536 { return Err(Error::new("MtuResponseLimit","mtu-tuning","Ответ больше 64 КБ; выберите /generate_204")); }
        Ok(started.elapsed().as_secs_f64()*1000.0)
    }
}
#[cfg(windows)]
fn ensure_tun_route(target:Ipv4Addr) -> Result<()> {
    use windows_sys::Win32::{NetworkManagement::IpHelper::GetBestInterfaceEx,Networking::WinSock::*};
    let mut address=SOCKADDR_IN{sin_family:AF_INET,..Default::default()};
    address.sin_addr.S_un.S_addr=u32::from_ne_bytes(target.octets());
    let mut index=0;
    let code=unsafe{GetBestInterfaceEx((&address as *const SOCKADDR_IN).cast(),&mut index)};
    let owned=code==0 && crate::routes::inspect()?.interfaces.iter().any(|i|i.index==index&&i.up&&i.alias.eq_ignore_ascii_case("NetPulse"));
    if !owned { return Err(Error::new("MtuRouteChanged","mtu-tuning","Маршрут тестового сайта не проходит через активный TUN NetPulse. Проверьте режим подключения и исключения.")); }
    Ok(())
}
#[cfg(not(windows))]
fn ensure_tun_route(_:Ipv4Addr)->Result<()> { Err(Error::new("PlatformUnsupported","mtu-tuning","TUN тест требует Windows")) }
#[cfg(windows)]
fn tun_echo(target:Ipv4Addr)->Result<f64> {
    use windows_sys::Win32::{Foundation::INVALID_HANDLE_VALUE,NetworkManagement::IpHelper::*};
    let handle=unsafe{IcmpCreateFile()};
    if handle==INVALID_HANDLE_VALUE||handle.is_null(){return Err(Error::io("mtu-icmp",std::io::Error::last_os_error()));}
    // Small echoes compare loss/RTT at each configured adapter MTU. This is
    // deliberately not a physical PMTU/DF search or a throughput measurement.
    let payload=*b"NetPulse-MTU";
    let mut buffer=[0u64;64];
    let count=unsafe{IcmpSendEcho2Ex(handle,std::ptr::null_mut(),None,std::ptr::null(),
        u32::from_ne_bytes([172,31,255,1]),u32::from_ne_bytes(target.octets()),
        payload.as_ptr().cast(),payload.len() as u16,std::ptr::null(),buffer.as_mut_ptr().cast(),std::mem::size_of_val(&buffer) as u32,1200)};
    let status=if count>0 {unsafe{(*(buffer.as_ptr() as *const ICMP_ECHO_REPLY)).Status}} else {std::io::Error::last_os_error().raw_os_error().unwrap_or_default() as u32};
    let time=if count>0 {unsafe{(*(buffer.as_ptr() as *const ICMP_ECHO_REPLY)).RoundTripTime}} else {0};
    unsafe{IcmpCloseHandle(handle)};
    if status!=0||count==0{return Err(Error::new("MtuIcmpUnavailable","mtu-tuning",format!("ICMP не ответил ({status}). VLESS/Hysteria2/SOCKS не передают ICMP; выберите HTTPS.")));}
    Ok(time as f64)
}
#[cfg(not(windows))]
fn tun_echo(_:Ipv4Addr)->Result<f64>{Err(Error::new("PlatformUnsupported","mtu-tuning","ICMP требует Windows"))}
#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Recovery { original:crate::model::Preferences,expected:crate::model::Preferences }
pub fn save_recovery(directory:&std::path::Path,original:&crate::model::Preferences,expected:&crate::model::Preferences)->crate::Result<()> {
 use std::io::Write;
 let bytes=serde_json::to_vec(&Recovery{original:original.clone(),expected:expected.clone()}).map_err(|_|crate::Error::new("MtuRecovery","mtu-tuning","Recovery serialization failed"))?;
 let pending=directory.join("mtu-tuning-recovery.tmp");
 let mut file=std::fs::File::create(&pending).map_err(|e|crate::Error::io("mtu-recovery",e))?;
 file.write_all(&bytes).and_then(|_|file.sync_all()).map_err(|e|crate::Error::io("mtu-recovery",e))?;
 drop(file);std::fs::rename(pending,directory.join("mtu-tuning-recovery.json")).map_err(|e|crate::Error::io("mtu-recovery",e))
}
pub fn recover(store:&crate::store::Store)->crate::Result<()> {
 let path=store.data_directory().join("mtu-tuning-recovery.json");
 if !path.exists(){return Ok(())}
 let bytes=std::fs::read(&path).map_err(|e|crate::Error::io("mtu-recovery",e))?;
 if bytes.len()>2*1024*1024{return Err(crate::Error::new("MtuRecovery","mtu-recovery","Recovery document too large"))}
 let record:Recovery=serde_json::from_slice(&bytes).map_err(|_|crate::Error::new("MtuRecovery","mtu-recovery","Invalid recovery document"))?;
 record.original.validate()?;record.expected.validate()?;
 let mut current=store.preferences()?;
 if current.mtu==record.expected.mtu&&current.network_preset==record.expected.network_preset {
  current.mtu=record.original.mtu;current.network_preset=record.original.network_preset;store.save_preferences(&current)?;
 }
 std::fs::remove_file(path).map_err(|e|crate::Error::io("mtu-recovery",e))
}
#[cfg(test)] mod tests {
    use super::*;
    fn trial(mtu:u16, samples:&[f64], attempts:usize)->Trial { Trial::from_samples(mtu,"candidate",samples.to_vec(),attempts,None) }
    #[test] fn sequential_range_and_bounds_are_explicit() {
        let r=Request::default(); r.validate().unwrap();
        assert_eq!(r.candidates(),(1440..=1472).collect::<Vec<_>>());
        for r in [Request{minimum:1473,maximum:1472,..Request::default()},Request{maximum:1550,..Request::default()},Request{samples:6,..Request::default()},Request{minimum:1280,maximum:1500,..Request::default()}] { assert!(r.validate().is_err()); }
        for target in ["https://user:pass@example.com/x","http://example.com","https://example.com/#x","localhost/path","example.com\n"] {
            let r=Request{target:target.into(),..Request::default()};
            assert!(r.validate().is_err(),"{target}");
        }
        assert!(Request{target:"google.com".into(),..r}.validate().is_ok());
    }
    #[test] fn loss_and_robust_latency_choose_recommendation_without_claiming_speed() {
        let r=Request::default();let baseline=trial(1500,&[50.,51.,49.,50.,50.],5);
        let faster=trial(1450,&[40.,41.,39.,40.,40.],5);
        let lossy=trial(1440,&[20.,20.,20.],5);
        let report=compare(&r,&[faster,lossy],&baseline,&baseline);
        assert_eq!(report.suggested_mtu,Some(1450));assert!(!report.throughput_measured);
        let outlier=trial(1451,&[49.,50.,4000.,49.,50.],5);
        assert!(compare(&r,&[outlier],&baseline,&baseline).suggested_mtu.is_none());
    }
    #[test] fn changed_network_failed_samples_or_noise_do_not_claim_best_mtu() {
        let r=Request::default();let baseline=trial(1500,&[50.;5],5);
        let faster=trial(1450,&[40.;5],5);
        assert!(compare(&r,&[faster.clone()],&baseline,&trial(1500,&[30.;5],5)).suggested_mtu.is_none());
        assert!(compare(&r,&[trial(1450,&[49.;5],5)],&baseline,&baseline).suggested_mtu.is_none());
        assert!(compare(&r,&[trial(1450,&[1.,1.],5)],&baseline,&baseline).suggested_mtu.is_none());
        assert!(compare(&r,&[faster],&trial(1500,&[],5),&baseline).suggested_mtu.is_none());
        let trial=trial(1450,&[f64::NAN,40.,f64::INFINITY],5);assert_eq!(trial.successes,1);assert_eq!(trial.loss_percent,80.);
    }
    #[test] fn recommendation_must_repeat_before_it_can_be_applied() {
        let r=Request::default();let base=trial(1500,&[50.;5],5);let winner=trial(1450,&[40.;5],5);
        let proposal=compare(&r,std::slice::from_ref(&winner),&base,&base);
        assert_eq!(confirm(proposal,&r,std::slice::from_ref(&winner),&base,&base,&trial(1450,&[41.;5],5)).suggested_mtu,Some(1450));
        let proposal=compare(&r,std::slice::from_ref(&winner),&base,&base);
        assert!(confirm(proposal,&r,std::slice::from_ref(&winner),&base,&base,&trial(1450,&[55.;5],5)).suggested_mtu.is_none());
    }
    #[test] fn crash_recovery_restores_only_owned_mtu_and_preserves_user_settings() {
        let d=tempfile::tempdir().unwrap();let store=crate::store::Store::open(&d.path().join("test.db")).unwrap();
        let original=crate::model::Preferences::default();let mut expected=original.clone();expected.mtu=1440;
        save_recovery(d.path(),&original,&expected).unwrap();expected.theme="light".into();store.save_preferences(&expected).unwrap();
        recover(&store).unwrap();let restored=store.preferences().unwrap();assert_eq!(restored.mtu,original.mtu);assert_eq!(restored.theme,"light");
        save_recovery(d.path(),&original,&expected).unwrap();let mut user=expected.clone();user.mtu=1459;store.save_preferences(&user).unwrap();
        recover(&store).unwrap();assert_eq!(store.preferences().unwrap().mtu,1459);
    }
}
