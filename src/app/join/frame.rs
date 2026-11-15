//! The output frame popover: a size (largest, a clip's, or typed W×H) and a frame rate (a
//! preset or typed).

use super::bar::fps_label;
use super::field::number_box;
use super::*;

impl App {
    pub(in crate::app) fn frame_pop(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let Some(r) = self.reel_now() else { return };
        self.pop_head(ui, icon::FRAME_CORNERS, t!("join.frame"), |_| {});
        ui.label(
            RichText::new(t!("join.frame_sub"))
                .font(mono(11.0))
                .color(MUTED),
        );
        ui.add_space(10.0);
        let mut chosen = None;
        let [fw, fh] = r.frame;

        let (big, top) = reel::largest(&r.clips);
        let mut sizes = vec![(t!("join.largest").to_string(), big)];
        for (i, c) in r.clips.iter().enumerate() {
            sizes.push((format!("{}", i + 1), c.info.size));
        }
        let names: Vec<String> = sizes
            .iter()
            .map(|(n, s)| format!("{n} · {}×{}", s[0], s[1]))
            .collect();
        let preset = sizes.iter().position(|(_, s)| reel::even(*s) == r.frame);
        let custom = preset.is_none();
        let label = format!("{} · {fw}×{fh}", t!("join.custom"));
        let mut typed = None;
        let mut draw = |ui: &mut Ui, rect: Rect| {
            let rect = rect.shrink(1.0);
            let half = (rect.width() - 20.0) / 2.0;
            let a = Rect::from_min_size(rect.min, vec2(half, rect.height()));
            let b = a.translate(vec2(half + 20.0, 0.0));
            let mid = rect.center();
            ui.painter()
                .text(mid, Align2::CENTER_CENTER, "×", mono(12.0), MUTED);
            let shown = |v: u32| custom.then(|| v.to_string());
            let w = number_box(ui, a, "join_w", shown(fw), t!("join.w"), custom);
            let h = number_box(ui, b, "join_h", shown(fh), t!("join.h"), custom);
            if w.is_some() || h.is_some() {
                let side = |v: Option<f64>, d: u32| v.map_or(d, |v| v.round() as u32);
                typed = Some([side(w, fw), side(h, fh)].map(|v| v.clamp(16, 8192)));
            }
        };
        field_label(ui, icon::FRAME_CORNERS, t!("join.size"));
        let at = preset.unwrap_or(sizes.len());
        if let Some(i) = dropdown_with(ui, "join_size", &names, &[], at, Some((&label, &mut draw)))
        {
            chosen = Some((sizes[i].1, r.fps));
        }
        if let Some(s) = typed {
            chosen = Some((s, r.fps));
        }

        ui.add_space(10.0);
        field_label(ui, icon::FILM_STRIP, t!("join.fps"));
        let mut rates: Vec<f64> = r
            .clips
            .iter()
            .map(|c| c.info.fps)
            .chain([24.0, 30.0, 60.0, top])
            .collect();
        rates.sort_by(f64::total_cmp);
        rates.dedup_by(|a, b| (*a - *b).abs() < 0.01);
        let labels: Vec<String> = rates.iter().map(|&f| fps_label(f)).collect();
        let preset = rates.iter().position(|f| (f - r.fps).abs() < 0.01);
        let custom = preset.is_none();
        let mut typed = None;
        let mut draw = |ui: &mut Ui, rect: Rect| {
            let shown = custom.then(|| fps_label(r.fps));
            let f = number_box(ui, rect, "join_fps_n", shown, t!("join.custom"), custom);
            typed = f.map(|f| f.clamp(1.0, 240.0));
        };
        let at = preset.unwrap_or(usize::MAX);
        if let Some(i) = segmented_with(ui, "join_fps", &labels, at, Some((76.0, &mut draw))) {
            chosen = Some((r.frame, rates[i]));
        }
        if let Some(f) = typed {
            chosen = Some((r.frame, f));
        }
        ui.add_space(8.0);
        ui.label(
            RichText::new(t!("join.frame_hint"))
                .font(mono(10.5))
                .color(MUTED),
        );
        if let Some((size, fps)) = chosen {
            self.set_frame(size, fps, ctx);
        }
    }
}
