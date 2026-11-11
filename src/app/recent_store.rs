//! Recent files: what the start screen shows, recording on open, the thumbnail cache.

use super::*;
use crate::recent::{self, Recent};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
mod tests;

const SHOWN: usize = 5;
pub(super) const ROW: f32 = 46.0;
pub(super) const HEAD: f32 = 24.0;

pub(super) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Height the list takes in the start screen's stack, including the gap after it; 0 for no rows.
pub(super) fn height(rows: usize) -> f32 {
    if rows == 0 {
        0.0
    } else {
        HEAD + ROW * rows as f32 + super::start::STACK_GAP
    }
}

/// "2024-03-09" for unix seconds (civil-from-days, proleptic Gregorian).
fn date(secs: u64) -> String {
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y}-{m:02}-{d:02}")
}

pub(super) fn ago(now: u64, then: u64) -> String {
    let s = now.saturating_sub(then);
    match s {
        0..3600 => t!("recent.minutes", n = (s / 60).max(1)).to_string(),
        3600..86_400 => t!("recent.hours", n = s / 3600).to_string(),
        86_400..172_800 => t!("recent.yesterday").to_string(),
        _ => date(then),
    }
}

pub(super) fn clock(secs: f64) -> String {
    let s = secs.round() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

impl App {
    /// The entries that still exist, newest first, as many as the list shows.
    pub(super) fn recent_shown(&self) -> Vec<Recent> {
        let found = self.recent.iter().filter(|r| r.path.exists());
        found.take(SHOWN).cloned().collect()
    }

    /// Open entry `i` of the shown list (⌘1–⌘5, Enter, click).
    pub(super) fn open_recent(&mut self, i: usize) {
        if let Some(r) = self.recent_shown().get(i) {
            self.open(r.path.clone());
        }
    }

    /// Put the open video first in the list and save it; a failed write is ignored.
    pub(super) fn record_recent(&mut self) {
        let Some(v) = &self.video else { return };
        let item = Recent {
            path: v.path.clone(),
            opened: now(),
            codec: v.codec.clone(),
            size: v.size,
            dur: v.dur,
        };
        recent::push(&mut self.recent, item);
        let _ = recent::save(&self.recent);
        self.recent_tex.clear();
    }

    /// Cache the first filmstrip tile as the file's thumbnail, when the tiles cover the whole file.
    pub(super) fn save_recent_thumb(&self, img: &egui::ColorImage) {
        let Some(v) = &self.video else { return };
        let (a, b) = self.timeline.thumb_range;
        if self.thumb_crop.is_none() && a <= 1e-3 && b >= v.dur - 1e-3 {
            recent::save_thumb(
                &v.path,
                [img.width() as u32, img.height() as u32],
                img.as_raw(),
            );
        }
    }

    pub(super) fn recent_thumb(
        &mut self,
        ctx: &egui::Context,
        path: &Path,
    ) -> Option<TextureHandle> {
        self.recent_tex
            .entry(path.to_path_buf())
            .or_insert_with(|| {
                let ([w, h], rgba) = recent::load_thumb(path)?;
                let img = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
                Some(ctx.load_texture("recent", img, Default::default()))
            })
            .clone()
    }
}
