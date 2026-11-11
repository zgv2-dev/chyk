//! Finding the parts of a video where the picture actually changes, so the static parts can be cut.

use crate::ffmpeg;
use crate::i18n::t;
use eframe::egui::{Context, Pos2, Rect, pos2};
use std::io::{BufReader, ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

mod pixels;
mod run;
#[cfg(test)]
mod tests;

pub use pixels::keep;
use pixels::*;
use run::*;

/// Frames compared per second. 10 Hz is plenty for screen recordings and keeps hours of video cheap.
pub const FPS: f64 = 10.0;
/// Pixels per grayscale frame used for comparison: 320×180 for a 16:9 frame. A crop gets the
/// same budget for its own area (never more than its real pixels), so small regions keep detail.
const SAMPLE_AREA: f32 = 320.0 * 180.0;
/// A pixel counts as changed when its gray value moves by more than this.
const DIFF: u8 = 10;
/// Fewer changed pixels than this are noise: compression artifacts, a blinking cursor.
/// At 320 px wide this still catches a single typed character on a 1080p screen.
const MIN_PIXELS: usize = 6;

#[derive(Clone)]
pub struct Scan {
    /// Absolute source times where something changed, merged and followed by one sample.
    pub active: Vec<(f64, f64)>,
    /// Source times the scan looked at; activity outside it is unknown.
    pub range: (f64, f64),
}

pub enum Msg {
    Progress(f32),
    /// Scan results so far, for live feedback: replace intervals from index `from` on (the last
    /// one sent may have grown) with `intervals`; everything before `scanned` (seconds) is known.
    Partial {
        from: usize,
        intervals: Vec<(f64, f64)>,
        scanned: f64,
    },
    Done(Result<Scan, String>),
}

pub struct Running {
    pub rx: Receiver<Msg>,
    cancel: Arc<AtomicBool>,
    end: Arc<AtomicU64>,
}

impl Running {
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Move where the scan stops (source seconds). Behind what it already read: it stops now.
    pub fn set_end(&self, t: f64) {
        self.end.store(t.to_bits(), Ordering::Relaxed);
    }
}

impl Scan {
    /// This scan joined with one of the range right next to it (or overlapping).
    pub fn merge(mut self, other: Scan) -> Scan {
        self.active.extend(other.active);
        self.active.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut merged: Vec<(f64, f64)> = Vec::new();
        for (a, b) in self.active {
            match merged.last_mut() {
                Some(last) if a <= last.1 + 1e-6 => last.1 = last.1.max(b),
                _ => merged.push((a, b)),
            }
        }
        let range = (
            self.range.0.min(other.range.0),
            self.range.1.max(other.range.1),
        );
        Scan {
            active: merged,
            range,
        }
    }
}

/// Scan `src` in the background. `aspect` is the displayed (rotated) aspect ratio, so the
/// sampled frames line up with the rectangles drawn over the video.
/// What to compare: the frame's display `aspect` and pixel `size`, the applied `crop` (only it ends
/// up in the export), and ignored `zones`, all normalized to the whole frame.
pub struct Area {
    pub aspect: f32,
    pub size: [u32; 2],
    pub crop: Option<Rect>,
    pub zones: Vec<Rect>,
    /// Only look at these source times (the trim plus padding); `None` = the whole file.
    pub range: Option<(f64, f64)>,
}

pub fn scan(src: PathBuf, dur: f64, area: Area, ctx: Context) -> Running {
    let (tx, rx) = channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let stop = cancel.clone();
    let to = area.range.map_or(f64::INFINITY, |(_, b)| b);
    let end = Arc::new(AtomicU64::new(to.to_bits()));
    let limit = end.clone();
    std::thread::spawn(move || {
        let res = run(&src, dur, &area, &tx, &stop, &limit, &ctx);
        if !stop.load(Ordering::Relaxed) {
            let _ = tx.send(Msg::Done(res));
            crate::wake(&ctx);
        }
    });
    Running { rx, cancel, end }
}
