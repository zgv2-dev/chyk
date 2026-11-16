//! The portable build's first run: a small frameless window with a progress bar while ffmpeg
//! and mpv unpack into `Chyk` next to the exe. Closes itself when done.

use super::*;
use std::io;

const SIZE: [f32; 2] = [380.0, 112.0];

/// Unpacks this build's libraries, showing progress, and returns once they're on disk.
#[cfg(bundled)]
pub fn run() -> io::Result<()> {
    let dir = crate::bundle::dir();
    run_with(move |progress| crate::bundle::unpack(&dir, progress))
}

/// Runs `work` on a background thread behind the window. `work` reports its share done (0 to 1).
pub fn run_with(
    work: impl FnOnce(&mut dyn FnMut(f32)) -> io::Result<()> + Send + 'static,
) -> io::Result<()> {
    let (tx, rx) = channel::<Msg>();
    let result = std::sync::Arc::new(std::sync::Mutex::new(None));
    let out = result.clone();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Chyk")
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../../assets/chyk.png"))
                    .expect("assets/chyk.png is a valid PNG"),
            )
            .with_inner_size(SIZE)
            .with_resizable(false)
            .with_decorations(false)
            .with_taskbar(true),
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "Chyk",
        options,
        Box::new(move |cc| {
            fonts(&cc.egui_ctx);
            theme(&cc.egui_ctx);
            let ctx = cc.egui_ctx.clone();
            std::thread::spawn(move || {
                let mut last = 0.0;
                let r = work(&mut |p| {
                    // Every whole percent is plenty for a bar this size.
                    if p - last >= 0.01 {
                        last = p;
                        let _ = tx.send(Msg::Progress(p));
                        ctx.request_repaint();
                    }
                });
                let _ = tx.send(Msg::Done(r));
                ctx.request_repaint();
            });
            Ok(Box::new(UnpackWindow {
                rx,
                progress: 0.0,
                out,
            }))
        }),
    )
    .map_err(|e| io::Error::other(e.to_string()))?;
    let done = result.lock().ok().and_then(|mut r| r.take());
    done.unwrap_or_else(|| Err(io::Error::other("closed before unpacking finished")))
}

/// From the unpacking thread: progress, then the result once every file is in place.
enum Msg {
    Progress(f32),
    Done(io::Result<()>),
}

struct UnpackWindow {
    rx: Receiver<Msg>,
    progress: f32,
    out: std::sync::Arc<std::sync::Mutex<Option<io::Result<()>>>>,
}

impl eframe::App for UnpackWindow {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Progress(p) => self.progress = p.min(1.0),
                Msg::Done(result) => {
                    self.progress = 1.0;
                    if let Ok(mut out) = self.out.lock() {
                        *out = Some(result);
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        #[cfg(windows)]
        chrome::round_corners();
        // No settings here yet: the size Auto picks.
        set_ui_size(ui.ctx(), crate::settings::UiSize::Auto);
        follow_ui_size(ui.ctx(), SIZE.into());
        // Not closable halfway: the libraries would be missing.
        if ui.ctx().input(|i| i.viewport().close_requested())
            && self.out.lock().is_ok_and(|o| o.is_none())
        {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        let full = ui.max_rect();
        let p = ui.painter();
        p.rect_filled(full, 0.0, Color32::from_rgb(1, 4, 2));
        p.rect_stroke(
            full,
            0.0,
            Stroke::new(1.0, Color32::from_rgb(26, 61, 41)),
            egui::StrokeKind::Inside,
        );
        let drag = ui.interact(full, ui.id().with("unpack-drag"), Sense::click_and_drag());
        if drag.drag_started() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        let inner = full.shrink2(vec2(22.0, 20.0));
        let brand = Rect::from_min_size(inner.min, vec2(inner.width(), 26.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(brand), |ui| {
            ui.horizontal_centered(|ui| {
                logo(ui);
                ui.label(RichText::new("C H Y K").font(bold(14.0)).color(GREEN));
            });
        });
        let p = ui.painter();
        let text_y = inner.top() + 46.0;
        p.text(
            pos2(inner.left(), text_y),
            Align2::LEFT_CENTER,
            t!("unpack.title"),
            mono(11.5),
            MUTED,
        );
        p.text(
            pos2(inner.right(), text_y),
            Align2::RIGHT_CENTER,
            format!("{:.0}%", self.progress * 100.0),
            mono(11.5),
            GREEN,
        );
        let track = Rect::from_min_size(
            pos2(inner.left(), inner.bottom() - 8.0),
            vec2(inner.width(), 6.0),
        );
        p.rect(
            track,
            3.0,
            CARD,
            Stroke::new(1.0, LINE),
            egui::StrokeKind::Inside,
        );
        let fill = Rect::from_min_size(track.min, vec2(track.width() * self.progress, 6.0));
        p.add(glow(GREEN.gamma_multiply(0.4), 10).as_shape(fill, 3));
        p.rect_filled(fill, 3.0, GREEN);
    }
}
