//! Select on authenticated HTTPS availability, loss and robust latency, never TCP alone.
use crate::diagnostics::LatencyResult;
pub fn score(result: &LatencyResult, requested: usize) -> Option<f64> {
    if result.successes * 5 < requested * 3 || result.samples.is_empty() {
        return None;
    }
    let mut samples: Vec<_> = result
        .samples
        .iter()
        .copied()
        .filter(|x| x.is_finite() && *x >= 0.)
        .collect();
    if samples.is_empty() {
        return None;
    }
    samples.sort_by(f64::total_cmp);
    let median = samples[samples.len() / 2];
    let jitter = samples[samples.len() * 9 / 10] - samples[samples.len() / 10];
    let loss = 1. - result.successes.min(requested) as f64 / requested as f64;
    Some(median + jitter * 0.5 + loss * 2000.)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(samples: Vec<f64>) -> LatencyResult {
        LatencyResult {
            profile_id: "fixture".into(),
            method: "proxy_https_rtt".into(),
            protocol: "vless".into(),
            timestamp: 0,
            destination: String::new(),
            min_ms: None,
            avg_ms: None,
            max_ms: None,
            successes: samples.len(),
            timeouts: 0,
            failures: 0,
            samples,
            error: None,
            note: String::new(),
        }
    }
    #[test]
    fn loss_and_jitter_beat_a_lucky_low_ping() {
        let good = fixture(vec![100.; 10]);
        let lossy = fixture(vec![1.; 6]);
        assert!(score(&good, 10).unwrap() < score(&lossy, 10).unwrap());
        assert!(score(&fixture(vec![1.; 5]), 10).is_none());
        assert!(score(&fixture(vec![]), 10).is_none());
        assert!(score(&fixture(vec![f64::NAN; 10]), 10).is_none());
    }
}
