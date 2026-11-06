//! The updater's header button, left of Export (on the start screen, at the title bar's end).
//! Exactly Export's height: the label over the version tag, or a progress chip while downloading.
//! When the header is short of room it shrinks to an icon, or the chip to its bar and cancel.

use super::*;
use crate::update::{self, State};

const H: f32 = 34.0;
const CHIP_W: f32 = 214.0;
const CHIP_COMPACT_W: f32 = 104.0;

impl App {
    /// `need`: width the header's middle section wants beside this button.
    pub(super) fn update_button(&mut self, ui: &mut Ui, ctx: &egui::Context, need: f32) {
        let room = ui.available_width() - need;
        let (glyph, label, tag, ready) = match &self.update.state {
            State::None => return,
            State::Downloading { tag, .. } => {
                let tag = tag.clone();
                return self.update_progress(ui, &tag, room < CHIP_W);
            }
            State::Available { tag, .. } => {
                (icon::ARROW_CIRCLE_UP, t!("update.available"), tag, false)
            }
            State::Ready { tag, .. } => (icon::ARROW_CLOCKWISE, t!("update.now"), tag, true),
            State::Updating { tag, .. } => {
                (icon::ARROW_CLOCKWISE, t!("update.updating"), tag, true)
            }
        };
        let tag = tag.clone();
        let busy = matches!(self.update.state, State::Updating { .. });
        let blocked = busy || (ready && self.job.is_some());
        let p = ui.painter();
        let icon_g = p.layout_no_wrap(glyph.into(), mono(16.0), Color32::PLACEHOLDER);
        let label_g = p.layout_no_wrap(label.into(), bold(11.5), Color32::PLACEHOLDER);
        let tag_g = p.layout_no_wrap(tag.clone(), mono(9.5), Color32::PLACEHOLDER);
        let full_w = 11.0 + icon_g.size().x + 9.0 + label_g.size().x.max(tag_g.size().x) + 14.0;
        let compact = room < full_w;
        let (rect, resp) =
            ui.allocate_exact_size(vec2(if compact { H } else { full_w }, H), Sense::click());
        let hot = resp.hovered() && !blocked;
        let p = ui.painter();
        let (fill, stroke) = if ready {
            p.add(glow(GREEN.gamma_multiply(0.18), 18).as_shape(rect, 8));
            (
                GREEN.gamma_multiply(if hot { 0.2 } else { 0.14 }),
                GREEN.gamma_multiply(0.6),
            )
        } else {
            (
                GREEN.gamma_multiply(if hot { 0.11 } else { 0.05 }),
                GREEN_DIM,
            )
        };
        p.rect(
            rect,
            8.0,
            fill,
            Stroke::new(1.0, stroke),
            egui::StrokeKind::Inside,
        );
        let color = if blocked { MUTED } else { GREEN };
        let y = rect.center().y;
        if compact {
            p.galley_with_override_text_color(rect.center() - icon_g.size() / 2.0, icon_g, color);
        } else {
            let icon_w = icon_g.size().x;
            p.galley_with_override_text_color(
                pos2(rect.left() + 11.0, y - icon_g.size().y / 2.0),
                icon_g,
                color,
            );
            let x = rect.left() + 11.0 + icon_w + 9.0;
            let (lh, th) = (label_g.size().y, tag_g.size().y);
            let top = y - (lh + th - 2.0) / 2.0;
            p.galley_with_override_text_color(pos2(x, top), label_g, color);
            let tag_c = if ready && !blocked { GREEN_DIM } else { MUTED };
            p.galley_with_override_text_color(pos2(x, top + lh - 2.0), tag_g, tag_c);
        }
        if hot {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if busy {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, !blocked, label));
        let mut tip = match &self.update.state {
            State::Available { manual: true, .. } => t!("tip.update_manual", tag = tag),
            State::Available { .. } => t!("tip.update_available", tag = tag),
            State::Updating { .. } => format!("{label} {tag}"),
            _ if blocked => t!("tip.update_wait").to_string(),
            _ => t!("tip.update_now", tag = tag),
        };
        if compact && !busy {
            tip = format!("{label} · {tag}\n{tip}");
        }
        if resp.on_hover_text(tip).clicked() && !blocked {
            self.update_clicked(ctx);
        }
    }

    /// Download chip: icon, version, bar and percent, then cancel. Compact: just the bar and cancel.
    fn update_progress(&mut self, ui: &mut Ui, tag: &str, compact: bool) {
        let w = if compact { CHIP_COMPACT_W } else { CHIP_W };
        let (rect, resp) = ui.allocate_exact_size(vec2(w, H), Sense::hover());
        let frac = self.update.progress();
        let pct = format!("{:.0}%", frac * 100.0);
        let p = ui.painter();
        p.rect(
            rect,
            8.0,
            CARD,
            Stroke::new(1.0, LINE),
            egui::StrokeKind::Inside,
        );
        let y = rect.center().y;
        let left = |x: f32| pos2(rect.left() + x, y);
        let bar = if compact {
            Rect::from_min_size(pos2(rect.left() + 12.0, y - 2.0), vec2(w - 52.0, 4.0))
        } else {
            p.text(
                left(12.0),
                Align2::LEFT_CENTER,
                icon::DOWNLOAD_SIMPLE,
                mono(15.0),
                GREEN,
            );
            p.text(left(34.0), Align2::LEFT_CENTER, tag, mono(11.5), TEXT);
            p.text(left(164.0), Align2::LEFT_CENTER, &pct, mono(11.0), MUTED);
            Rect::from_min_size(pos2(rect.left() + 92.0, y - 2.0), vec2(64.0, 4.0))
        };
        p.rect_filled(bar, 2.0, LINE);
        progress_fill(p, bar, frac, 2.0, GREEN);
        if compact {
            resp.on_hover_text(format!("{tag} · {pct}"));
        }
        let x_rect = Rect::from_center_size(pos2(rect.right() - 16.0, y), vec2(24.0, 24.0));
        let x = ui.interact(x_rect, ui.id().with("update_cancel"), Sense::click());
        let x_c = if x.hovered() { TEXT } else { MUTED };
        ui.painter().text(
            x_rect.center(),
            Align2::CENTER_CENTER,
            icon::X,
            mono(14.0),
            x_c,
        );
        if x.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if x.on_hover_text(t!("tip.update_cancel")).clicked() {
            self.update.cancel();
        }
    }

    fn update_clicked(&mut self, ctx: &egui::Context) {
        match &self.update.state {
            State::Available { manual: true, .. } => {
                let page = format!("{}/releases/latest", update::REPO);
                if let Err(e) = std::process::Command::new("xdg-open").arg(page).spawn() {
                    self.status = Some((t!("status.update_failed", error = e), true));
                }
            }
            State::Available { .. } => self.update.download(),
            State::Ready { .. } => self.update_now(ctx),
            _ => {}
        }
    }

    /// Swap in the download and reopen with the same video and edits. Nothing to confirm: the
    /// session comes back as it was. The swap runs in the background; poll closes Chyk after.
    fn update_now(&mut self, ctx: &egui::Context) {
        let mut args = Vec::new();
        if let (Some(s), Some(file)) = (self.session(), update::session_file())
            && std::fs::create_dir_all(file.parent().unwrap_or(&file)).is_ok()
            && std::fs::write(&file, s.to_text()).is_ok()
        {
            args = vec!["--restore".into(), file.into_os_string()];
        }
        self.update.update_now(args);
        ctx.request_repaint();
    }
}
