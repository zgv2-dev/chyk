//! Join: clips laid end to end on one timeline. mpv plays them as one virtual file (`edl://`);
//! each clip keeps its own trim and fit, and the output frame they are fitted into.

use crate::ffmpeg::{ClipInfo, Fit};
use std::path::PathBuf;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Clip {
    pub(super) path: PathBuf,
    pub(super) info: ClipInfo,
    /// Kept part, in the clip's own seconds.
    pub(super) trim: (f64, f64),
    pub(super) fit: Fit,
}

impl Clip {
    pub(super) fn new(path: PathBuf, info: ClipInfo) -> Self {
        let trim = (0.0, info.dur);
        Self {
            path,
            info,
            trim,
            fit: Fit::Fit,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Reel {
    pub(super) clips: Vec<Clip>,
    /// The clip the trim box edits.
    pub(super) sel: usize,
    /// Output `[w, h]` (even) and frame rate.
    pub(super) frame: [u32; 2],
    pub(super) fps: f64,
    /// The clip whose fit mpv shows now.
    pub(super) shown: Option<usize>,
}

/// `[w, h]` rounded up to even sides (4:2:0 encoders reject odd ones).
pub(super) fn even([w, h]: [u32; 2]) -> [u32; 2] {
    [w.max(1).div_ceil(2) * 2, h.max(1).div_ceil(2) * 2]
}

/// The largest width, height and frame rate among `clips`, sides rounded up to even.
pub(super) fn largest(clips: &[Clip]) -> ([u32; 2], f64) {
    let w = clips.iter().map(|c| c.info.size[0]).max().unwrap_or(2);
    let h = clips.iter().map(|c| c.info.size[1]).max().unwrap_or(2);
    let fps = clips.iter().map(|c| c.info.fps).fold(0.0, f64::max);
    (even([w, h]), if fps > 0.0 { fps } else { 30.0 })
}

impl Reel {
    /// Where clip `i` starts on the timeline: the full lengths of the clips before it.
    pub(super) fn start(&self, i: usize) -> f64 {
        self.clips[..i.min(self.clips.len())]
            .iter()
            .map(|c| c.info.dur)
            .sum()
    }

    pub(super) fn total(&self) -> f64 {
        self.start(self.clips.len())
    }

    /// The clip under timeline time `t`.
    pub(super) fn at(&self, t: f64) -> usize {
        let mut end = 0.0;
        for (i, c) in self.clips.iter().enumerate() {
            end += c.info.dur;
            if t < end {
                return i;
            }
        }
        self.clips.len().saturating_sub(1)
    }

    /// Timeline range of clip `i`.
    pub(super) fn bounds(&self, i: usize) -> (f64, f64) {
        let s = self.start(i);
        (s, s + self.clips[i].info.dur)
    }

    /// Clip `i`'s kept part on the timeline.
    pub(super) fn kept_range(&self, i: usize) -> (f64, f64) {
        let s = self.start(i);
        (s + self.clips[i].trim.0, s + self.clips[i].trim.1)
    }

    /// Length of the joined result.
    pub(super) fn kept(&self) -> f64 {
        self.clips.iter().map(|c| c.trim.1 - c.trim.0).sum()
    }

    /// Joined (output) time at timeline time `t`: the kept time before it.
    pub(super) fn output_time(&self, t: f64) -> f64 {
        let mut out = 0.0;
        for i in 0..self.clips.len() {
            let (a, b) = self.kept_range(i);
            out += (t.min(b) - a).max(0.0);
        }
        out
    }

    /// mpv's virtual file for the whole clips, one after another.
    pub(super) fn edl(&self) -> String {
        let parts: Vec<String> = self
            .clips
            .iter()
            .map(|c| {
                let p = c.path.to_string_lossy();
                // `%len%` quotes the path by its byte length, so commas and semicolons are safe.
                format!("%{}%{p},0,{:.6}", p.len(), c.info.dur)
            })
            .collect();
        format!("edl://{}", parts.join(";"))
    }

    /// Where playback at timeline time `t` should be: inside a kept part, `None` to stay;
    /// before a clip's kept part, its start; past it, the next clip's; past the last, `Err`.
    pub(super) fn play_target(&self, t: f64) -> Result<Option<f64>, ()> {
        let i = self.at(t);
        let (a, b) = self.kept_range(i);
        if t < a - 0.02 {
            Ok(Some(a))
        } else if t >= b - 0.01 {
            match (i + 1 < self.clips.len()).then(|| self.kept_range(i + 1).0) {
                Some(next) => Ok(Some(next)),
                None => Err(()),
            }
        } else {
            Ok(None)
        }
    }

    /// Move clip `i` by `by` places; the selection follows it. Returns its new place.
    pub(super) fn shift(&mut self, i: usize, by: isize) -> usize {
        let j = (i as isize + by).clamp(0, self.clips.len() as isize - 1) as usize;
        let c = self.clips.remove(i);
        self.clips.insert(j, c);
        self.sel = j;
        j
    }

    /// Whether every clip already has the output frame's size and rate (nothing to fit).
    pub(super) fn uniform(&self) -> bool {
        self.clips.iter().all(|c| {
            c.info.size == self.frame && (c.info.fps - self.fps).abs() < 0.01 && c.fit.keeps_frame()
        })
    }

    /// What decides the join's export, for the unsaved-changes check.
    pub(super) fn edits(&self) -> ReelEdits {
        let clips = self
            .clips
            .iter()
            .map(|c| (c.path.clone(), c.trim, c.fit))
            .collect();
        (clips, self.frame, (self.fps * 1000.0).round() as i64)
    }
}

/// Clips (path, trim, fit), frame and frame rate in thousandths.
pub(super) type ReelEdits = (Vec<(PathBuf, (f64, f64), Fit)>, [u32; 2], i64);
