//! A number box for the output frame popover, drawn into a dropdown row or a segment.

use super::*;

/// A text box in `rect` for a number, committed on Enter or when it loses focus (Esc drops it).
/// Shows `shown`, or `hint` when that is `None`, and paints like a chosen option when `on`.
pub(super) fn number_box(
    ui: &mut Ui,
    rect: Rect,
    id: &str,
    shown: Option<String>,
    hint: &str,
    on: bool,
) -> Option<f64> {
    let id = ui.id().with(id);
    let focused = ui.memory(|m| m.has_focus(id));
    let mut buf = match focused {
        true => ui.data(|d| d.get_temp::<String>(id)).unwrap_or_default(),
        false => shown.unwrap_or_default(),
    };
    let edge = Stroke::new(1.0, GREEN.gamma_multiply(0.5));
    let (fill, stroke, color) = if on {
        (GREEN.gamma_multiply(0.11), edge, GREEN)
    } else if focused {
        (CARD_HI, edge, TEXT)
    } else {
        (Color32::TRANSPARENT, Stroke::new(1.0, LINE), TEXT)
    };
    ui.painter()
        .rect(rect, 6.0, fill, stroke, egui::StrokeKind::Inside);
    let edit = egui::TextEdit::singleline(&mut buf)
        .id(id)
        .frame(egui::Frame::NONE)
        .font(mono(12.0))
        .text_color(color)
        .hint_text(RichText::new(hint).font(mono(12.0)).color(MUTED))
        .horizontal_align(Align::Center)
        .vertical_align(Align::Center)
        .margin(vec2(4.0, 0.0));
    let resp = ui.put(rect, edit);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::Text);
    }
    ui.data_mut(|d| d.insert_temp(id, buf.clone()));
    let esc = ui.input(|i| i.key_pressed(Key::Escape));
    if !resp.lost_focus() || esc {
        return None;
    }
    let v: f64 = buf.trim().replace(',', ".").parse().ok()?;
    v.is_finite().then_some(v)
}
