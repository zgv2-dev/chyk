use super::*;

/// The ring a control wears while Tab has focused it.
pub(super) fn focus_ring(p: &egui::Painter, r: Rect, radius: f32) {
    p.rect_stroke(
        r.expand(2.0),
        radius + 2.0,
        Stroke::new(2.0, GREEN.gamma_multiply(0.6)),
        egui::StrokeKind::Inside,
    );
}
