//! The strip scan only reads the trimmed part (plus padding): what the trim cuts off can't matter.
//! When the trim grows, only the new part is scanned and merged in, once the handle is released.

use super::*;

/// Whether a scan of `have` answers for `need` (both source time ranges).
pub(super) fn covers(have: (f64, f64), need: (f64, f64)) -> bool {
    have.0 <= need.0 + 1e-3 && have.1 >= need.1 - 1e-3
}

impl App {
    /// Source times a scan must cover: the trim, widened by the padding on both sides, since
    /// activity just outside the trim still keeps frames inside it.
    pub(super) fn scan_need(&self) -> (f64, f64) {
        let dur = self.video.as_ref().map_or(0.0, |v| v.dur);
        let pad = self.strip.pad.max(0.0);
        let end = if dur > 0.0 {
            (self.trim.1 + pad).min(dur)
        } else {
            f64::INFINITY
        };
        ((self.trim.0 - pad).max(0.0), end)
    }

    /// Bring the scan up to the trim once the user lets go (never mid-drag):
    /// - a scan running rightward follows the trim's end: on to a later one, or it stops at once
    ///   when the end moved back into what it already read;
    /// - with nothing running, the part missing on the left, then on the right, is scanned and
    ///   merged in. Narrowing never rescans.
    pub(super) fn extend_scan(&mut self, ctx: &egui::Context) {
        if !self.strip.enabled
            || self.video.is_none()
            || self.job.is_some()
            || ctx.input(|i| i.pointer.any_down())
        {
            return;
        }
        let need = self.scan_need();
        if self.strip.held == Some(need) {
            return;
        }
        self.strip.held = None;
        if let Some(job) = &self.strip.job {
            let piece = self.strip.scan_range;
            let rightward = self
                .strip
                .scan
                .as_ref()
                .is_none_or(|s| piece.0 >= s.range.1 - 1e-3);
            if rightward && (piece.1 - need.1).abs() > 1e-3 {
                job.set_end(need.1);
                self.strip.scan_range.1 = need.1;
            }
            return;
        }
        let Some(have) = self.strip.scan.as_ref().map(|s| s.range) else {
            return;
        };
        if need.0 < have.0 - 1e-3 {
            self.scan_piece(ctx, (need.0, have.0));
        } else if need.1 > have.1 + 1e-3 {
            self.scan_piece(ctx, (have.1, need.1));
        }
    }
}
