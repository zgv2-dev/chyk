use super::*;

/// A progress value drawn smoothly between periodic reports (the scan line, progress bars). In
/// between reports it moves at the measured speed, but never more than one report interval
/// (at least [`Glide::LEAD`]) past the last report, so a slowdown makes it wait rather than run
/// ahead, and it never moves back.
#[derive(Default)]
pub(super) struct Glide {
    pub(super) shown: f64,
    /// Units per second, smoothed.
    pub(super) rate: f64,
    /// Seconds between reports, smoothed.
    pub(super) interval: f64,
    /// Last report: when, and the value.
    pub(super) report: Option<(Instant, f64)>,
    pub(super) frame: Option<Instant>,
}

impl Glide {
    pub(super) const LEAD: f64 = 0.2;

    pub(super) fn reported(&mut self, value: f64) {
        let now = Instant::now();
        if let Some((t, s)) = self.report {
            let dt = (now - t).as_secs_f64();
            if dt > 0.02 {
                let r = ((value - s) / dt).max(0.0);
                let ema = |old: f64, new: f64| {
                    if old == 0.0 {
                        new
                    } else {
                        old * 0.7 + new * 0.3
                    }
                };
                self.rate = ema(self.rate, r);
                self.interval = ema(self.interval, dt);
            }
        }
        self.report = Some((now, value));
    }

    /// Advance for this frame and return the position to draw.
    pub(super) fn tick(&mut self) -> f64 {
        let now = Instant::now();
        let dt = self.frame.map_or(0.0, |t| (now - t).as_secs_f64());
        self.frame = Some(now);
        if let Some((t, s)) = self.report {
            let lead = Self::LEAD.max(self.interval);
            let ahead = (self.rate * (now - t).as_secs_f64()).min(self.rate * lead);
            let target = s + ahead;
            if target > self.shown {
                // Eased, so speed changes and late reports blend in instead of jumping.
                self.shown += (target - self.shown) * (1.0 - (-12.0 * dt).exp());
            }
        }
        self.shown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_front_glides_but_never_runs_ahead_or_back() {
        let mut f = Glide::default();
        f.reported(0.0);
        std::thread::sleep(Duration::from_millis(100));
        f.reported(10.0); // 100 s of video per second
        let mut last = f.tick();
        // No more reports: the scan stalled. The line keeps easing forward, but stops within
        // `LEAD` seconds of scanning past the last report.
        for _ in 0..30 {
            std::thread::sleep(Duration::from_millis(10));
            let x = f.tick();
            assert!(x >= last, "went back: {x} < {last}");
            last = x;
        }
        assert!(
            last > 10.0 && last <= 10.0 + 100.0 * Glide::LEAD + 1.0,
            "{last}"
        );
    }
}
