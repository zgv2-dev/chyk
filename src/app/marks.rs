use super::*;

/// The Chyk mark: the app icon's scissors and trim box without the tile (`assets/mark.png`,
/// 4x the size it is drawn at, so it stays crisp on 2x displays).
pub(super) fn logo(ui: &mut Ui) -> Response {
    let (r, resp) = ui.allocate_exact_size(vec2(30.0, 24.0), Sense::hover());
    let id = egui::Id::new("chyk-mark");
    let tex = ui.ctx().data(|d| d.get_temp::<egui::TextureHandle>(id));
    let tex = tex.unwrap_or_else(|| {
        let png = eframe::icon_data::from_png_bytes(include_bytes!("../../assets/mark.png"))
            .expect("assets/mark.png is a valid PNG");
        let img = egui::ColorImage::from_rgba_unmultiplied(
            [png.width as usize, png.height as usize],
            &png.rgba,
        );
        let tex = ui
            .ctx()
            .load_texture("chyk-mark", img, egui::TextureOptions::LINEAR);
        ui.ctx().data_mut(|d| d.insert_temp(id, tex.clone()));
        tex
    });
    egui::Image::new(&tex).paint_at(ui, r);
    resp
}

/// File name over its codec/size/duration line, painted with tighter line spacing than labels
/// so both fit the header row. Long names are cut with an ellipsis.
pub(super) fn file_block(ui: &mut Ui, name: &str, meta: &[(&str, String)]) {
    let p = ui.painter();
    let mut job = egui::text::LayoutJob::single_section(
        name.into(),
        egui::TextFormat::simple(bold(13.0), TEXT),
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width(320.0);
    let name_g = p.layout_job(job);
    let parts: Vec<_> = meta
        .iter()
        .flat_map(|(glyph, text)| {
            [
                p.layout_no_wrap((*glyph).into(), mono(11.0), GREEN_DIM),
                p.layout_no_wrap(text.clone(), mono(11.0), MUTED),
            ]
        })
        .collect();
    let meta_w: f32 = parts.iter().map(|g| g.size().x).sum::<f32>()
        + 5.0 * parts.len() as f32
        + 8.0 * (meta.len() as f32 - 1.0);
    let w = name_g.size().x.max(meta_w);
    let (r, _) = ui.allocate_exact_size(vec2(w, HEADER_ROW.min(32.0)), Sense::hover());
    let p = ui.painter();
    let name_h = name_g.size().y;
    p.galley(pos2(r.min.x, r.center().y - 1.0 - name_h), name_g, TEXT);
    let mut x = r.min.x;
    for (i, g) in parts.into_iter().enumerate() {
        let gw = g.size().x;
        p.galley(pos2(x, r.center().y + 1.0), g, MUTED);
        x += gw + if i % 2 == 1 { 13.0 } else { 5.0 };
    }
}

/// Small green dot on a mode button whose effect is applied while its panel is closed.
pub(super) fn applied_dot(ui: &Ui, button: Rect, on: bool) {
    if on {
        ui.painter()
            .circle_filled(button.right_top() + vec2(-6.0, 6.0), 2.5, GREEN);
    }
}

/// Diagonal red hatching for stretches strip will cut.
pub(super) fn hatch(p: &egui::Painter, r: Rect) {
    let p = p.with_clip_rect(r.intersect(p.clip_rect()));
    p.rect_filled(r, 0.0, Color32::from_rgba_premultiplied(40, 0, 8, 140));
    let s = Stroke::new(2.5, RED.gamma_multiply(0.42));
    let mut x = r.min.x - r.height();
    while x < r.max.x {
        p.line_segment([pos2(x, r.max.y), pos2(x + r.height(), r.min.y)], s);
        x += 8.0;
    }
    p.vline(r.min.x, r.y_range(), Stroke::new(1.0, RED));
    p.vline(r.max.x, r.y_range(), Stroke::new(1.0, RED));
}
