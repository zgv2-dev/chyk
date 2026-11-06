//! Finding the parts of a video where the picture actually changes, so the static parts can be cut.

use crate::ffmpeg;
use crate::i18n::t;
use eframe::egui::{Context, Pos2, Rect, pos2};
use std::io::{BufReader, ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
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
}

impl Running {
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
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
}

pub fn scan(src: PathBuf, dur: f64, area: Area, ctx: Context) -> Running {
    let (tx, rx) = channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let stop = cancel.clone();
    std::thread::spawn(move || {
        let res = run(&src, dur, &area, &tx, &stop, &ctx);
        if !stop.load(Ordering::Relaxed) {
            let _ = tx.send(Msg::Done(res));
            ctx.request_repaint();
        }
    });
    Running { rx, cancel }
}
