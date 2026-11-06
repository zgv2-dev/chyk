//! Everything that shells out to the ffmpeg CLI: export and filmstrip thumbnails.

use crate::i18n::t;
use eframe::egui::{ColorImage, Context};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, OnceLock};

mod args;
mod cut;
mod encode;
mod graph;
mod run;
#[cfg(test)]
mod tests;
mod thumbs;

use args::*;
use cut::*;
use encode::*;
use graph::*;
pub(crate) use run::{error_tail, export};
pub use thumbs::thumbnails;

/// Encoders to try, best first. Unavailable ones are filtered out; failing ones fall through to the next.
const H264: &[&str] = &[
    "h264_videotoolbox",
    "h264_nvenc",
    "h264_qsv",
    "h264_amf",
    "libx264",
    "h264_mf",
    "libopenh264",
];
const HEVC: &[&str] = &[
    "hevc_videotoolbox",
    "hevc_nvenc",
    "hevc_qsv",
    "hevc_amf",
    "libx265",
    "hevc_mf",
];

/// `ffmpeg` bundled with Chyk, else the one on PATH.
pub(crate) fn command(name: &str) -> Command {
    #[cfg(bundled)]
    let dir = Some(crate::bundle::dir());
    #[cfg(not(bundled))]
    let dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    let bundled = dir
        .map(|d| d.join(format!("{name}{}", std::env::consts::EXE_SUFFIX)))
        .filter(|p| p.is_file());
    let mut cmd = Command::new(bundled.as_deref().unwrap_or(Path::new(name)));
    cmd.stdin(Stdio::null());
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000); // CREATE_NO_WINDOW
    cmd
}

#[derive(Clone)]
pub struct Job {
    pub src: PathBuf,
    pub dst: PathBuf,
    pub start: f64,
    pub end: f64,
    /// `[w, h, x, y]` in source pixels.
    pub crop: Option<[u32; 4]>,
    /// Re-encode even without a crop, so the cut lands on the exact frame.
    pub precise: bool,
    /// Source video codec as named by ffmpeg (`h264`, `hevc`, ...).
    pub codec: String,
    /// Index into [`QUALITIES`]. Anything below the best re-encodes; [`COMPRESS`] also re-encodes audio.
    pub quality: usize,
    /// Drop all audio tracks.
    pub mute: bool,
    /// Retiming factor for the output. `1.0` leaves the source speed unchanged.
    pub speed: f64,
    /// Keep only these absolute source-time intervals, concatenated. Forces a re-encode.
    pub keep: Option<Vec<(f64, f64)>>,
}

impl Job {
    /// Source duration: the trim length, or the sum of the kept intervals.
    pub fn kept_duration(&self) -> f64 {
        self.keep.as_ref().map_or(self.end - self.start, |keep| {
            keep.iter().map(|&(a, b)| (b - a).max(0.0)).sum()
        })
    }

    pub fn retime(&self) -> bool {
        (self.speed - 1.0).abs() > 1e-9
    }

    /// Duration after retiming. Used for progress and estimates.
    pub fn output_duration(&self) -> f64 {
        self.kept_duration() / self.speed.max(0.001)
    }
}

/// Export containers, by file extension.
pub const FORMATS: &[&str] = &["mp4", "mov", "mkv"];
/// Quality levels, best first. Only the best one may stream-copy.
/// UI text keys, see [`crate::i18n`].
pub const QUALITIES: &[&str] = &[
    "quality.high",
    "quality.medium",
    "quality.low",
    "quality.compress",
];
/// Longest filter graph passed on the command line; a longer one goes in a file. Windows caps the
/// whole command line at 32 KiB, and each kept interval costs ~90 characters of expressions.
const GRAPH_INLINE: usize = 24_000;
/// Index into [`QUALITIES`] for the size-first preset: re-encodes video and audio for the smallest file.
pub const COMPRESS: usize = 3;

pub enum Msg {
    Progress(f32),
    /// Bytes written, after a successful export.
    Done(Result<u64, String>),
}

pub struct Running {
    pub rx: Receiver<Msg>,
    cancel: Arc<AtomicBool>,
    /// Whether this export used [`COMPRESS`].
    pub compress: bool,
}

impl Running {
    /// Stops ffmpeg and deletes the partial output.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
