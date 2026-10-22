//! Bounded comparison of measured candidates; no claim of a universal best MTU.
use serde::{Serialize,Deserialize};
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct Trial {
 pub mtu:u16,pub phase:String,pub download_mbps:Option<f64>,pub latency_ms:Option<f64>,pub jitter_ms:Option<f64>,pub loaded_ms:Option<f64>,pub bytes:u64,pub error:Option<String>,
}
impl Trial { fn valid(&self)->bool {self.error.is_none()&&self.bytes>=512*1024&&self.download_mbps.is_some_and(|n|n.is_finite()&&n>0.0)&&self.latency_ms.is_some_and(|n|n.is_finite()&&n>0.0)} }
#[derive(Serialize)]
pub struct Comparison {pub suggested_mtu:Option<u16>,pub baseline_mtu:u16,pub goal:String,pub reason:String,pub baseline_stable:bool}
pub fn candidates(current:u16,ceiling:u16)->Vec<u16> {
 let mut values=vec![current];for n in [1280,1360,1400,1420,1460,1500] {if n<=ceiling&&!values.contains(&n){values.push(n)}}values
}
pub fn compare(trials:&[Trial],baseline:&Trial,control:&Trial,goal:&str)->Comparison {
 let mut report=Comparison{suggested_mtu:None,baseline_mtu:baseline.mtu,goal:goal.into(),reason:"No reliable improvement over the original MTU".into(),baseline_stable:false};
 if !baseline.valid()||!control.valid(){report.reason="Baseline measurement failed; keep the original MTU".into();return report}
 let original_speed=baseline.download_mbps.unwrap();let original_latency=baseline.latency_ms.unwrap();
 if (control.download_mbps.unwrap()/original_speed-1.0).abs()>0.25 || (control.latency_ms.unwrap()/original_latency-1.0).abs()>0.25 {report.reason="The connection changed during the test; repeat before choosing an MTU".into();return report}
 // Compare against the better original-MTU sample in each metric. A gradual
 // improvement in the network itself must not be credited to a different MTU.
 let bs=original_speed.max(control.download_mbps.unwrap());let bl=original_latency.min(control.latency_ms.unwrap());
 let base_loaded=[baseline.loaded_ms,control.loaded_ms].into_iter().flatten().filter(|n|n.is_finite()&&*n>0.0).reduce(f64::min);
 let base_jitter=[baseline.jitter_ms,control.jitter_ms].into_iter().flatten().filter(|n|n.is_finite()&&*n>=0.0).reduce(f64::min);
 report.baseline_stable=true;
 let mut best=1.08;
 let mut seen=std::collections::HashSet::new();
 for first in trials.iter().filter(|t|t.mtu!=baseline.mtu&&t.valid()) {
  if !seen.insert(first.mtu){continue}
  let repeats:Vec<_>=trials.iter().filter(|t|t.mtu==first.mtu&&t.valid()).collect();
  if repeats.len()<2 {continue}
  let min_speed=repeats.iter().map(|t|t.download_mbps.unwrap()).fold(f64::INFINITY,f64::min);
  let max_speed=repeats.iter().map(|t|t.download_mbps.unwrap()).fold(0.0,f64::max);
  let min_latency=repeats.iter().map(|t|t.latency_ms.unwrap()).fold(f64::INFINITY,f64::min);
  let max_latency=repeats.iter().map(|t|t.latency_ms.unwrap()).fold(0.0,f64::max);
  if max_speed/min_speed>1.25||max_latency/min_latency>1.25{continue}
  let mut t=first.clone();t.download_mbps=Some(repeats.iter().map(|t|t.download_mbps.unwrap()).sum::<f64>()/repeats.len() as f64);t.latency_ms=Some(repeats.iter().map(|t|t.latency_ms.unwrap()).sum::<f64>()/repeats.len() as f64);
  let loaded:Vec<_>=repeats.iter().filter_map(|t|t.loaded_ms).filter(|n|n.is_finite()&&*n>0.0).collect();
  let jitter:Vec<_>=repeats.iter().filter_map(|t|t.jitter_ms).filter(|n|n.is_finite()&&*n>=0.0).collect();
  t.loaded_ms=(!loaded.is_empty()).then(||loaded.iter().sum::<f64>()/loaded.len() as f64);t.jitter_ms=(!jitter.is_empty()).then(||jitter.iter().sum::<f64>()/jitter.len() as f64);
  let speed=t.download_mbps.unwrap()/bs;let latency=bl/t.latency_ms.unwrap();
  if speed<if goal=="latency"{0.85}else{0.90} || latency<0.87 {continue}
  let loaded=match(base_loaded,t.loaded_ms){(Some(a),Some(b))if a>0.0&&b>0.0=>a/b,_=>1.0};
  let jitter=match(base_jitter,t.jitter_ms){(Some(a),Some(b))=> (a+1.0)/(b+1.0),_=>1.0};
  let score=if goal=="latency"{0.20*speed+0.55*latency+0.20*loaded.min(3.0)+0.05*jitter.min(3.0)}else{0.55*speed+0.30*latency+0.10*loaded.min(3.0)+0.05*jitter.min(3.0)};
  if score>best {best=score;report.suggested_mtu=Some(t.mtu)}
 }
 if let Some(mtu)=report.suggested_mtu {
  let values:Vec<_>=trials.iter().filter(|t|t.mtu==mtu&&t.valid()).collect();
  if values.len()<2 {report.suggested_mtu=None;report.reason="The leading candidate needs a repeated measurement".into();return report}
  let a=values[0];let b=values[values.len()-1];
  if (a.download_mbps.unwrap()/b.download_mbps.unwrap()-1.0).abs()>0.25 || (a.latency_ms.unwrap()/b.latency_ms.unwrap()-1.0).abs()>0.25 {report.suggested_mtu=None;report.reason="Repeated candidate measurements were inconsistent".into();return report}
  report.reason="Best repeated result in this test; apply manually and verify your applications".into();
 }
 report
}

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
 fn t(mtu:u16,speed:f64,latency:f64)->Trial{Trial{mtu,phase:"test".into(),download_mbps:Some(speed),latency_ms:Some(latency),jitter_ms:Some(2.0),loaded_ms:Some(latency+5.0),bytes:4*1024*1024,error:None}}
 #[test]fn crash_recovery_restores_only_mtu_and_preserves_user_settings(){let d=tempfile::tempdir().unwrap();let store=crate::store::Store::open(&d.path().join("test.db")).unwrap();let original=crate::model::Preferences::default();let mut expected=original.clone();expected.mtu=1280;expected.network_preset="custom".into();save_recovery(d.path(),&original,&expected).unwrap();expected.theme="light".into();store.save_preferences(&expected).unwrap();recover(&store).unwrap();let restored=store.preferences().unwrap();assert_eq!(restored.mtu,original.mtu);assert_eq!(restored.theme,"light");assert!(!d.path().join("mtu-tuning-recovery.json").exists());}
 #[test]fn requires_repetition_and_stable_control(){let b=t(1400,100.0,50.0);let good=t(1360,120.0,40.0);assert!(compare(&[good.clone()],&b,&b,"balanced").suggested_mtu.is_none());assert_eq!(compare(&[good.clone(),good.clone()],&b,&b,"balanced").suggested_mtu,Some(1360));assert!(compare(&[good.clone(),good],&b,&t(1400,45.0,90.0),"balanced").suggested_mtu.is_none());}
 #[test]fn noise_or_failures_do_not_choose_a_false_winner(){let b=t(1400,100.0,50.0);let noise=t(1420,102.0,49.0);assert!(compare(&[noise.clone(),noise],&b,&b,"balanced").suggested_mtu.is_none());let mut bad=t(1280,500.0,10.0);bad.error=Some("timeout".into());assert!(compare(&[bad.clone(),bad],&b,&b,"balanced").suggested_mtu.is_none());assert!(!candidates(1400,1420).contains(&1500));}
 #[test]fn a_network_wide_improvement_is_not_attributed_to_mtu(){let mut original=t(1400,100.0,50.0);original.loaded_ms=Some(150.0);original.jitter_ms=Some(30.0);let control=t(1400,124.0,40.0);let candidate=t(1360,125.0,40.0);for goal in ["balanced","latency"]{assert!(compare(&[candidate.clone(),candidate.clone()],&original,&control,goal).suggested_mtu.is_none());}}
}
