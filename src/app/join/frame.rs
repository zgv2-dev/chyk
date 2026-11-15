//! The output frame popover: a size (largest, a clip's, or custom W×H) and a frame rate.

use super::bar::fps_label;
use super::*;

/// One custom number field, committed on Enter or when it loses focus.
fn number(
    ui: &mut Ui,
    w: f32,
    v: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    decimals: usize,
) -> bool {
    ui.style_mut().override_font_id = Some(mono(12.0));
    ui.spacing_mut().interact_size.y = 28.0;
    let drag = egui::DragValue::new(v)
        .range(range)
        .max_decimals(decimals)
        .update_while_editing(false);
    ui.add_sized(vec2(w, 28.0), drag).changed()
}

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
        let (big, top) = reel::largest(&r.clips);
        let mut sizes = vec![(t!("join.largest").to_string(), big)];
        for (i, c) in r.clips.iter().enumerate() {
            sizes.push((format!("{}", i + 1), c.info.size));
        }
        let mut chosen = None;
        field_label(ui, icon::FRAME_CORNERS, t!("join.size"));
        for (name, s) in &sizes {
            let text = format!("{name} · {}×{}", s[0], s[1]);
            let b = button(ui, "", &text, reel::even(*s) == r.frame, CHIP);
            if b.clicked() {
                chosen = Some((*s, r.fps));
            }
        }
        let custom = !sizes.iter().any(|(_, s)| reel::even(*s) == r.frame);
        let _ = button(ui, "", t!("join.custom"), custom, CHIP);
        let [mut w, mut h] = r.frame.map(f64::from);
        let half = (ui.available_width() - 24.0).max(60.0) / 2.0;
        ui.horizontal(|ui| {
            let a = number(ui, half, &mut w, 16.0..=8192.0, 0);
            ui.label(RichText::new("×").font(mono(12.0)).color(MUTED));
            if a | number(ui, half, &mut h, 16.0..=8192.0, 0) {
                chosen = Some(([w as u32, h as u32], r.fps));
            }
        });
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
        let mut labels: Vec<String> = rates.iter().map(|&f| fps_label(f)).collect();
        labels.push(t!("join.custom").to_string());
        let id = ui.id().with("join_fps_custom");
        let picked = ctx.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
        let preset = rates.iter().position(|f| (f - r.fps).abs() < 0.01);
        let on = preset.filter(|_| !picked).unwrap_or(rates.len());
        match segmented(ui, "join_fps", &labels, on) {
            Some(i) if i < rates.len() => {
                ctx.data_mut(|d| d.insert_temp(id, false));
                chosen = Some((r.frame, rates[i]));
            }
            Some(_) => {
                ctx.data_mut(|d| d.insert_temp(id, true));
            }
            None => {}
        }
        if on == rates.len() {
            let mut f = r.fps;
            if ui
                .horizontal(|ui| number(ui, half, &mut f, 1.0..=240.0, 3))
                .inner
            {
                chosen = Some((r.frame, f));
            }
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
