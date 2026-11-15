//! The setup and uninstall window. Chyk.exe shows it instead of the editor when it starts
//! outside an install, or with `--uninstall`. A rail of steps on the left, the page on the right,
//! buttons along the bottom. The work runs in `crate::setup` on a background thread.

use super::*;
use crate::setup::{self, Choices, Kind, Mode, Msg, Scope};

const SIZE: [f32; 2] = [820.0, 520.0];
const HEAD_H: f32 = 40.0;
const FOOT_H: f32 = 58.0;
const RAIL_W: f32 = 210.0;
const PAD: f32 = 34.0;
const ROW_H: f32 = 50.0;
const WARN: Color32 = Color32::from_rgb(232, 196, 106);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Page {
    Welcome,
    Options,
    Installing,
    Done,
    Uninstall,
    Removing,
}

impl Page {
    /// Its place in the rail's steps.
    fn step(self) -> usize {
        match self {
            Self::Welcome | Self::Uninstall | Self::Removing => 0,
            Self::Options => 1,
            Self::Installing => 2,
            Self::Done => 3,
        }
    }
}

pub struct SetupWindow {
    mode: Mode,
    title: String,
    page: Page,
    choices: Choices,
    keep_settings: bool,
    delete_download: bool,
    lines: Vec<(Kind, String)>,
    progress: f32,
    job: Option<Receiver<Msg>>,
    installed: Option<PathBuf>,
    failed: bool,
    /// The folder was picked with Change…, so switching the scope keeps it.
    custom_dir: bool,
    /// The UAC prompt was declined.
    denied: bool,
    /// The registered install is for all users: uninstalling needs admin rights.
    uninstall_scope: Option<Scope>,
}

/// Opens the window and runs it until it closes.
pub fn run(mode: Mode) -> eframe::Result {
    let title = match mode {
        Mode::Install | Mode::InstallAllUsers(_) => t!("setup.title", version = setup::VERSION),
        Mode::Uninstall | Mode::UninstallNow { .. } => t!("setup.title_uninstall").to_owned(),
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(&title)
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../../assets/chyk.png"))
                    .expect("assets/chyk.png is a valid PNG"),
            )
            .with_inner_size(SIZE)
            .with_resizable(false)
            .with_maximize_button(false)
            .with_decorations(!chrome::CUSTOM),
        ..Default::default()
    };
    eframe::run_native(
        "Chyk",
        options,
        Box::new(move |cc| {
            fonts(&cc.egui_ctx);
            theme(&cc.egui_ctx);
            Ok(Box::new(SetupWindow::new(mode, title)))
        }),
    )
}

impl SetupWindow {
    fn new(mode: Mode, title: String) -> Self {
        let mut choices = Choices::default();
        let mut keep_settings = true;
        let page = match &mode {
            Mode::Install => Page::Welcome,
            Mode::InstallAllUsers(dir) => {
                choices.scope = Scope::Machine;
                choices.dir = dir.clone();
                Page::Options
            }
            Mode::Uninstall => Page::Uninstall,
            Mode::UninstallNow {
                keep_settings: keep,
            } => {
                keep_settings = *keep;
                // Started on the first frame, once there is a context to repaint.
                Page::Removing
            }
        };
        Self {
            mode,
            title,
            page,
            custom_dir: matches!(page, Page::Options),
            denied: false,
            uninstall_scope: setup::installed_scope(),
            choices,
            keep_settings,
            delete_download: true,
            lines: Vec::new(),
            progress: 0.0,
            job: None,
            installed: None,
            failed: false,
        }
    }

    fn start(&mut self, ctx: &egui::Context, page: Page) {
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        let choices = self.choices.clone();
        let keep = self.keep_settings;
        std::thread::spawn(move || {
            let (wtx, wrx) = channel();
            let worker = std::thread::spawn(move || match page {
                Page::Installing => setup::install(choices, wtx),
                _ => setup::uninstall(keep, wtx),
            });
            // Forward each message and wake the window, so the log fills in as steps finish.
            for msg in wrx {
                let _ = tx.send(msg);
                ctx.request_repaint();
            }
            // A panic sends no Done: report it, or the window would wait for good.
            if worker.join().is_err() {
                let _ = tx.send(Msg::Line(Kind::Fail, t!("setup.crashed").to_owned()));
                let _ = tx.send(Msg::Done(None));
                ctx.request_repaint();
            }
        });
        self.page = page;
        self.job = Some(rx);
    }

    fn poll(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.job else { return };
        loop {
            let msg = match rx.try_recv() {
                Ok(msg) => msg,
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                // The worker is gone without a Done.
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.lines
                        .push((Kind::Fail, t!("setup.crashed").to_owned()));
                    Msg::Done(None)
                }
            };
            match msg {
                Msg::Line(kind, text) => self.lines.push((kind, text)),
                Msg::Progress(p) => self.progress = p,
                Msg::Done(exe) => {
                    self.failed = exe.is_none();
                    self.installed = exe;
                    self.job = None;
                    match self.page {
                        Page::Installing if !self.failed => self.page = Page::Done,
                        // Uninstalled: the cleanup runs once this window is gone. On failure the
                        // window stays so the error can be read.
                        Page::Removing if !self.failed => {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close)
                        }
                        _ => {}
                    }
                    return;
                }
            }
        }
    }

    fn head(&mut self, ui: &mut Ui, r: Rect) {
        let p = ui.painter();
        p.rect_filled(r, 0.0, PANEL);
        p.hline(r.x_range(), r.bottom(), Stroke::new(1.0, LINE));
        p.text(
            pos2(r.left() + 14.0, r.center().y),
            Align2::LEFT_CENTER,
            &self.title,
            bold(12.5),
            TEXT,
        );
        let drag = ui.interact(r, ui.id().with("setup-title"), Sense::click_and_drag());
        if drag.drag_started() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        let buttons = Rect::from_min_max(pos2(r.right() - 80.0, r.top()), r.max);
        ui.scope_builder(egui::UiBuilder::new().max_rect(buttons), |ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let busy = self.job.is_some();
                window_buttons(
                    ui,
                    [chrome::Button::Close, chrome::Button::Minimize]
                        .iter()
                        .filter(|b| !busy || **b != chrome::Button::Close),
                );
            });
        });
    }

    fn rail(&self, ui: &mut Ui, r: Rect) {
        let p = ui.painter();
        p.rect_filled(r, 0.0, PANEL);
        p.vline(r.right(), r.y_range(), Stroke::new(1.0, LINE));
        let brand = Rect::from_min_size(r.min + vec2(18.0, 22.0), vec2(r.width() - 36.0, 30.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(brand), |ui| {
            ui.horizontal_centered(|ui| {
                logo(ui);
                ui.label(RichText::new("C H Y K").font(bold(16.0)).color(GREEN));
            });
        });
        let steps: Vec<&str> = match self.mode {
            Mode::Install | Mode::InstallAllUsers(_) => vec![
                t!("setup.step_welcome"),
                t!("setup.step_options"),
                t!("setup.step_install"),
                t!("setup.step_done"),
            ],
            Mode::Uninstall | Mode::UninstallNow { .. } => vec![t!("setup.uninstall")],
        };
        let at = self.page.step();
        let p = ui.painter();
        for (i, label) in steps.iter().enumerate() {
            let row = Rect::from_min_size(
                pos2(r.left() + 18.0, r.top() + 84.0 + i as f32 * 38.0),
                vec2(r.width() - 36.0, 34.0),
            );
            let dot = pos2(row.left() + 19.0, row.center().y);
            let (text, ring) = if i == at {
                p.rect_filled(row, 7.0, GREEN.gamma_multiply(0.1));
                p.rect_filled(
                    Rect::from_min_size(pos2(row.left(), row.top() + 9.0), vec2(3.0, 16.0)),
                    2.0,
                    GREEN,
                );
                p.add(
                    glow(GREEN.gamma_multiply(0.5), 10)
                        .as_shape(Rect::from_center_size(dot, vec2(12.0, 12.0)), 6),
                );
                p.circle_filled(dot, 4.0, GREEN);
                (GREEN, GREEN)
            } else if i < at {
                p.circle_filled(dot, 9.0, GREEN.gamma_multiply(0.12));
                p.text(dot, Align2::CENTER_CENTER, icon::CHECK, mono(10.0), GREEN);
                (MUTED, GREEN_DIM)
            } else {
                (LINE.gamma_multiply(2.0), LINE.gamma_multiply(2.0))
            };
            p.circle_stroke(dot, 9.0, Stroke::new(1.0, ring));
            p.text(
                pos2(row.left() + 40.0, row.center().y),
                Align2::LEFT_CENTER,
                *label,
                mono(12.5),
                text,
            );
        }
        p.text(
            pos2(r.left() + 18.0, r.bottom() - 18.0),
            Align2::LEFT_BOTTOM,
            format!("v{} · x64", setup::VERSION),
            mono(10.5),
            MUTED.gamma_multiply(0.6),
        );
    }

    fn page(&mut self, ui: &mut Ui, r: Rect) {
        let inner = r.shrink2(vec2(PAD, 30.0));
        match self.page {
            Page::Welcome => self.welcome(ui, inner),
            Page::Options => self.options(ui, inner),
            Page::Installing | Page::Removing => self.working(ui, inner),
            Page::Done => self.done(ui, inner),
            Page::Uninstall => self.uninstall(ui, inner),
        }
    }

    fn welcome(&mut self, ui: &mut Ui, r: Rect) {
        let y = heading(ui, r, t!("setup.welcome"));
        ui.painter().text(
            pos2(r.left(), y + 2.0),
            Align2::LEFT_TOP,
            t!("setup.specs", version = setup::VERSION, size = exe_mb()),
            mono(11.5),
            MUTED,
        );
        // Who it's for: the folder follows unless it was picked by hand.
        let was = self.choices.scope;
        let mut scope = was;
        radios(
            ui,
            pos2(r.left(), y + 34.0),
            r.width(),
            &mut scope,
            [
                (icon::USER, t!("setup.scope_user"), Scope::User),
                (icon::USERS_THREE, t!("setup.scope_all"), Scope::Machine),
            ],
        );
        if scope != was {
            self.set_scope(scope);
        }
        // Install folder, at the bottom of the page.
        let row = Rect::from_min_max(pos2(r.left(), r.bottom() - 28.0), r.max);
        let p = ui.painter();
        let label = p.layout_no_wrap(t!("setup.location").to_uppercase(), mono(11.0), MUTED);
        let lw = label.size().x;
        p.galley(
            pos2(row.left(), row.center().y - label.size().y / 2.0),
            label,
            MUTED,
        );
        let change_w = 120.0;
        let field = Rect::from_min_max(
            pos2(row.left() + lw + 12.0, row.top()),
            pos2(row.right() - change_w - 10.0, row.bottom()),
        );
        p.rect(
            field,
            7.0,
            PANEL,
            Stroke::new(1.0, LINE),
            egui::StrokeKind::Inside,
        );
        if self.denied {
            p.text(
                pos2(field.left(), row.top() - 10.0),
                Align2::LEFT_BOTTOM,
                t!("setup.elevate_denied"),
                mono(11.0),
                RED,
            );
        }
        let mut job = egui::text::LayoutJob::single_section(
            self.choices.dir.display().to_string(),
            egui::TextFormat::simple(mono(11.0), TEXT),
        );
        job.wrap = egui::text::TextWrapping::truncate_at_width(field.width() - 20.0);
        let g = p.layout_job(job);
        p.galley(
            pos2(field.left() + 10.0, field.center().y - g.size().y / 2.0),
            g,
            TEXT,
        );
        let at = Rect::from_min_max(pos2(row.right() - change_w, row.top()), row.max);
        ui.scope_builder(egui::UiBuilder::new().max_rect(at), |ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if button(ui, icon::FOLDER_OPEN, t!("setup.change"), false, GHOST).clicked()
                    && let Some(dir) = rfd::FileDialog::new().pick_folder()
                {
                    self.choices.dir = setup::folder_for(&dir);
                    self.custom_dir = true;
                    // A folder only an administrator can write to (Program Files) means all users.
                    if self.choices.scope == Scope::User && !setup::writable(&self.choices.dir) {
                        self.choices.scope = Scope::Machine;
                    }
                }
            });
        });
    }

    fn set_scope(&mut self, scope: Scope) {
        self.choices.scope = scope;
        self.denied = false;
        if !self.custom_dir
            && let Some(dir) = setup::default_dir(scope)
        {
            self.choices.dir = dir;
        }
    }

    /// Next needs administrator rights first: an all-users install from a normal setup.
    fn needs_elevation(&self) -> bool {
        self.choices.scope == Scope::Machine && !setup::elevated()
    }

    /// Restarts setup elevated for an all-users install into the chosen folder, on Options.
    fn elevate_install(&mut self, ctx: &egui::Context) {
        let args = [
            "--setup".into(),
            "--all-users".into(),
            self.choices.dir.clone().into_os_string(),
        ];
        match setup::restart_elevated(&args) {
            Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Err(_) => self.denied = true,
        }
    }

    /// Restarts the uninstall elevated, removing at once.
    fn elevate_uninstall(&mut self, ctx: &egui::Context) {
        let mut args = vec!["--uninstall".into(), "--confirm".into()];
        if self.keep_settings {
            args.push("--keep-settings".into());
        }
        match setup::restart_elevated(&args) {
            Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Err(_) => self.denied = true,
        }
    }

    fn options(&mut self, ui: &mut Ui, r: Rect) {
        let y = heading(ui, r, t!("setup.options"));
        let c = &mut self.choices;
        let rows: [(&str, &str, &mut bool); 3] = [
            (icon::CURSOR_CLICK, t!("settings.explorer"), &mut c.menu),
            (icon::DESKTOP, t!("setup.desktop"), &mut c.desktop),
            (
                icon::ARROW_CIRCLE_UP,
                t!("settings.check_updates"),
                &mut c.updates,
            ),
        ];
        card(ui, pos2(r.left(), y + 14.0), r.width(), rows);
    }

    fn working(&mut self, ui: &mut Ui, r: Rect) {
        let title = match self.page {
            Page::Removing if self.failed => t!("setup.uninstall_failed"),
            Page::Removing => t!("setup.removing"),
            _ if self.failed => t!("setup.failed"),
            _ => t!("setup.installing"),
        };
        let y = heading(ui, r, title) + 14.0;
        let p = ui.painter();
        let track = Rect::from_min_size(pos2(r.left(), y), vec2(r.width() - 52.0, 8.0));
        p.rect(
            track,
            4.0,
            CARD,
            Stroke::new(1.0, LINE),
            egui::StrokeKind::Inside,
        );
        let fill = Rect::from_min_size(track.min, vec2(track.width() * self.progress, 8.0));
        let bar = if self.failed { RED } else { GREEN };
        p.add(glow(bar.gamma_multiply(0.4), 12).as_shape(fill, 4));
        p.rect_filled(fill, 4.0, bar);
        p.text(
            pos2(r.right(), track.center().y),
            Align2::RIGHT_CENTER,
            format!("{:.0}%", self.progress * 100.0),
            mono(13.0),
            bar,
        );
        let log = Rect::from_min_max(pos2(r.left(), y + 26.0), pos2(r.right(), r.bottom()));
        let mut ly = log.top() + 12.0;
        let wrap = log.width() - 52.0;
        let mut shapes = Vec::new();
        for (kind, text) in &self.lines {
            let (glyph, color) = match kind {
                Kind::Ok => (icon::CHECK, GREEN),
                Kind::Warn => (icon::WARNING, WARN),
                Kind::Fail => (icon::X_CIRCLE, RED),
            };
            let text_color = if *kind == Kind::Ok { MUTED } else { color };
            let g = p.layout(text.clone(), mono(11.0), text_color, wrap);
            shapes.push((pos2(log.left() + 14.0, ly), glyph, color, g.clone()));
            ly += g.size().y + 8.0;
        }
        let card_r = Rect::from_min_max(log.min, pos2(log.right(), (ly + 4.0).min(log.bottom())));
        if !shapes.is_empty() {
            p.rect(
                card_r,
                9.0,
                CARD,
                Stroke::new(1.0, LINE),
                egui::StrokeKind::Inside,
            );
        }
        for (at, glyph, color, g) in shapes {
            p.text(
                at + vec2(0.0, 1.0),
                Align2::LEFT_TOP,
                glyph,
                mono(12.0),
                color,
            );
            p.galley(at + vec2(24.0, 0.0), g, TEXT);
        }
    }

    fn done(&mut self, ui: &mut Ui, r: Rect) {
        let p = ui.painter();
        let c = pos2(r.center().x, r.top() + 42.0);
        p.add(
            glow(GREEN.gamma_multiply(0.35), 24)
                .as_shape(Rect::from_center_size(c, vec2(64.0, 64.0)), 32),
        );
        p.circle(c, 32.0, PANEL, Stroke::new(1.0, GREEN.gamma_multiply(0.5)));
        p.text(c, Align2::CENTER_CENTER, icon::CHECK, mono(28.0), GREEN);
        p.text(
            pos2(r.center().x, c.y + 62.0),
            Align2::CENTER_CENTER,
            t!("setup.done"),
            bold(21.0),
            TEXT,
        );
        let mut y = c.y + 100.0;
        if self.downloaded().is_some() {
            card(
                ui,
                pos2(r.left(), y),
                r.width(),
                [(
                    icon::TRASH,
                    t!("setup.delete_download"),
                    &mut self.delete_download,
                )],
            );
            y += ROW_H + 16.0;
        }
        // Warnings from the install (or a failed launch) stay readable here.
        let p = ui.painter();
        for (kind, text) in self.lines.iter().filter(|(k, _)| *k != Kind::Ok) {
            let (glyph, color) = if *kind == Kind::Warn {
                (icon::WARNING, WARN)
            } else {
                (icon::X_CIRCLE, RED)
            };
            let g = p.layout(text.clone(), mono(11.0), color, r.width() - 24.0);
            p.text(
                pos2(r.left(), y + 1.0),
                Align2::LEFT_TOP,
                glyph,
                mono(12.0),
                color,
            );
            let h = g.size().y;
            p.galley(pos2(r.left() + 24.0, y), g, color);
            y += h + 8.0;
        }
    }

    fn uninstall(&mut self, ui: &mut Ui, r: Rect) {
        let y = heading(ui, r, t!("setup.uninstall_q")) + 14.0;
        let p = ui.painter();
        let items = [
            (icon::APP_WINDOW, t!("setup.rm_app")),
            (icon::CURSOR_CLICK, t!("setup.rm_menu")),
            (icon::FILM_STRIP, t!("setup.rm_assoc")),
            (icon::FOLDER_SIMPLE, t!("setup.rm_cache")),
        ];
        let h = 37.0;
        let list = Rect::from_min_size(pos2(r.left(), y), vec2(r.width(), h * items.len() as f32));
        p.rect(
            list,
            9.0,
            CARD,
            Stroke::new(1.0, LINE),
            egui::StrokeKind::Inside,
        );
        for (i, (glyph, label)) in items.iter().enumerate() {
            let top = list.top() + i as f32 * h;
            if i > 0 {
                p.hline(list.x_range(), top, Stroke::new(1.0, LINE));
            }
            let cy = top + h / 2.0;
            p.text(
                pos2(list.left() + 14.0, cy),
                Align2::LEFT_CENTER,
                *glyph,
                mono(17.0),
                MUTED,
            );
            p.text(
                pos2(list.left() + 46.0, cy),
                Align2::LEFT_CENTER,
                *label,
                mono(12.5),
                TEXT,
            );
        }
        card(
            ui,
            pos2(r.left(), list.bottom() + 12.0),
            r.width(),
            [(
                icon::SLIDERS_HORIZONTAL,
                t!("setup.keep_settings"),
                &mut self.keep_settings,
            )],
        );
        if self.denied {
            ui.painter().text(
                pos2(r.left(), list.bottom() + 12.0 + ROW_H + 16.0),
                Align2::LEFT_TOP,
                t!("setup.elevate_denied"),
                mono(11.0),
                RED,
            );
        }
    }

    /// The downloaded copy this window runs from, when it isn't the installed one.
    fn downloaded(&self) -> Option<PathBuf> {
        let me = std::env::current_exe().ok()?;
        let installed = self.installed.as_ref()?;
        (!setup::same_file(&me, installed)).then_some(me)
    }

    fn foot(&mut self, ui: &mut Ui, r: Rect) {
        let p = ui.painter();
        p.rect_filled(r, 0.0, PANEL);
        p.hline(r.x_range(), r.top(), Stroke::new(1.0, LINE));
        let inner = Rect::from_min_max(
            pos2(r.left() + RAIL_W + PAD - 10.0, r.top()),
            pos2(r.right() - 16.0, r.bottom()),
        );
        let ctx = ui.ctx().clone();
        ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
            ui.horizontal_centered(|ui| match self.page {
                Page::Welcome => {
                    right(ui, |ui| {
                        // All users: the shield, and Next restarts setup through the UAC prompt.
                        let elevate = self.needs_elevation();
                        let glyph = if elevate {
                            icon::SHIELD_CHECK
                        } else {
                            icon::ARROW_RIGHT
                        };
                        if primary(ui, glyph, t!("setup.next")).clicked() {
                            if elevate {
                                self.elevate_install(&ctx);
                            } else {
                                self.page = Page::Options;
                            }
                        }
                    });
                }
                Page::Options => {
                    // The elevated copy can't go back to Just for me; it has no Welcome page.
                    if self.mode == Mode::Install
                        && button(ui, icon::ARROW_LEFT, t!("setup.back"), false, GHOST).clicked()
                    {
                        self.page = Page::Welcome;
                    }
                    right(ui, |ui| {
                        if primary(ui, icon::DOWNLOAD_SIMPLE, t!("setup.install")).clicked() {
                            self.start(&ctx, Page::Installing);
                        }
                    });
                }
                Page::Installing | Page::Removing => {
                    if self.failed {
                        right(ui, |ui| {
                            if button(ui, "", t!("setup.close"), false, GHOST).clicked() {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        });
                    }
                }
                Page::Done => {
                    if button(ui, "", t!("setup.close"), false, GHOST).clicked() {
                        self.finish(&ctx, false);
                    }
                    right(ui, |ui| {
                        if primary(ui, icon::PLAY, t!("setup.open")).clicked() {
                            self.finish(&ctx, true);
                        }
                    });
                }
                Page::Uninstall => {
                    if button(ui, "", t!("setup.cancel"), false, GHOST).clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    right(ui, |ui| {
                        let elevate =
                            self.uninstall_scope == Some(Scope::Machine) && !setup::elevated();
                        let glyph = if elevate {
                            icon::SHIELD_CHECK
                        } else {
                            icon::TRASH
                        };
                        if danger(ui, glyph, t!("setup.uninstall_btn")).clicked() {
                            if elevate {
                                self.elevate_uninstall(&ctx);
                            } else {
                                self.start(&ctx, Page::Removing);
                            }
                        }
                    });
                }
            });
        });
    }

    /// Done: start the installed copy if asked, drop the download if asked, and close.
    fn finish(&mut self, ctx: &egui::Context, open: bool) {
        let opened = match &self.installed {
            Some(exe) if open => setup::open_unelevated(exe),
            _ => Ok(()),
        };
        if let Err(e) = opened {
            // Stay open, and keep the download, so there is still a copy to start.
            self.lines
                .push((Kind::Fail, t!("setup.open_failed", error = e)));
            return;
        }
        if self.delete_download
            && let Some(file) = self.downloaded()
        {
            setup::delete_later(&file);
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

impl eframe::App for SetupWindow {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // The elevated uninstall removes at once.
        if self.page == Page::Removing
            && self.job.is_none()
            && !self.failed
            && self.lines.is_empty()
        {
            self.start(ctx, Page::Removing);
        }
        self.poll(ctx);
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        // No closing halfway through an install or uninstall.
        if self.job.is_some() && ui.ctx().input(|i| i.viewport().close_requested()) {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        let full = ui.max_rect();
        ui.painter()
            .rect_filled(full, 0.0, Color32::from_rgb(1, 4, 2));
        let head_h = if chrome::CUSTOM { HEAD_H } else { 0.0 };
        let head = Rect::from_min_size(full.min, vec2(full.width(), head_h));
        let foot = Rect::from_min_max(pos2(full.left(), full.bottom() - FOOT_H), full.max);
        let rail = Rect::from_min_max(
            pos2(full.left(), head.bottom()),
            pos2(full.left() + RAIL_W, foot.top()),
        );
        let page = Rect::from_min_max(
            pos2(rail.right(), head.bottom()),
            pos2(full.right(), foot.top()),
        );
        if chrome::CUSTOM {
            self.head(ui, head);
        }
        self.rail(ui, rail);
        self.page(ui, page);
        self.foot(ui, foot);
        if chrome::CUSTOM {
            ui.painter().rect_stroke(
                full,
                0.0,
                Stroke::new(1.0, Color32::from_rgb(26, 61, 41)),
                egui::StrokeKind::Inside,
            );
        }
    }
}

/// The page title; returns the y below it.
fn heading(ui: &Ui, r: Rect, text: &str) -> f32 {
    let g = ui.painter().layout_no_wrap(text.into(), bold(21.0), TEXT);
    let h = g.size().y;
    ui.painter().galley(r.min, g, TEXT);
    r.top() + h + 6.0
}

/// A card of rows, each an icon, a label and a switch.
fn card<const N: usize>(ui: &mut Ui, at: Pos2, width: f32, rows: [(&str, &str, &mut bool); N]) {
    let r = Rect::from_min_size(at, vec2(width, ROW_H * N as f32));
    ui.painter().rect(
        r,
        9.0,
        CARD,
        Stroke::new(1.0, LINE),
        egui::StrokeKind::Inside,
    );
    for (i, (glyph, label, on)) in rows.into_iter().enumerate() {
        let row = Rect::from_min_size(
            pos2(r.left(), r.top() + i as f32 * ROW_H),
            vec2(width, ROW_H),
        );
        let resp = ui.interact(row, ui.id().with(("setup-row", label)), Sense::click());
        if resp.clicked() {
            *on = !*on;
        }
        let p = ui.painter();
        if i > 0 {
            p.hline(r.x_range(), row.top(), Stroke::new(1.0, LINE));
        }
        if resp.hovered() {
            p.rect_filled(row.shrink(1.0), 8.0, CARD_HI);
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        let cy = row.center().y;
        p.text(
            pos2(row.left() + 14.0, cy),
            Align2::LEFT_CENTER,
            glyph,
            mono(18.0),
            MUTED,
        );
        p.text(
            pos2(row.left() + 46.0, cy),
            Align2::LEFT_CENTER,
            label,
            mono(12.5),
            TEXT,
        );
        let sw = Rect::from_center_size(pos2(row.right() - 29.0, cy), vec2(30.0, 18.0));
        p.rect_filled(sw, 9.0, if *on { GREEN } else { LINE });
        let knob = if *on {
            sw.right() - 9.0
        } else {
            sw.left() + 9.0
        };
        p.circle_filled(pos2(knob, cy), 7.0, if *on { INK } else { MUTED });
        if resp.has_focus() {
            focus_ring(p, row.shrink(2.0), 8.0);
        }
        let on = *on;
        resp.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, on, label)
        });
    }
}

/// A card of rows, each an icon, a label and a radio dot; one is chosen.
fn radios<T: Copy + PartialEq, const N: usize>(
    ui: &mut Ui,
    at: Pos2,
    width: f32,
    value: &mut T,
    rows: [(&str, &str, T); N],
) {
    let r = Rect::from_min_size(at, vec2(width, ROW_H * N as f32));
    ui.painter().rect(
        r,
        9.0,
        CARD,
        Stroke::new(1.0, LINE),
        egui::StrokeKind::Inside,
    );
    for (i, (glyph, label, this)) in rows.into_iter().enumerate() {
        let row = Rect::from_min_size(
            pos2(r.left(), r.top() + i as f32 * ROW_H),
            vec2(width, ROW_H),
        );
        let resp = ui.interact(row, ui.id().with(("setup-radio", label)), Sense::click());
        if resp.clicked() {
            *value = this;
        }
        let on = *value == this;
        let p = ui.painter();
        if i > 0 {
            p.hline(r.x_range(), row.top(), Stroke::new(1.0, LINE));
        }
        if resp.hovered() {
            p.rect_filled(row.shrink(1.0), 8.0, CARD_HI);
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        let cy = row.center().y;
        p.text(
            pos2(row.left() + 14.0, cy),
            Align2::LEFT_CENTER,
            glyph,
            mono(18.0),
            if on { GREEN } else { MUTED },
        );
        p.text(
            pos2(row.left() + 46.0, cy),
            Align2::LEFT_CENTER,
            label,
            mono(12.5),
            TEXT,
        );
        let dot = pos2(row.right() - 24.0, cy);
        p.circle_stroke(dot, 8.0, Stroke::new(1.5, if on { GREEN } else { LINE }));
        if on {
            p.circle_filled(dot, 4.0, GREEN);
        }
        if resp.has_focus() {
            focus_ring(p, row.shrink(2.0), 8.0);
        }
        resp.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, on, label)
        });
    }
}

fn right(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.with_layout(Layout::right_to_left(Align::Center), add);
}

/// The green call to action, sized to its label.
fn primary(ui: &mut Ui, glyph: &str, label: &str) -> Response {
    let w = ui
        .painter()
        .layout_no_wrap(label.into(), bold(12.5), INK)
        .size()
        .x;
    primary_caps(ui, glyph, label, vec2(w + 52.0, 34.0), false, &[])
}

/// The red outlined Uninstall button.
fn danger(ui: &mut Ui, glyph: &str, label: &str) -> Response {
    let p = ui.painter();
    let g = p.layout_no_wrap(label.into(), bold(12.5), RED);
    let size = vec2(g.size().x + 52.0, 34.0);
    let (r, resp) = ui.allocate_exact_size(size, Sense::click());
    let fill = RED.gamma_multiply(if resp.hovered() { 0.14 } else { 0.06 });
    let p = ui.painter();
    p.rect(
        r,
        7.0,
        fill,
        Stroke::new(1.0, RED.gamma_multiply(0.45)),
        egui::StrokeKind::Inside,
    );
    p.text(
        pos2(r.left() + 16.0, r.center().y),
        Align2::LEFT_CENTER,
        glyph,
        mono(14.0),
        RED,
    );
    p.galley(
        pos2(r.left() + 38.0, r.center().y - g.size().y / 2.0),
        g,
        RED,
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if resp.has_focus() {
        focus_ring(p, r, 7.0);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    resp
}

/// This exe's size in MB, for the Welcome page.
fn exe_mb() -> u64 {
    std::env::current_exe()
        .and_then(std::fs::metadata)
        .map_or(0, |m| m.len().div_ceil(1024 * 1024))
}
