//! One ffmpeg log entry drawn: when, what kind, how it ended, the command and what ffmpeg printed.

use super::log_text::*;
use super::*;
use eframe::egui::text::{LayoutJob, TextFormat};
use ffmpeg::log::{Entry, Kind};

/// Log text: 11 pt at 1.55 line height.
const TEXT_PT: f32 = 11.0;
const LINE_H: f32 = 17.0;

fn fmt(color: Color32) -> TextFormat {
    TextFormat {
        font_id: mono(TEXT_PT),
        color,
        line_height: Some(LINE_H),
        ..Default::default()
    }
}

fn chip(ui: &mut Ui, kind: Kind) {
    let on = kind == Kind::Export;
    let (text, edge) = if on {
        (GREEN, GREEN.gamma_multiply(0.4))
    } else {
        (MUTED, LINE)
    };
    egui::Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, edge))
        .corner_radius(4.0)
        .inner_margin(egui::Margin::symmetric(6, 0))
        .show(ui, |ui| {
            ui.set_height(LINE_H - 2.0);
            let label = kind_label(Some(kind)).to_lowercase();
            ui.label(RichText::new(label).font(mono(10.0)).color(text));
        });
}

/// Right-aligned result: laid out right to left, so the text goes before its mark. `live`
/// replaces "running" for an export.
fn status_ui(ui: &mut Ui, s: &Status, live: Option<String>) {
    let (text, color) = match (s, live) {
        (Status::Running, Some(live)) => (live, GREEN),
        _ => status_text(s),
    };
    ui.spacing_mut().item_spacing.x = 5.0;
    ui.label(RichText::new(text).font(mono(10.5)).color(color));
    let (glyph, color) = match s {
        Status::Running => {
            let (r, _) = ui.allocate_exact_size(vec2(6.0, 6.0), Sense::hover());
            ui.painter().circle_filled(r.center(), 3.0, GREEN);
            return;
        }
        Status::Done(..) => (icon::CHECK, GREEN),
        Status::Failed(_) => (icon::X, RED),
        Status::Waiting | Status::Probed(_) => return,
    };
    ui.label(RichText::new(glyph).font(mono(12.0)).color(color));
}

/// `export_done`: the running export's share done, for its live result.
pub(super) fn entry_ui(ui: &mut Ui, e: &Entry, export_done: Option<f32>) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let at = RichText::new(wall_clock(e.at))
            .font(mono(TEXT_PT))
            .color(MUTED);
        ui.add(egui::Label::new(at).selectable(true));
        chip(ui, e.kind);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let live = (e.kind == Kind::Export)
                .then(|| progress_text(e.progress.as_deref(), export_done))
                .flatten();
            status_ui(ui, &status(e), live)
        });
    });
    // Long paths and filter graphs have no spaces to wrap at.
    let mut cmd = LayoutJob::default();
    cmd.wrap.break_anywhere = true;
    if let Some(r) = e.runs.first() {
        cmd.append("$ ", 0.0, fmt(GREEN));
        cmd.append(&r.args, 0.0, fmt(TEXT));
        cmd.append(&more(e), 0.0, fmt(MUTED));
        ui.add(egui::Label::new(cmd).wrap().selectable(true));
    }
    let mut out = LayoutJob::default();
    out.wrap.break_anywhere = true;
    let lines = e.runs.iter().flat_map(|r| {
        let bad = failed(e.kind, r);
        r.output.iter().map(move |l| (l, is_error(l, bad)))
    });
    for (l, err) in lines.chain(e.progress.iter().map(|p| (p, false))) {
        let sep = if out.text.is_empty() { "" } else { "\n" };
        out.append(
            &format!("{sep}{l}"),
            0.0,
            fmt(if err { RED } else { MUTED }),
        );
    }
    if !out.text.is_empty() {
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 14,
                top: 2,
                ..Default::default()
            })
            .show(ui, |ui| {
                ui.add(egui::Label::new(out).wrap().selectable(true))
            });
    }
    ui.add_space(10.0);
    let y = ui.cursor().min.y - ui.spacing().item_spacing.y / 2.0;
    let x = ui.max_rect().x_range();
    let dash = egui::Shape::dashed_line(
        &[pos2(x.min, y), pos2(x.max, y)],
        Stroke::new(1.0, LINE.gamma_multiply(0.8)),
        3.0,
        3.0,
    );
    ui.painter().extend(dash);
}
