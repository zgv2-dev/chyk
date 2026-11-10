//! Output size for the export popover. A stream copy is its share of the source file; a
//! re-encode is measured in the background by encoding samples ([`ffmpeg::sample_rate`]).

use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::TryRecvError;

/// Settings must hold still this long before samples are encoded.
const SETTLE: Duration = Duration::from_millis(300);

/// Where the measured re-encode for the current settings stands.
pub(super) enum Rate {
    Pending,
    Failed,
    /// Output bytes per second at the current resolution.
    Known(f64),
}

/// Background sampling for the current export settings.
#[derive(Default)]
pub(super) struct Sampler {
    /// The last measured job and its bytes per second (`None`: no encoder worked).
    done: Option<(ffmpeg::Job, Option<f64>)>,
    running: Option<(ffmpeg::Job, Receiver<Option<f64>>, Arc<AtomicBool>)>,
    /// The job waiting for the settings to settle, and since when.
    waiting: Option<(ffmpeg::Job, Instant)>,
}

impl Sampler {
    /// The rate for `want`, starting sampling once the settings settle and stopping samples
    /// of settings that changed.
    pub(super) fn rate(&mut self, want: ffmpeg::Job, ctx: &egui::Context) -> Rate {
        if let Some((job, rx, stop)) = self.running.take() {
            match rx.try_recv() {
                Ok(rate) => self.done = Some((job, rate)),
                Err(TryRecvError::Disconnected) => self.done = Some((job, None)),
                Err(TryRecvError::Empty) if job == want => {
                    self.running = Some((job, rx, stop));
                    return Rate::Pending;
                }
                Err(TryRecvError::Empty) => stop.store(true, Ordering::Relaxed),
            }
        }
        if let Some((job, rate)) = &self.done
            && *job == want
        {
            return rate.map_or(Rate::Failed, Rate::Known);
        }
        let since = match &self.waiting {
            Some((job, at)) if *job == want => *at,
            _ => Instant::now(),
        };
        let left = SETTLE.saturating_sub(since.elapsed());
        if !left.is_zero() {
            self.waiting = Some((want, since));
            ctx.request_repaint_after(left);
            return Rate::Pending;
        }
        self.waiting = None;
        let (tx, rx) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let (job, flag, ctx) = (want.clone(), stop.clone(), ctx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(ffmpeg::sample_rate(&job, &flag));
            ctx.request_repaint();
        });
        self.running = Some((want, rx, stop));
        Rate::Pending
    }
}

impl App {
    /// Output size at resolution `size` (index into [`ffmpeg::SIZES`]) with the other current
    /// settings, from `rate` measured at the current resolution. `None` while unknown.
    pub(super) fn estimate(&self, size: usize, rate: &Rate) -> Option<u64> {
        let v = self.video.as_ref()?;
        let len = self.output_length()?;
        if self.stream_copy(size) {
            // Retiming keeps every frame, so a copy follows the source time kept.
            let part = len * self.speed / v.dur.max(0.001);
            return Some((v.bytes as f64 * part.min(1.0)) as u64);
        }
        let Rate::Known(rate) = rate else {
            return None;
        };
        // Other resolutions follow the measured one: bitrate grows about as pixels^0.75
        // (0.71–0.79 measured from 4K down to 144p).
        let px = |s: usize| {
            let [w, h] = self.scale(s).or(self.cropped()).unwrap_or([1, 1]);
            f64::from(w) * f64::from(h)
        };
        Some((rate * (px(size) / px(self.size)).powf(0.75) * len) as u64)
    }

    /// "≈ 1.2 GB", "≈ …" while measuring, or empty when there is nothing to show.
    pub(super) fn size_note(&self, size: usize, rate: &Rate) -> String {
        match (self.estimate(size, rate), rate) {
            (Some(bytes), _) => approx(bytes),
            (None, Rate::Pending) if self.output_length().is_some() => "≈ …".into(),
            _ => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Waits out the settle delay, starts sampling a missing file and gets `Failed`; a changed
    /// job starts over.
    #[test]
    fn sampler_settles_measures_and_restarts() {
        let ctx = egui::Context::default();
        let job = ffmpeg::Job {
            src: "missing.mp4".into(),
            dst: PathBuf::new(),
            start: 0.0,
            end: 4.0,
            crop: None,
            scale: None,
            precise: false,
            codec: "h264".into(),
            quality: 0,
            mute: false,
            speed: 1.0,
            keep: None,
        };
        let mut s = Sampler::default();
        assert!(matches!(s.rate(job.clone(), &ctx), Rate::Pending));
        assert!(s.running.is_none(), "still settling");
        std::thread::sleep(SETTLE);
        let started = Instant::now();
        while matches!(s.rate(job.clone(), &ctx), Rate::Pending) {
            assert!(started.elapsed() < Duration::from_secs(10), "no result");
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(matches!(s.rate(job.clone(), &ctx), Rate::Failed));
        let other = ffmpeg::Job { quality: 1, ..job };
        assert!(matches!(s.rate(other, &ctx), Rate::Pending));
    }
}
