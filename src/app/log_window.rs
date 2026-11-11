//! The ffmpeg log in its own OS window: every command Chyk ran this session, newest last.

use super::log_entry::entry_ui;
use super::log_head::head;
use super::*;
use ffmpeg::log::{self, Entry, Kind};
use std::sync::{Arc, Mutex, PoisonError};

/// The kind filter's choices; `None` is All.
pub(super) const KINDS: [Option<Kind>; 7] = [
    None,
    Some(Kind::Export),
    Some(Kind::Scan),
    Some(Kind::Estimate),
    Some(Kind::Thumbnails),
    Some(Kind::Frame),
    Some(Kind::Probe),
];

/// The log window repaints for its own input, and for log changes at most this often.
const REFRESH: Duration = Duration::from_millis(100);

fn viewport() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("chyk-ffmpeg-log")
}

/// What the log window shows. It draws on its own passes (a deferred viewport), so the main
/// window shares this with it; the filter outlives closing it.
#[derive(Default)]
pub(super) struct LogView {
    /// The `log::generation()` `entries` were copied at; `None` until the window's first pass.
    seen: Option<u64>,
    /// When `entries` was copied.
    copied: Option<Instant>,
    entries: Vec<Entry>,
    /// Index into [`KINDS`].
    pub(super) filter: usize,
    /// Width the filter and Copy took last pass, so the title can end before them.
    pub(super) right: f32,
    /// The running export's share done, which the log itself doesn't know.
    export_done: Option<f32>,
    /// The window asked to close; the main window turns `log_open` off.
    closed: bool,
}

pub(super) type SharedLog = Arc<Mutex<LogView>>;

fn lock(view: &SharedLog) -> std::sync::MutexGuard<'_, LogView> {
    view.lock().unwrap_or_else(PoisonError::into_inner)
}

impl LogView {
    pub(super) fn shown(&self) -> Vec<&Entry> {
        let kind = KINDS[self.filter];
        self.entries
            .iter()
            .filter(|e| kind.is_none_or(|k| e.kind == k))
            .collect()
    }

    /// Copies the log when it changed, at most every [`REFRESH`]; otherwise looks again then.
    fn refresh(&mut self, ctx: &egui::Context) {
        let generation = log::generation();
        if self.seen == Some(generation) {
            return;
        }
        // egui runs a delayed pass one predicted frame early, so that it shows on time.
        let early = Duration::try_from_secs_f32(ctx.input(|i| i.predicted_dt)).unwrap_or_default();
        match self.copied.map(|at| at.elapsed()) {
            Some(e) if e + early < REFRESH && self.seen.is_some() => {
                ctx.request_repaint_after(REFRESH - e)
            }
            _ => {
                self.entries = log::entries();
                self.seen = Some(generation);
                self.copied = Some(Instant::now());
            }
        }
    }
}

impl App {
    /// The main window's side, every pass: keep the log window alive and wake it when the log moved.
    pub(super) fn log_window(&mut self, ctx: &egui::Context) {
        let mut view = lock(&self.log_view);
        if std::mem::take(&mut view.closed) {
            self.log_open = false;
        }
        if !self.log_open {
            // Reopening makes a new window: set it up and copy the log afresh.
            view.seen = None;
            view.entries = Vec::new();
            return;
        }
        view.export_done = self.job.as_ref().map(|_| self.progress);
        if view.seen.is_some_and(|s| s != log::generation()) {
            let wait = view
                .copied
                .map_or(Duration::ZERO, |at| REFRESH.saturating_sub(at.elapsed()));
            // Zero would be an immediate repaint, which egui runs twice.
            ctx.request_repaint_after_for(wait.max(Duration::from_millis(1)), viewport());
        }
        drop(view);
        let builder = egui::ViewportBuilder::default()
            .with_title("Chyk — ffmpeg log")
            .with_inner_size([880.0, 500.0])
            .with_min_inner_size([520.0, 300.0]);
        let shared = self.log_view.clone();
        ctx.show_viewport_deferred(viewport(), builder, move |ui, _| {
            draw(ui, &mut lock(&shared));
        });
    }
}

/// One pass of the log window, on its own input or a log change.
fn draw(ui: &mut Ui, view: &mut LogView) {
    if view.seen.is_none() {
        // Dark title bar like the main window's, whatever the system theme.
        let dark = egui::ViewportCommand::SetTheme(egui::SystemTheme::Dark);
        ui.ctx().send_viewport_cmd(dark);
    }
    view.refresh(ui.ctx());
    // Keys pressed here reach only this window, so they can't close the video.
    let close = ui.input_mut(|i| {
        i.viewport().close_requested()
            | i.consume_key(Modifiers::NONE, Key::Escape)
            | i.consume_key(Modifiers::COMMAND, Key::W)
            | i.consume_key(Modifiers::COMMAND, Key::L)
    });
    if close {
        view.closed = true;
        ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
    }
    head(ui, view);
    body(ui, view);
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
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .auto_shrink(false)
                .show(ui, |ui| {
                    for e in shown {
                        entry_ui(ui, e, view.export_done);
                    }
                });
        });
}
