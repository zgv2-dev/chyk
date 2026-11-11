use super::*;

/// Keyboard hints under the drop target: key caps and a short label each, centered on `cx` from
/// `top` down. One row when it fits in `avail`, else two.
pub(super) fn legend(p: &egui::Painter, top: f32, avail: f32, cx: f32) {
    let caps =
        |cmds: &[Cmd]| -> Vec<String> { cmds.iter().flat_map(|&c| keys::cmd_caps(c)).collect() };
    let hints = [
        (caps(&[Cmd::PlayPause]), t!("hint.play").to_string()),
        (
            caps(&[Cmd::SetIn, Cmd::SetOut]),
            t!("hint.trim").to_string(),
        ),
        (caps(&[Cmd::Crop]), t!("mode.crop").to_string()),
        (caps(&[Cmd::Strip]), t!("mode.strip").to_string()),
        (caps(&[Cmd::Export]), t!("export.button").to_string()),
        (caps(&[Cmd::Shortcuts]), t!("status.shortcuts").to_string()),
    ];
    let gap = 18.0;
    let width = |(c, l): &(Vec<String>, String)| {
        keys::caps_width(p, c) + 7.0 + p.layout_no_wrap(l.to_string(), mono(11.5), MUTED).size().x
    };
    let widths: Vec<f32> = hints.iter().map(width).collect();
    let one_row = widths.iter().sum::<f32>() + gap * 5.0 <= avail;
    let per_row = if one_row {
        hints.len()
    } else {
        hints.len() / 2
    };
    for (row, chunk) in hints.chunks(per_row).enumerate() {
        let w = &widths[row * per_row..][..chunk.len()];
        let mut x = cx - (w.iter().sum::<f32>() + gap * (chunk.len() - 1) as f32) / 2.0;
        let y = top + row as f32 * 30.0;
        for ((c, label), w) in chunk.iter().zip(w) {
            let used = keys::paint_caps(p, pos2(x, y), c);
            p.text(
                pos2(x + used + 7.0, y + 11.0),
                Align2::LEFT_CENTER,
                label,
                mono(11.5),
                MUTED,
            );
            x += w + gap;
        }
    }
}
