use super::*;

/// Shortcut legend under the drop target.
pub(super) fn legend(ui: &Ui, p: &egui::Painter, r: Rect) {
    let short = |m, k| ui.ctx().format_shortcut(&egui::KeyboardShortcut::new(m, k));
    let legend: [(Vec<String>, &str); 8] = [
        (vec![short(Modifiers::COMMAND, Key::O)], icon::FOLDER_OPEN),
        (vec![t!("key.space").into()], icon::PLAY),
        (vec!["←".into(), "→".into()], icon::FILM_STRIP),
        (vec!["I".into(), "O".into()], icon::SCISSORS),
        (vec!["C".into()], icon::CROP),
        (vec![",".into(), ".".into()], icon::GAUGE),
        (vec![short(Modifiers::COMMAND, Key::S)], icon::EXPORT),
        (vec![short(Modifiers::COMMAND, Key::W)], icon::X_CIRCLE),
    ];
    // Two rows of four in equal-width columns, narrower than the drop box. In each cell the
    // key caps are right-aligned to a shared edge, so the icons line up across rows.
    let measure =
        |s: &str| (p.layout_no_wrap(s.into(), mono(11.0), TEXT).size().x + 12.0).max(22.0);
    let keys_w = |keys: &[String]| {
        keys.iter().map(|k| measure(k)).sum::<f32>() + 4.0 * (keys.len() as f32 - 1.0)
    };
    let key_col = legend.iter().map(|(k, _)| keys_w(k)).fold(0.0, f32::max);
    let cell = key_col + 8.0 + 14.0;
    let gap = 24.0;
    let total = LEGEND_PER_ROW as f32 * cell + (LEGEND_PER_ROW - 1) as f32 * gap;
    for (n, (keys, glyph)) in legend.iter().enumerate() {
        let (row, col) = (n / LEGEND_PER_ROW, n % LEGEND_PER_ROW);
        let x0 = r.center().x - total / 2.0 + col as f32 * (cell + gap);
        let y = r.max.y + 26.0 + row as f32 * 30.0;
        let mut x = x0 + key_col - keys_w(keys);
        for k in keys {
            x += keycap(p, pos2(x, y), k) + 4.0;
        }
        p.text(
            pos2(x0 + key_col + 8.0, y + 11.0),
            Align2::LEFT_CENTER,
            *glyph,
            mono(14.0),
            GREEN_DIM,
        );
    }
}
