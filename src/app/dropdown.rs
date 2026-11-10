use super::*;
use egui::{Frame, Popup, PopupAnchor, PopupCloseBehavior, PopupKind, RectAlign, SetOpenCommand};

/// Row height in the open list: the box's height inside its 3 pt padding.
const ROW: f32 = 30.0;

/// Choice box like a macOS pop-up button: the list opens over the box with the chosen row on it,
/// earlier options above and later ones below. `notes` are dim extras right of each option.
/// Returns the clicked index.
pub(super) fn dropdown(
    ui: &mut Ui,
    id: &str,
    options: &[String],
    notes: &[String],
    selected: usize,
) -> Option<usize> {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::click());
    let edge = if resp.hovered() {
        GREEN.gamma_multiply(0.5)
    } else {
        LINE
    };
    let p = ui.painter();
    p.rect(
        rect,
        8.0,
        CARD,
        Stroke::new(1.0, edge),
        egui::StrokeKind::Inside,
    );
    let label = &options[selected];
    let x = vec2(12.0, 0.0);
    p.text(
        rect.left_center() + x,
        Align2::LEFT_CENTER,
        label,
        mono(12.0),
        TEXT,
    );
    let caret = icon::CARET_UP_DOWN;
    p.text(
        rect.right_center() - x,
        Align2::RIGHT_CENTER,
        caret,
        mono(13.0),
        MUTED,
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, label));

    // The frame's 1 pt stroke and 3 pt margin put row 0 at top + 4; the chosen row lands on
    // the box's inner area at rect.top + 3. Kept inside the window when the list is tall.
    let screen = ui.ctx().content_rect();
    let h = options.len() as f32 * ROW + 8.0;
    let top = (rect.top() - 1.0 - selected as f32 * ROW).clamp(
        screen.top() + 8.0,
        (screen.bottom() - 8.0 - h).max(screen.top() + 8.0),
    );
    let anchor = PopupAnchor::Position(pos2(rect.left(), top));
    let mut chosen = None;
    Popup::new(ui.id().with(id), ui.ctx().clone(), anchor, ui.layer_id())
        // Tooltip order: the popover moves itself to the top of the foreground every frame.
        .kind(PopupKind::Tooltip)
        .align(RectAlign::BOTTOM_START)
        .align_alternatives(&[])
        .open_memory(resp.clicked().then_some(SetOpenCommand::Toggle))
        .close_behavior(PopupCloseBehavior::CloseOnClick)
        .frame(
            Frame::new()
                .fill(PANEL)
                .stroke(Stroke::new(1.0, LINE))
                .corner_radius(8.0)
                .inner_margin(3)
                .shadow(shadow()),
        )
        .show(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for (i, label) in options.iter().enumerate() {
                let size = vec2(rect.width() - 8.0, ROW);
                let (r, row) = ui.allocate_exact_size(size, Sense::click());
                let on = i == selected;
                let (fill, color) = if on {
                    (GREEN.gamma_multiply(0.11), GREEN)
                } else if row.hovered() {
                    (CARD_HI, TEXT)
                } else {
                    (Color32::TRANSPARENT, MUTED)
                };
                let p = ui.painter();
                p.rect_filled(r, 6.0, fill);
                let x = vec2(9.0, 0.0);
                p.text(
                    r.left_center() + x,
                    Align2::LEFT_CENTER,
                    label,
                    mono(12.0),
                    color,
                );
                if let Some(note) = notes.get(i) {
                    p.text(
                        r.right_center() - x,
                        Align2::RIGHT_CENTER,
                        note,
                        mono(11.0),
                        MUTED,
                    );
                }
                if row.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                row.widget_info(|| {
                    egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, on, label)
                });
                if row.clicked() {
                    chosen = Some(i);
                }
            }
        });
    chosen
}
