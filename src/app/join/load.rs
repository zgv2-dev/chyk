//! A join's clips going to mpv and coming back as one video; selecting, moving and removing
//! clips.

use super::*;

impl App {
    /// Give mpv the clips again (after adding, moving or removing); `reel_loaded` takes it from there.
    pub(in crate::app) fn reload_reel(&mut self) {
        let Some(r) = &self.reel else { return };
        let (url, first) = (r.edl(), r.clips[0].path.clone());
        self.cancel_scan();
        self.player.cmd("loadfile", &[&url]);
        self.loading = Some(first);
        self.timeline.strips.clear();
        self.timeline.strip_key = None;
        self.thumbs = vec![None; THUMBS];
        self.thumbs_rx = None;
    }

    /// mpv opened the join: the editor shows it as one video in the output frame.
    pub(in crate::app) fn reel_loaded(&mut self, ctx: &egui::Context) {
        let Some(r) = &self.reel else { return };
        let [w, h] = r.frame;
        // The encoder follows the biggest clip's codec, as the export does.
        let big = r
            .clips
            .iter()
            .max_by_key(|c| c.info.size[0] * c.info.size[1]);
        let name = r.clips[0]
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        let title = format!("{name} +{} — Chyk", r.clips.len() - 1);
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        self.video = Some(Video {
            path: r.clips[0].path.clone(),
            dur: r.total(),
            size: r.frame,
            aspect: w as f32 / h.max(1) as f32,
            codec: big.map_or_else(String::new, |c| c.info.codec.clone()),
            bytes: r
                .clips
                .iter()
                .map(|c| std::fs::metadata(&c.path).map_or(0, |m| m.len()))
                .sum(),
        });
        self.trim = r.kept_range(r.sel);
        let (start, total) = (self.trim.0, r.total());
        // Strip doesn't scan joined clips yet.
        self.strip.enabled = false;
        if self.panel == Some(Panel::Strip) {
            self.panel = None;
        }
        self.timeline = Timeline {
            zoom: 1.0,
            center: total / 2.0,
            ..Default::default()
        };
        if let Some(m) = &mut self.reel {
            m.shown = None;
        }
        self.thumb_crop = None;
        self.deck_need = [0.0; 2];
        self.meta = None;
        self.load_thumbs(ctx);
        self.player.seek(start);
        if self.saved.is_none() {
            self.saved = Some(self.edits());
        }
        self.history.reset(self.snapshot());
    }

    /// Make clip `i` the one the trim box edits.
    pub(in crate::app) fn select_clip(&mut self, i: usize) {
        let Some(mut r) = self.reel_now() else { return };
        if i >= r.clips.len() || i == r.sel {
            return;
        }
        r.sel = i;
        // A placement drag belongs to the clip it started on.
        if self.drag == Some(Drag::Place) {
            self.drag = None;
        }
        self.trim = r.kept_range(i);
        let (a, b) = r.bounds(i);
        if !(a..b).contains(&self.pos()) {
            self.player.seek(self.trim.0);
        }
        self.reel = Some(r);
    }

    /// Move the selected clip `by` places.
    pub(in crate::app) fn shift_clip(&mut self, by: isize) {
        let Some(mut r) = self.reel_now() else { return };
        let i = r.sel;
        if r.shift(i, by) != i {
            self.reel = Some(r);
            self.reload_reel();
        }
    }

    /// Remove clip `i`. With one left, it opens on its own again, keeping its trim.
    pub(in crate::app) fn remove_clip(&mut self, i: usize) {
        let Some(mut r) = self.reel_now() else { return };
        if i >= r.clips.len() {
            return;
        }
        r.clips.remove(i);
        if i < r.sel {
            r.sel -= 1;
        }
        r.sel = r.sel.min(r.clips.len() - 1);
        if r.clips.len() == 1 {
            let c = r.clips.remove(0);
            let rest = self.reel_rest(c.path.clone(), c.trim);
            // mpv keeps zoom and pan across files; the lone video shows plain.
            self.show_fit(crate::ffmpeg::Fit::Fit, c.info.size, r.frame);
            self.open(c.path);
            self.restore = rest;
            return;
        }
        self.reel = Some(r);
        self.reload_reel();
    }
}
