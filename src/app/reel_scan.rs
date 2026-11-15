//! Strip on a join: each clip is scanned on its own, in timeline order, fitted into the output
//! frame at a small size so the crop and ignored areas drawn on that frame line up in every fit.
//! Results stay in the clip's own seconds: the hatching and the bar add the clip's start, the
//! export takes them as they are. Padding never crosses a seam.

use super::reel::Reel;
use super::*;

/// What a clip's scan depends on. Same file, fit, frame, crop, areas and sensitivity: the same
/// scan, wherever the clip sits and however it is trimmed.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub(super) struct ClipKey {
    path: PathBuf,
    fit: ffmpeg::Fit,
    frame: [u32; 2],
    crop: Option<[u32; 4]>,
    zones: Vec<[u32; 4]>,
    sens: crate::settings::Sensitivity,
}

fn bits(r: Rect) -> [u32; 4] {
    [r.min.x, r.min.y, r.max.x, r.max.y].map(f32::to_bits)
}

/// One stretch of the timeline where Strip's activity is known: the active intervals, and the
/// part of it that is both scanned and kept by the trim (all timeline seconds).
pub(super) type Found = (Vec<(f64, f64)>, f64, f64);

/// A running scan piece: what it found so far, its range and the line's place (timeline seconds).
pub(super) type Piece<'a> = (&'a [(f64, f64)], (f64, f64), f64);

/// What is known of one scan target, shifted onto the timeline by `off`: the finished scan, plus
/// the running piece's findings up to `front` when that piece grows it rightward.
pub(super) fn found(
    scan: Option<&strip::Scan>,
    running: Option<Piece>,
    off: f64,
    trim: (f64, f64),
) -> Option<Found> {
    let shift = |v: &[(f64, f64)]| {
        v.iter()
            .map(|&(a, b)| (a + off, b + off))
            .collect::<Vec<_>>()
    };
    let (active, lo, hi) = match (scan, running) {
        (Some(s), Some((partial, piece, front))) if piece.0 >= s.range.1 + off - 1e-3 => {
            let mut active = shift(&s.active);
            active.extend_from_slice(partial);
            (active, s.range.0 + off, front)
        }
        (Some(s), _) => (shift(&s.active), s.range.0 + off, s.range.1 + off),
        (None, Some((partial, piece, front))) => (partial.to_vec(), piece.0, front),
        (None, None) => return None,
    };
    Some((active, lo.max(trim.0), hi.min(trim.1)))
}

/// Seconds `a` and `b` share.
fn overlap(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.1.min(b.1) - a.0.max(b.0)).max(0.0)
}

impl App {
    fn clip_key(&self, r: &Reel, i: usize) -> ClipKey {
        let c = &r.clips[i];
        ClipKey {
            path: c.path.clone(),
            fit: c.fit,
            frame: r.frame,
            crop: self.strip.scan_crop.map(bits),
            zones: self.strip.zones.iter().copied().map(bits).collect(),
            sens: self.strip.sensitivity,
        }
    }

    /// Clip `i`'s source seconds a scan must cover: its trim, widened by the padding.
    fn clip_need(&self, r: &Reel, i: usize) -> (f64, f64) {
        let c = &r.clips[i];
        let pad = self.strip.pad.max(0.0);
        let end = if c.info.dur > 0.0 {
            (c.trim.1 + pad).min(c.info.dur)
        } else {
            c.trim.1 + pad
        };
        ((c.trim.0 - pad).max(0.0), end)
    }

    /// Every clip's key and need, in timeline order.
    pub(super) fn clip_needs(&self, r: &Reel) -> Vec<(ClipKey, (f64, f64))> {
        (0..r.clips.len())
            .map(|i| (self.clip_key(r, i), self.clip_need(r, i)))
            .collect()
    }

    /// The running clip scan's clip, if it still is that clip.
    fn running_clip(&self, r: &Reel) -> Option<usize> {
        let (key, i) = self.strip.clip.as_ref()?;
        (self.strip.job.is_some() && *i < r.clips.len() && self.clip_key(r, *i) == *key)
            .then_some(*i)
    }

    /// Per clip, the source intervals Strip keeps of its trim, from whatever its scan covers.
    /// `None` while a clip has no scan at all.
    pub(super) fn reel_keeps(&self, r: &Reel) -> Option<Vec<Vec<(f64, f64)>>> {
        r.clips
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let scan = self.clip_scans.get(&self.clip_key(r, i))?;
                Some(strip::keep(
                    &scan.active,
                    self.strip.pad,
                    c.trim.0,
                    c.trim.1,
                ))
            })
            .collect()
    }

    /// Whether every clip's scan covers its trim and padding (what an export needs).
    pub(super) fn reel_covered(&self, r: &Reel) -> bool {
        self.clip_needs(r).iter().all(|(key, need)| {
            self.clip_scans
                .get(key)
                .is_some_and(|s| covers(s.range, *need))
        })
    }

    /// Strip's known activity on the join, one stretch per clip, for the hatching.
    pub(super) fn reel_found(&self, r: &Reel) -> Vec<Found> {
        let front = self.strip.front.shown.min(self.strip.scanned) - self.strip.pad;
        let run = self.running_clip(r);
        (0..r.clips.len())
            .filter_map(|i| {
                let scan = self.clip_scans.get(&self.clip_key(r, i));
                let running = (run == Some(i))
                    .then(|| (&self.strip.partial[..], self.strip.scan_range, front));
                found(scan, running, r.start(i), r.kept_range(i))
            })
            .collect()
    }

    /// How far the join's scans are, 0–1: the needed seconds already scanned, over all clips.
    pub(super) fn reel_scan_share(&self, r: &Reel) -> f64 {
        let run = self.running_clip(r);
        let (mut done, mut total) = (0.0, 0.0);
        for (i, (key, need)) in self.clip_needs(r).into_iter().enumerate() {
            total += (need.1 - need.0).max(0.0);
            if let Some(s) = self.clip_scans.get(&key) {
                done += overlap(s.range, need);
            }
            // The running piece, as far as the line has drawn it, counts only inside the need:
            // an extension can start at the old scan's end, short of a trim moved past it.
            if run == Some(i) {
                let (off, (a, b)) = (r.start(i), self.strip.scan_range);
                let front = self.strip.front.shown.clamp(a, b.max(a));
                done += overlap((a - off, front - off), need);
            }
        }
        (done / total.max(1e-9)).clamp(0.0, 1.0)
    }

    /// Start the next clip scan Strip needs on the join, in timeline order: a clip with no scan,
    /// or the part its trim and padding grew past. A running scan of a clip that moved, changed
    /// fit, or whose crop or areas changed is cancelled; one growing rightward follows the trim.
    pub(super) fn reel_scan(&mut self, ctx: &egui::Context) {
        // Zone editing keeps scans off, as on a single video.
        if self.strip.edit || self.loading.is_some() {
            return;
        }
        let Some(r) = self.reel_now() else { return };
        let needs = self.clip_needs(&r);
        if self.strip.job.is_some() {
            let Some(i) = self.running_clip(&r) else {
                self.cancel_scan();
                return self.reel_scan(ctx);
            };
            let (off, need) = (r.start(i), needs[i].1);
            let piece = self.strip.scan_range;
            let rightward = self
                .clip_scans
                .get(&needs[i].0)
                .is_none_or(|s| piece.0 >= s.range.1 + off - 1e-3);
            if rightward
                && (piece.1 - off - need.1).abs() > 1e-3
                && let Some(job) = &self.strip.job
            {
                job.set_end(need.1);
                self.strip.scan_range.1 = need.1 + off;
            }
            return;
        }
        if self.strip.held_reel.as_ref() == Some(&needs) {
            return;
        }
        self.strip.held_reel = None;
        for (i, (key, need)) in needs.into_iter().enumerate() {
            let piece = match self.clip_scans.get(&key).map(|s| s.range) {
                None => need,
                Some(have) if need.0 < have.0 - 1e-3 => (need.0, have.0),
                Some(have) if need.1 > have.1 + 1e-3 => (have.1, need.1),
                Some(_) => continue,
            };
            return self.scan_clip(ctx, &r, i, key, piece);
        }
    }

    /// Scan `piece` (source seconds) of clip `i` in the background.
    fn scan_clip(
        &mut self,
        ctx: &egui::Context,
        r: &Reel,
        i: usize,
        key: ClipKey,
        piece: (f64, f64),
    ) {
        let c = &r.clips[i];
        let [fw, fh] = r.frame;
        let crop = self.strip.scan_crop;
        let share = crop.map_or(1.0, |c| f64::from(c.width() * c.height()).max(1e-4));
        // About twice the compared size across the crop, so the scan's last scale only shrinks.
        let k = 2.0 * (f64::from(strip::SAMPLE_AREA) / (f64::from(fw * fh) * share)).sqrt();
        let (pre, size) = ffmpeg::scan_fit(c.fit, r.frame, k);
        let area = strip::Area {
            aspect: fw as f32 / fh.max(1) as f32,
            size,
            crop,
            zones: self.strip.zones.clone(),
            pre,
            range: Some(piece),
            thresholds: super::settings_apply::strip_thresholds(self.strip.sensitivity),
        };
        let off = r.start(i);
        self.strip.clip = Some((key, i));
        self.strip.scan_range = (piece.0 + off, piece.1 + off);
        self.strip.progress = 0.0;
        self.strip.partial.clear();
        self.strip.scanned = piece.0 + off;
        self.strip.front = Glide {
            shown: piece.0 + off,
            ..Default::default()
        };
        self.status = None;
        let (src, dur) = (c.path.clone(), c.info.dur);
        self.strip.job = Some(strip::scan(src, dur, area, ctx.clone()));
    }

    /// Scan every clip again from scratch (the Rescan button on a join).
    pub(super) fn rescan_reel(&mut self, ctx: &egui::Context) {
        let Some(r) = self.reel_now() else { return };
        self.cancel_scan();
        for (key, _) in self.clip_needs(&r) {
            self.clip_scans.remove(&key);
        }
        self.strip.held_reel = None;
        self.strip.scan_crop = self.crop;
        ctx.request_repaint();
    }

    /// Don't start the scans Strip needs again by themselves until the trim, padding or clips
    /// change (after a cancel or a failure).
    pub(super) fn hold_scan(&mut self) {
        match self.reel_now() {
            Some(r) => self.strip.held_reel = Some(self.clip_needs(&r)),
            None => self.strip.held = Some(self.scan_need()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(active: Vec<(f64, f64)>, range: (f64, f64)) -> strip::Scan {
        strip::Scan { active, range }
    }

    #[test]
    fn a_clips_scan_moves_to_its_place_and_stays_inside_its_trim() {
        // Clip 2 starts at 10 s and keeps 2–8 s of itself: 12–18 s on the timeline.
        let s = scan(vec![(1.0, 3.0), (7.0, 9.0)], (0.0, 10.0));
        let (active, lo, hi) = found(Some(&s), None, 10.0, (12.0, 18.0)).unwrap();
        assert_eq!(active, vec![(11.0, 13.0), (17.0, 19.0)]);
        assert_eq!((lo, hi), (12.0, 18.0));
    }

    #[test]
    fn only_scanned_seconds_inside_the_need_count() {
        // A trim moved to 20–30 past a scan of 0–10: the extension reads 10–30, and its first
        // ten seconds are outside what the clip needs.
        assert_eq!(overlap((10.0, 20.0), (20.0, 30.0)), 0.0);
        assert_eq!(overlap((10.0, 25.0), (20.0, 30.0)), 5.0);
        assert_eq!(overlap((0.0, 10.0), (20.0, 30.0)), 0.0);
    }

    #[test]
    fn a_rightward_piece_adds_what_it_found_up_to_the_line() {
        let s = scan(vec![(1.0, 2.0)], (0.0, 4.0));
        let partial = [(15.0, 15.5)];
        let running = Some((&partial[..], (14.0, 20.0), 16.0));
        let (active, lo, hi) = found(Some(&s), running, 10.0, (10.0, 20.0)).unwrap();
        assert_eq!(active, vec![(11.0, 12.0), (15.0, 15.5)]);
        assert_eq!((lo, hi), (10.0, 16.0));
        // A piece on the left doesn't extend what is known rightward.
        let left = Some((&partial[..], (9.0, 10.0), 9.5));
        assert_eq!(found(Some(&s), left, 10.0, (9.0, 20.0)).unwrap().2, 14.0);
        // Nothing scanned, nothing running: nothing known.
        assert!(found(None, None, 10.0, (10.0, 20.0)).is_none());
    }
}
