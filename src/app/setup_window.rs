//! The setup and uninstall window. Chyk.exe shows it instead of the editor when it starts
//! outside an install, or with `--uninstall`. A rail of steps on the left, the page on the right,
//! buttons along the bottom. The work runs in `crate::setup` on a background thread.

use super::setup_widgets::*;
use super::*;
use crate::setup::{self, Choices, Kind, Mode, Msg, Scope};

const SIZE: [f32; 2] = [820.0, 520.0];
const HEAD_H: f32 = 40.0;
const FOOT_H: f32 = 58.0;
const RAIL_W: f32 = 210.0;
const PAD: f32 = 34.0;
/// How long each line of the install list shows alone, so a fast install still reads step by step.
const LINE_PAUSE: std::time::Duration = std::time::Duration::from_millis(220);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Page {
    Welcome,
    Options,
    Installing,
    Done,
    Uninstall,
    Removing,
    Removed,
}

impl Page {
    /// Its place in the rail's steps.
    fn step(self) -> usize {
        match self {
            Self::Welcome | Self::Uninstall | Self::Removing => 0,
            // Past the only uninstall step, so it shows as done.
            Self::Removed => 1,
            Self::Options => 1,
            Self::Installing => 2,
            Self::Done => 3,
        }
    }
}

/// What to do with an install that's already there.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Action {
    /// Install over it with its own choices: fixes or updates the files, keeps settings.
    Repair,
    /// Clear it (and the settings, unless kept), then install again.
    Reinstall,
    Remove,
}

pub struct SetupWindow {
    mode: Mode,
    title: String,
    page: Page,
    choices: Choices,
    keep_settings: bool,
    delete_download: bool,
    lines: Vec<(Kind, String)>,
    /// The slow step under way, under the lines.
    busy: Option<String>,
    progress: f32,
    job: Option<Receiver<Msg>>,
    /// Messages from the job not shown yet: one line per [`LINE_PAUSE`].
    queued: std::collections::VecDeque<Msg>,
    /// When the next queued line may show.
    next_line: std::time::Instant,
    installed: Option<PathBuf>,
    failed: bool,
    /// The folder was picked with Change…, so switching the scope keeps it.
    custom_dir: bool,
    /// The UAC prompt was declined.
    denied: bool,
    /// The registered install is for all users: uninstalling needs admin rights.
    uninstall_scope: Option<Scope>,
    /// Chyk is installed already: Welcome offers Repair, Reinstall or Uninstall instead.
    existing: Option<setup::Existing>,
    action: Action,
}

/// Opens the window and runs it until it closes.
/// Set when an install or uninstall fails, so the process exits with 1: the uninstall window
/// waits on the elevated copy's exit code.
static FAILED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn run(mode: Mode) -> eframe::Result {
    let title = match mode {
        Mode::Install | Mode::InstallAllUsers { .. } => {
            t!("setup.title", version = setup::VERSION)
        }
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
    )?;
    if FAILED.load(std::sync::atomic::Ordering::Relaxed) {
        std::process::exit(1);
    }
    Ok(())
}

impl SetupWindow {
    fn new(mode: Mode, title: String) -> Self {
        let mut choices = Choices::default();
        let mut keep_settings = true;
        let existing = match mode {
            Mode::Install | Mode::InstallAllUsers { .. } => setup::existing(),
            _ => None,
        };
        let page = match &mode {
            Mode::Install => Page::Welcome,
            Mode::InstallAllUsers { dir, fresh } => {
                // Repair restarted elevated: offer the install's own choices again.
                match &existing {
                    Some(e) if !fresh && setup::same_file(&e.choices.dir, dir) => {
                        choices = e.choices.clone();
                    }
                    _ => {}
                }
                choices.scope = Scope::Machine;
                choices.dir = dir.clone();
                choices.fresh = *fresh;
                Page::Options
            }
            Mode::Uninstall => Page::Uninstall,
            Mode::UninstallNow {
                keep_settings: keep,
                ..
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
            busy: None,
            progress: 0.0,
            job: None,
            queued: Default::default(),
            next_line: std::time::Instant::now(),
            installed: None,
            failed: false,
            existing,
            action: Action::Repair,
        }
    }

    fn start(&mut self, ctx: &egui::Context, page: Page) {
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        let choices = self.choices.clone();
        let keep = self.keep_settings;
        let user_data = !self.machine_only();
        std::thread::spawn(move || {
            let (wtx, wrx) = channel();
            let worker = std::thread::spawn(move || match page {
                Page::Installing => setup::install(choices, wtx),
                _ => setup::uninstall(keep, user_data, wtx),
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
            match rx.try_recv() {
                Ok(msg) => self.queued.push_back(msg),
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                // The worker is gone without a Done.
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    if !self.queued.iter().any(|m| matches!(m, Msg::Done(_))) {
                        self.queued
                            .push_back(Msg::Line(Kind::Fail, t!("setup.crashed").to_owned()));
                        self.queued.push_back(Msg::Done(None));
                    }
                    break;
                }
            }
        }
        while let Some(msg) = self.queued.front() {
            let now = std::time::Instant::now();
            if matches!(msg, Msg::Line(..) | Msg::Busy(_) | Msg::Done(_)) && now < self.next_line {
                ctx.request_repaint_after(self.next_line - now);
                return;
            }
            let Some(msg) = self.queued.pop_front() else {
                return;
            };
            match msg {
                Msg::Line(kind, text) => {
                    self.lines.push((kind, text));
                    self.busy = None;
                    self.next_line = now + LINE_PAUSE;
                }
                Msg::Busy(text) => self.busy = Some(text),
                Msg::Progress(p) => self.progress = p,
                Msg::Denied => {
                    self.denied = true;
                    self.page = Page::Uninstall;
                    self.job = None;
                    self.queued.clear();
                    return;
                }
                Msg::Done(exe) => return self.finished(ctx, exe),
            }
        }
    }

    /// The job ended: `exe` is the installed (or removed) Chyk, `None` on failure.
    fn finished(&mut self, ctx: &egui::Context, exe: Option<PathBuf>) {
        self.failed = exe.is_none();
        if self.failed {
            FAILED.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.installed = exe;
        self.job = None;
        self.busy = None;
        self.queued.clear();
        if self.failed {
            // The window stays on the working page so the error can be read.
            return;
        }
        match self.page {
            Page::Installing => self.page = Page::Done,
            // The elevated worker: the window that started it reports the result.
            Page::Removing if self.machine_only() => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close)
            }
            // The cleanup of the files runs once this window is gone.
            Page::Removing => self.page = Page::Removed,
            _ => {}
        }
    }

    /// This is the elevated copy that removes only the machine-wide part.
    fn machine_only(&self) -> bool {
        matches!(
            self.mode,
            Mode::UninstallNow {
                machine_only: true,
                ..
            }
        )
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
        // The same right margin as the other windows' headers.
        let buttons = Rect::from_min_max(
            pos2(r.right() - 100.0, r.top()),
            pos2(r.right() - 8.0, r.bottom()),
        );
        ui.scope_builder(egui::UiBuilder::new().max_rect(buttons), |ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // No closing mid-job; Removed has its own Close button instead.
                let no_close = self.job.is_some() || self.page == Page::Removed;
                window_buttons(
                    ui,
                    [chrome::Button::Close, chrome::Button::Minimize]
                        .iter()
                        .filter(|b| !no_close || **b != chrome::Button::Close),
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
        let removing = matches!(self.page, Page::Uninstall | Page::Removing | Page::Removed);
        let steps: Vec<&str> = match self.mode {
            Mode::Install | Mode::InstallAllUsers { .. } if !removing => vec![
                t!("setup.step_welcome"),
                t!("setup.step_options"),
                t!("setup.step_install"),
                t!("setup.step_done"),
            ],
            _ => vec![t!("setup.uninstall")],
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
            Page::Done | Page::Removed => self.done(ui, inner),
            Page::Uninstall => self.uninstall(ui, inner),
        }
    }

    fn welcome(&mut self, ui: &mut Ui, r: Rect) {
        if self.existing.is_some() {
            return self.maintain(ui, r);
        }
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
                (icon::USER, t!("setup.scope_user"), "", Scope::User),
                (icon::USERS_THREE, t!("setup.scope_all"), "", Scope::Machine),
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

    /// Welcome when Chyk is installed already: Repair (Update, from an older version), Reinstall
    /// or Uninstall, and whether settings stay.
    fn maintain(&mut self, ui: &mut Ui, r: Rect) {
        let Some(e) = &self.existing else { return };
        let y = heading(ui, r, t!("setup.installed_title"));
        let mut where_ = egui::text::LayoutJob::single_section(
            format!("v{} · {}", e.version, e.choices.dir.display()),
            egui::TextFormat::simple(mono(11.5), MUTED),
        );
        where_.wrap = egui::text::TextWrapping::truncate_at_width(r.width());
        let g = ui.painter().layout_job(where_);
        ui.painter().galley(pos2(r.left(), y + 2.0), g, MUTED);
        let update = setup::newer(setup::VERSION, &e.version);
        let (repair, repair_sub) = if update {
            (
                t!("setup.update").to_owned(),
                t!(
                    "setup.update_sub",
                    old = e.version,
                    version = setup::VERSION
                ),
            )
        } else {
            (
                t!("setup.repair").to_owned(),
                t!("setup.repair_sub").to_owned(),
            )
        };
        let bottom = radios(
            ui,
            pos2(r.left(), y + 34.0),
            r.width(),
            &mut self.action,
            [
                (
                    if update {
                        icon::ARROW_CIRCLE_UP
                    } else {
                        icon::WRENCH
                    },
                    &repair,
                    &repair_sub,
                    Action::Repair,
                ),
                (
                    icon::ARROWS_CLOCKWISE,
                    t!("setup.reinstall"),
                    t!("setup.reinstall_sub"),
                    Action::Reinstall,
                ),
                (
                    icon::TRASH,
                    t!("setup.remove"),
                    t!("setup.uninstall_sub"),
                    Action::Remove,
                ),
            ],
        );
        if self.action != Action::Repair {
            card(
                ui,
                pos2(r.left(), bottom + 12.0),
                r.width(),
                [(
                    icon::SLIDERS_HORIZONTAL,
                    t!("setup.keep_settings"),
                    &mut self.keep_settings,
                )],
            );
        }
        if self.denied {
            ui.painter().text(
                pos2(r.left(), r.bottom()),
                Align2::LEFT_BOTTOM,
                t!("setup.elevate_denied"),
                mono(11.0),
                RED,
            );
        }
    }

    /// Next on the Repair / Reinstall / Uninstall page.
    fn maintain_next(&mut self, ctx: &egui::Context) {
        let Some(e) = &self.existing else { return };
        self.denied = false;
        match self.action {
            Action::Remove => {
                self.uninstall_scope = Some(e.choices.scope);
                self.page = Page::Uninstall;
                return;
            }
            Action::Repair => self.choices = e.choices.clone(),
            Action::Reinstall => {
                // Here, not in an elevated copy, which may run as another account.
                setup::clear_user_data(self.keep_settings);
                self.choices = Choices {
                    scope: e.choices.scope,
                    dir: e.choices.dir.clone(),
                    fresh: true,
                    ..Choices::default()
                };
            }
        }
        self.custom_dir = true;
        if self.needs_elevation() {
            self.elevate_install(ctx);
        } else {
            self.page = Page::Options;
        }
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
        let mut args = vec![
            "--setup".into(),
            "--all-users".into(),
            self.choices.dir.clone().into_os_string(),
        ];
        if self.choices.fresh {
            args.push("--fresh".into());
        }
        if let Some(url) = std::env::var_os(setup::LIBS_URL_ENV) {
            args.extend(["--libs-url".into(), url]);
        }
        match setup::restart_elevated(&args) {
            Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Err(_) => self.denied = true,
        }
    }

    /// Runs the machine-wide part of the uninstall in an elevated copy and waits for it, then
    /// removes this account's data here: UAC may run the copy as another administrator.
    fn elevate_uninstall(&mut self, ctx: &egui::Context) {
        let keep = self.keep_settings;
        let mut args = vec![
            "--uninstall".into(),
            "--confirm".into(),
            "--machine-only".into(),
        ];
        if keep {
            args.push("--keep-settings".into());
        }
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let me = std::env::current_exe().unwrap_or_default();
            let msgs = match setup::run_and_wait(&me, &args, true) {
                Err(_) => vec![Msg::Denied],
                Ok(0) => match setup::clean_user(keep) {
                    // The elevated copy did these; its own window closed without a list.
                    Ok(()) => [
                        t!("setup.rm_menu"),
                        t!("setup.rm_assoc"),
                        t!("setup.rm_app"),
                        t!("setup.rm_cache"),
                    ]
                    .into_iter()
                    .map(|l| Msg::Line(Kind::Ok, l.to_owned()))
                    .chain([Msg::Done(Some(me))])
                    .collect(),
                    Err(e) => vec![
                        Msg::Line(Kind::Fail, t!("setup.rm_failed", error = e)),
                        Msg::Done(None),
                    ],
                },
                Ok(_) => vec![
                    Msg::Line(Kind::Fail, t!("setup.uninstall_failed").to_owned()),
                    Msg::Done(None),
                ],
            };
            for msg in msgs {
                let _ = tx.send(msg);
            }
            ctx.request_repaint();
        });
        self.denied = false;
        self.page = Page::Removing;
        self.job = Some(rx);
    }

    fn options(&mut self, ui: &mut Ui, r: Rect) {
        let y = heading(ui, r, t!("setup.options"));
        let c = &mut self.choices;
        let rows: [(&str, &str, &mut bool); 3] = [
            (icon::CURSOR_CLICK, t!("settings.shell_menu"), &mut c.menu),
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
        steps(ui, log, &self.lines, self.busy.as_deref());
    }

    /// Done and Removed: the title, what was done, and (Done, from a downloaded setup) whether
    /// to delete that file.
    fn done(&mut self, ui: &mut Ui, r: Rect) {
        let title = if self.page == Page::Removed {
            t!("setup.removed")
        } else {
            t!("setup.done")
        };
        let p = ui.painter();
        let g = p.layout_no_wrap(title.into(), bold(21.0), TEXT);
        let h = g.size().y;
        let c = pos2(r.left() + 13.0, r.top() + h / 2.0);
        p.add(
            glow(GREEN.gamma_multiply(0.35), 12)
                .as_shape(Rect::from_center_size(c, vec2(26.0, 26.0)), 13),
        );
        p.circle(c, 13.0, PANEL, Stroke::new(1.0, GREEN.gamma_multiply(0.5)));
        p.text(c, Align2::CENTER_CENTER, icon::CHECK, mono(14.0), GREEN);
        p.galley(pos2(r.left() + 38.0, r.top()), g, TEXT);
        let list = Rect::from_min_max(pos2(r.left(), r.top() + h + 20.0), r.max);
        let bottom = steps(ui, list, &self.lines, None);
        if self.page == Page::Done && self.downloaded().is_some() {
            card(
                ui,
                pos2(r.left(), bottom + 12.0),
                r.width(),
                [(
                    icon::TRASH,
                    t!("setup.delete_download"),
                    &mut self.delete_download,
                )],
            );
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
                Page::Welcome if self.existing.is_some() => {
                    if button(ui, "", t!("setup.cancel"), false, GHOST).clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    right(ui, |ui| {
                        // Each choice's own step needs admin rights for an all-users install.
                        let elevate = self.action != Action::Remove
                            && self
                                .existing
                                .as_ref()
                                .is_some_and(|e| e.choices.scope == Scope::Machine)
                            && !setup::elevated();
                        let glyph = if elevate {
                            icon::SHIELD_CHECK
                        } else {
                            icon::ARROW_RIGHT
                        };
                        if primary(ui, glyph, t!("setup.next")).clicked() {
                            self.maintain_next(&ctx);
                        }
                    });
                }
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
                // Closing without opening is the window's own close button.
                Page::Done => {
                    right(ui, |ui| {
                        if primary(ui, icon::PLAY, t!("setup.open")).clicked() {
                            self.finish(&ctx, true);
                        }
                    });
                }
                Page::Removed => {
                    right(ui, |ui| {
                        if primary(ui, icon::CHECK, t!("setup.close")).clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                }
                Page::Uninstall => {
                    // From the Repair / Reinstall / Uninstall page: back to it.
                    if self.existing.is_some() {
                        if button(ui, icon::ARROW_LEFT, t!("setup.back"), false, GHOST).clicked() {
                            self.denied = false;
                            self.page = Page::Welcome;
                        }
                    } else if button(ui, "", t!("setup.cancel"), false, GHOST).clicked() {
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
        self.drop_download();
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    /// Deletes the downloaded setup once this window is gone, if asked. Once.
    fn drop_download(&mut self) {
        if std::mem::take(&mut self.delete_download)
            && let Some(file) = self.downloaded()
        {
            setup::delete_later(&file);
        }
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
        if ui.ctx().input(|i| i.viewport().close_requested()) {
            if self.job.is_some() {
                // No closing halfway through an install or uninstall.
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::CancelClose);
            } else if self.page == Page::Done {
                // Closed from Done without Open Chyk: still drop the setup file if asked.
                self.drop_download();
            }
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

/// The installed size in MB, for the Welcome page: this exe plus the libraries it adds.
fn exe_mb() -> u64 {
    #[cfg(bundled)]
    let libs = crate::bundle::size();
    #[cfg(not(bundled))]
    let libs = 0;
    let exe = std::env::current_exe()
        .and_then(std::fs::metadata)
        .map_or(0, |m| m.len());
    (exe + libs).div_ceil(1024 * 1024)
}
