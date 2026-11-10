//! Output size, measured by encoding a few short samples with the export's own settings: what
//! an encoder makes of a video depends on its content far more than on any formula.

use super::*;

/// Samples spread over the kept time, each this many seconds of source. Each jump between
/// samples costs the encoder like a scene cut, so fewer longer samples guess closer.
const SAMPLES: usize = 4;
const SAMPLE_LEN: f64 = 2.5;

/// `n` windows of `len` seconds centered at even steps through the kept time of `keep`, each
/// clipped to its interval. All of `keep` when it is no longer than the samples.
pub(super) fn sample_windows(keep: &[(f64, f64)], n: usize, len: f64) -> Vec<(f64, f64)> {
    let span = |&(a, b): &(f64, f64)| (b - a).max(0.0);
    let total: f64 = keep.iter().map(span).sum();
    if total <= n as f64 * len {
        return keep.to_vec();
    }
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        // Centers are total/n > len apart, so clipped windows never overlap.
        let mut t = total * (i as f64 + 0.5) / n as f64;
        for k in keep {
            if t <= span(k) {
                let mid = k.0 + t;
                let w = ((mid - len / 2.0).max(k.0), (mid + len / 2.0).min(k.1));
                if w.1 - w.0 > 0.01 {
                    out.push(w);
                }
                break;
            }
            t -= span(k);
        }
    }
    out
}

/// Output bytes per second of `job`, from samples encoded like the export (always re-encoded,
/// with the first encoder that works). `None` if none works, or once `stop` is set.
pub fn sample_rate(job: &Job, stop: &AtomicBool) -> Option<f64> {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let keep = job
        .keep
        .clone()
        .unwrap_or_else(|| vec![(job.start, job.end)]);
    let windows = sample_windows(&keep, SAMPLES, SAMPLE_LEN);
    let secs = windows.iter().map(|&(a, b)| b - a).sum::<f64>() / job.speed.max(0.001);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let dst = std::env::temp_dir().join(format!("chyk-sample-{}-{n}.mkv", std::process::id()));
    let _tmp = TempFile(dst.clone());
    let sample = Job {
        dst: dst.clone(),
        keep: Some(windows),
        ..job.clone()
    };
    let source = probe_source(&job.src);
    for enc in encoder_order(&job.codec, encoders(), job.quality == COMPRESS) {
        // A seek per sample: decoding the gaps between them would cost more than the samples.
        let max = max_inputs(job.src.as_os_str().len());
        let mut a = strip_args(&sample, &sample.keep.clone()?, enc, &source, (max, 0.0));
        let _graph = spill_graph(&mut a);
        let mut child = command("ffmpeg")
            .args(a)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let ok = loop {
            if stop.load(Ordering::Relaxed) {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            match child.try_wait() {
                Ok(Some(status)) => break status.success(),
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(20)),
                Err(_) => break false,
            }
        };
        if ok && secs > 0.0 {
            let bytes = std::fs::metadata(&dst).map_or(0, |m| m.len());
            return Some(bytes as f64 / secs);
        }
    }
    None
}
