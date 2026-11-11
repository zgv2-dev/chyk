//! The ffmpeg log in its own OS window: every command Chyk ran this session, newest last.

use super::log_entry::entry_ui;
use super::log_text::{entry_text, kind_label};
use super::*;
use ffmpeg::log::{self, Entry, Kind};

/// The kind filter's choices; `None` is All.
const KINDS: [Option<Kind>; 7] = [
    None,
    Some(Kind::Export),
    Some(Kind::Scan),
    Some(Kind::Estimate),
    Some(Kind::Thumbnails),
    Some(Kind::Frame),
    Some(Kind::Probe),
];

/// What the log window shows; the filter outlives closing it.
#[derive(Default)]
pub(super) struct LogView {
    /// The `log::generation()` `entries` were copied at; `None` until the window's first frame.
    seen: Option<u64>,
    entries: Vec<Entry>,
    /// Index into [`KINDS`].
    filter: usize,
    /// Width the filter and Copy took last frame, so the title can end before them.
    right: f32,
}

impl LogView {
    fn shown(&self) -> Vec<&Entry> {
        let kind = KINDS[self.filter];
        self.entries
            .iter()
            .filter(|e| kind.is_none_or(|k| e.kind == k))
            .collect()
    }
}

impl App {
    pub(super) fn log_window(&mut self, ctx: &egui::Context) {
        if !self.log_open {
            // Reopening makes a new window: set it up and copy the log afresh.
            self.log_view.seen = None;
            return;
        }
        let first = self.log_view.seen.is_none();
        // The copy is deep, so take it only when the log changed.
        let generation = log::generation();
        if self.log_view.seen != Some(generation) {
            self.log_view.entries = log::entries();
            self.log_view.seen = Some(generation);
        }
        let view = &mut self.log_view;
        let builder = egui::ViewportBuilder::default()
            .with_title("Chyk — ffmpeg log")
            .with_inner_size([880.0, 500.0])
            .with_min_inner_size([520.0, 300.0]);
        let id = egui::ViewportId::from_hash_of("chyk-ffmpeg-log");
        let close = ctx.show_viewport_immediate(id, builder, |ui, _| {
            if first {
                // Dark title bar like the main window's, whatever the system theme.
                let dark = egui::ViewportCommand::SetTheme(egui::SystemTheme::Dark);
                ui.ctx().send_viewport_cmd(dark);
            }
            // Keys pressed here reach only this window, so they can't close the video.
            let close = ui.input_mut(|i| {
                i.viewport().close_requested()
                    | i.consume_key(Modifiers::NONE, Key::Escape)
                    | i.consume_key(Modifiers::COMMAND, Key::W)
                    | i.consume_key(Modifiers::COMMAND, Key::L)
            });
            head(ui, view);
            body(ui, view);
            close
        });
        if close {
            self.log_open = false;
        }
    }
}

/// Title and read-only note, then the kind filter and Copy at the right, in reading order.
fn head(ui: &mut Ui, view: &mut LogView) {
    let bar = egui::Panel::top("log-head")
        .show_separator_line(false)
        .exact_size(40.0)
        .frame(egui::Frame::new().fill(PANEL).inner_margin(egui::Margin {
            left: 12,
            right: 8,
            top: 0,
            bottom: 0,
        }))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                let narrow = ui.available_width() < 640.0;
                let title_w = (ui.available_width() - view.right).max(0.0);
                ui.allocate_ui_with_layout(
                    vec2(title_w, ui.available_height()),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.set_min_width(title_w);
                        let title = RichText::new(t!("log.title")).font(bold(12.5)).color(TEXT);
                        ui.add(egui::Label::new(title).truncate());
                        let note = RichText::new(t!("log.readonly"))
                            .font(mono(11.0))
                            .color(MUTED);
                        ui.add(egui::Label::new(note).truncate());
                    },
                );
                let x = ui.cursor().min.x;
                filter(ui, view);
                let label = if narrow { "" } else { t!("meta.copy") };
                let copy =
                    button(ui, icon::COPY, label, false, GHOST).on_hover_text(t!("meta.copy"));
                if copy.clicked() {
                    let text: String = view.shown().into_iter().map(entry_text).collect();
                    ui.ctx().copy_text(text);
                }
                let right = ui.cursor().min.x - x;
                if (right - view.right).abs() > 0.5 {
                    view.right = right;
                    ui.ctx().request_repaint();
                }
            });
        });
    let r = bar.response.rect;
    ui.painter()
        .hline(r.x_range(), r.bottom(), Stroke::new(1.0, LINE));
}

fn filter(ui: &mut Ui, view: &mut LogView) {
    egui::Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(7.0)
        .inner_margin(egui::Margin::same(2))
        .show(ui, |ui| {
            // Without this the frame grows to the header row's full height.
            ui.set_height(SEG.height);
            ui.spacing_mut().item_spacing.x = 2.0;
            for (i, kind) in KINDS.into_iter().enumerate() {
                if button(ui, "", kind_label(kind), i == view.filter, SEG).clicked() {
                    view.filter = i;
                }
            }
        });
}

fn body(ui: &mut Ui, view: &LogView) {
    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(BG)
                .inner_margin(egui::Margin::symmetric(14, 6)),
        )
        .show(ui, |ui| {
            let shown = view.shown();
            if shown.is_empty() {
                let hint = RichText::new(t!("log.empty")).font(mono(11.0)).color(MUTED);
                ui.centered_and_justified(|ui| ui.label(hint));
                return;
            }
            // shortcut: lays out every shown entry each frame (this window redraws with the main
            // one); a full log (200 entries × 200 lines) costs ~5 ms in release. If that ever
            // stutters playback, draw only the visible rows with `show_viewport` and cached heights.
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .auto_shrink(false)
                .show(ui, |ui| {
                    for e in shown {
                        entry_ui(ui, e);
                    }
                });
        });
}
