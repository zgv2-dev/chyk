use super::*;

/// The video canvas: black with a faint green center and a quiet dot grid.
pub(super) fn canvas(ui: &Ui, rect: Rect) {
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0.0, BG);
    let core = Rect::from_center_size(rect.center(), rect.size() * 0.45);
    p.add(glow(Color32::from_rgb(4, 17, 10), 255).as_shape(core, 200));
    let dot = GREEN.gamma_multiply(0.07);
    let mut y = rect.min.y + 12.0;
    while y < rect.max.y {
        let mut x = rect.min.x + 12.0;
        while x < rect.max.x {
            p.circle_filled(pos2(x, y), 1.0, dot);
            x += 24.0;
        }
        y += 24.0;
    }
}

/// A few streams of digital rain falling behind the start screen, fading out behind the content box `clear`.
/// Deterministic in `time`, so it needs no state: each stream picks a new column every pass.
pub(super) fn rain(p: &egui::Painter, rect: Rect, clear: Rect, time: f64) {
    const GLYPHS: &[char] = &[
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F', ':', '.',
        '=', '*', '+', '<', '>', '/',
    ];
    const STREAMS: u64 = 7;
    const LINE: f32 = 19.0;
    // splitmix64: a cheap, well-spread hash of small integers.
    let hash = |mut x: u64| {
        x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
        x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        x ^ (x >> 31)
    };
    let font = mono(15.0);
    let cols = ((rect.width() / 22.0) as u64).max(1);
    for s in 0..STREAMS {
        let speed = 60.0 + (hash(s * 3) % 70) as f32;
        let len = 8 + hash(s * 3 + 1) % 12;
        let tail = len as f32 * LINE;
        let travel = rect.height() + tail;
        let phase = time as f32 * speed + (hash(s * 3 + 2) % 10_000) as f32;
        let pass = (phase / travel) as u64;
        let head = rect.min.y + phase % travel;
        let x = rect.min.x + 11.0 + (hash(s * 7919 + pass) % cols) as f32 * 22.0;
        // Glyphs sit on a grid and flicker to a new character a few times a second.
        let flicker = (time * 6.0) as u64;
        for k in 0..len {
            let y = head - k as f32 * LINE;
            if !rect.y_range().contains(y) {
                continue;
            }
            // Fully clear over the content box, easing to full strength 140 pt away from it.
            let d = pos2(x, y + LINE / 2.0).distance(clear.clamp(pos2(x, y + LINE / 2.0)));
            let t = (d / 140.0).clamp(0.0, 1.0);
            let mask = t * t * (3.0 - 2.0 * t);
            let fade = 1.0 - k as f32 / len as f32;
            let a = 0.28 * fade * mask;
            if a < 0.005 {
                continue;
            }
            let row = ((y - rect.min.y) / LINE) as u64;
            let g = GLYPHS[(hash(x as u64 * 131 + row * 7 + flicker / (1 + k % 3))
                % GLYPHS.len() as u64) as usize];
            let color = if k == 0 {
                Color32::from_rgb(217, 255, 233)
            } else {
                GREEN
            };
            p.text(
                pos2(x, y),
                Align2::CENTER_TOP,
                g,
                font.clone(),
                color.gamma_multiply(a),
            );
        }
    }
}
