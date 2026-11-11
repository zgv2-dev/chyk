//! Undo and redo of the edits (trim, crop, Strip, speed), recorded at rest: a drag, a typed value
//! or a scrub is one step because the frame it ends in is the first one to see a different state.

use super::*;

const DEPTH: usize = 100;

#[derive(Clone, PartialEq, Debug)]
pub(super) struct Snapshot {
    pub(super) trim: (f64, f64),
    pub(super) crop: Option<Rect>,
    pub(super) preset: usize,
    pub(super) strip: bool,
    pub(super) zones: Vec<Rect>,
    pub(super) pad: f64,
    pub(super) speed: f64,
}

pub(super) struct History<T> {
    past: Vec<T>,
    future: Vec<T>,
    /// The state as of the last settled frame.
    rest: Option<T>,
}

impl<T> Default for History<T> {
    fn default() -> Self {
        Self {
            past: Vec::new(),
            future: Vec::new(),
            rest: None,
        }
    }
}

impl<T: Clone + PartialEq> History<T> {
    /// Record `now` as the resting state; if it differs from the last one that is a new step.
    pub(super) fn settle(&mut self, now: &T) {
        match self.rest.take() {
            Some(r) if r != *now => {
                self.past.push(r);
                if self.past.len() > DEPTH {
                    self.past.remove(0);
                }
                self.future.clear();
            }
            _ => {}
        }
        self.rest = Some(now.clone());
    }

    pub(super) fn undo(&mut self, now: &T) -> Option<T> {
        self.settle(now);
        let prev = self.past.pop()?;
        self.future.push(now.clone());
        self.rest = Some(prev.clone());
        Some(prev)
    }

    pub(super) fn redo(&mut self, now: &T) -> Option<T> {
        self.settle(now);
        let next = self.future.pop()?;
        self.past.push(now.clone());
        self.rest = Some(next.clone());
        Some(next)
    }

    pub(super) fn reset(&mut self, now: T) {
        *self = Self {
            rest: Some(now),
            ..Self::default()
        };
    }
}

/// Not while a file loads: the history still belongs to the previous one.
pub(super) fn can_undo(video: bool, loading: bool) -> bool {
    video && !loading
}

/// The crop as an edit: a box over the whole frame (Crop just opened) crops nothing.
pub(super) fn edit_crop(c: Option<Rect>) -> Option<Rect> {
    c.filter(|&r| !crop::is_full(r))
}

impl App {
    pub(super) fn snapshot(&self) -> Snapshot {
        Snapshot {
            trim: self.trim,
            crop: edit_crop(self.crop),
            preset: self.preset,
            strip: self.strip.enabled,
            zones: self.strip.zones.clone(),
            pad: self.strip.pad,
            speed: self.speed,
        }
    }

    /// Plain field assignment, shared by undo and the session restore.
    pub(super) fn set_edits(&mut self, s: Snapshot) {
        self.trim = s.trim;
        self.crop = s.crop;
        self.preset = s.preset;
        self.set_speed(s.speed);
        self.strip.enabled = s.strip;
        self.strip.pad = s.pad;
        self.strip.zones = s.zones;
    }

    /// Undo and redo only: also keeps the panels, the scan and zone editing consistent.
    pub(super) fn apply_snapshot(&mut self, s: Snapshot, ctx: &egui::Context) {
        let zones_differ = self.strip.zones != s.zones;
        if self.strip.enabled != s.strip {
            // The bar follows the switch, as with the Strip button.
            self.end_zone_edit();
            match (s.strip, self.panel) {
                (true, None) => self.panel = Some(Panel::Strip),
                (false, Some(Panel::Strip)) => self.panel = None,
                _ => {}
            }
        }
        self.set_edits(s);
        if zones_differ {
            self.zones_changed();
        }
        if self.crop.is_none() && self.panel == Some(Panel::Crop) {
            self.panel = None;
        }
        self.crop_applied(ctx);
        // Zone editing keeps scans off; after it ends the user rescans, as before.
        if self.strip.enabled && !self.strip.edit {
            self.ensure_scan(ctx);
        }
    }

    /// Called at the end of every frame; records the edits once nothing is being dragged or typed.
    pub(super) fn settle_history(&mut self, ctx: &egui::Context) {
        let at_rest = self.video.is_some()
            && self.loading.is_none()
            && self.drag.is_none()
            && self.strip.drag.is_none()
            && !ctx.egui_wants_keyboard_input()
            && !ctx.input(|i| i.pointer.any_down());
        if at_rest {
            self.history.settle(&self.snapshot());
        }
    }

    // Allowed during an export: they change only the editor; the running export holds its own `Job`.
    pub(super) fn undo_redo(&mut self, redo: bool, ctx: &egui::Context) {
        if !can_undo(self.video.is_some(), self.loading.is_some()) {
            return;
        }
        let now = self.snapshot();
        let to = if redo {
            self.history.redo(&now)
        } else {
            self.history.undo(&now)
        };
        if let Some(s) = to {
            self.apply_snapshot(s, ctx);
            ctx.request_repaint();
        }
    }
}

#[cfg(test)]
mod tests;
