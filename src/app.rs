use crate::i18n::{t, tn};
use crate::{chrome, crop, ffmpeg, i18n, meta, player::Player, strip, zoom};
use eframe::egui::{
    self, Align, Align2, Color32, CursorIcon, FontId, Key, Layout, Modifiers, Pos2, Rect, Response,
    RichText, Sense, Stroke, TextureHandle, Ui, Vec2, pos2, vec2,
};
use egui_phosphor::regular as icon;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// Phosphor green: everything active.
const GREEN: Color32 = Color32::from_rgb(77, 255, 155);
/// Dim green for secondary icons.
const GREEN_DIM: Color32 = Color32::from_rgb(31, 143, 85);
const RED: Color32 = Color32::from_rgb(255, 92, 120);
const BG: Color32 = Color32::BLACK;
/// Header, deck and popover fill.
const PANEL: Color32 = Color32::from_rgb(3, 8, 5);
/// Raised controls.
const CARD: Color32 = Color32::from_rgb(7, 19, 12);
const CARD_HI: Color32 = Color32::from_rgb(12, 30, 19);
/// Hairline borders.
const LINE: Color32 = Color32::from_rgb(18, 48, 31);
const TEXT: Color32 = Color32::from_rgb(207, 238, 221);
const MUTED: Color32 = Color32::from_rgb(95, 138, 113);
/// Text and glyphs on green.
const INK: Color32 = Color32::from_rgb(0, 20, 10);
/// Floating bars over the video (86% opaque panel).
const GLASS: Color32 = Color32::from_rgba_premultiplied(3, 9, 5, 219);
/// Outside the crop and the trim selection.
const DIM: Color32 = Color32::from_black_alpha(163);
/// How long a confirmation (like "Saved") stays up, the last part of it fading out.
const STATUS_LIFE: Duration = Duration::from_secs(4);
const STATUS_FADE: Duration = Duration::from_millis(400);
/// Shortcut legend on the start screen: rows of this many shortcuts.
const LEGEND_PER_ROW: usize = 4;
/// Height of the shortcut legend (two rows of 22 pt key caps, 8 pt apart).
const LEGEND_H: f32 = 52.0;
/// Header row height; with 6 pt margins the header is 52 pt, and on macOS the native traffic
/// lights are moved to its vertical center (`chrome::place_traffic_lights`).
const HEADER_ROW: f32 = 40.0;
/// Filmstrip thumbnails are tinted toward the phosphor green.
const THUMB_TINT: Color32 = Color32::from_rgb(170, 220, 190);
const THUMBS: usize = 12;
/// Filmstrip tile size in pixels. ffmpeg center-crops frames to it.
const THUMB: [usize; 2] = [96, 72];
/// Shortest selectable trim, in seconds.
const MIN_LEN: f64 = 0.1;
/// Trim handle width, in points.
const HANDLE: f32 = 14.0;
/// Filmstrip height, in points.
const TRIM_H: f32 = 52.0;
/// Height of the bands along the selection's top and bottom edges that slide it, in points.
const GRIP: f32 = 9.0;
/// Thickness of the trim box's top and bottom bars, in points.
const FRAME: f32 = 4.0;
/// Holding a trim handle still this long zooms the timeline in around it, like QuickTime.
const HOLD: Duration = Duration::from_millis(450);
/// How far that hold zooms in.
const HOLD_ZOOM: f64 = 10.0;
/// Playback speed presets and limits.
const SPEEDS: &[f64] = &[0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0, 4.0];
const SPEED_MIN: f64 = 0.25;
const SPEED_MAX: f64 = 4.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Drag {
    /// Crop handle, see [`crop::drag`].
    Crop(i8, i8),
    Move,
    In,
    Out,
    Scrub,
    /// Trim selection, keeping its length.
    Slide,
    /// Pan the zoomed video stage.
    Pan,
}

struct Video {
    path: PathBuf,
    dur: f64,
    /// Frame size after rotation, in pixels. This is what ffmpeg's crop filter sees.
    size: [u32; 2],
    /// Display aspect ratio after rotation.
    aspect: f32,
    codec: String,
}

/// Detection state for stripping the stretches where nothing on screen changes.
#[derive(Default)]
struct Strip {
    enabled: bool,
    /// Seconds kept around each active interval.
    pad: f64,
    /// Ignore zones, normalized to the displayed frame.
    zones: Vec<Rect>,
    /// Whether zones are being drawn/edited on the video.
    edit: bool,
    selected: Option<usize>,
    scan: Option<strip::Scan>,
    job: Option<strip::Running>,
    progress: f32,
    /// Crop the current scan (or the running one) looked inside; `None` = the whole frame.
    scan_crop: Option<Rect>,
    /// While scanning: active intervals found so far, and how far the scan has read (seconds).
    partial: Vec<(f64, f64)>,
    scanned: f64,
    /// The scan line as drawn, moving smoothly between the scanner's 100 ms reports.
    front: Glide,
    /// Zone and handle being dragged.
    drag: Option<(usize, Drag)>,
    /// Zone rect when the drag began, to notice a real change.
    zone_start: Option<Rect>,
    /// Offset from the pointer to the grabbed corner.
    grab: Vec2,
}

/// Zoom/pan state for the trim bar.
#[derive(Default)]
struct Timeline {
    zoom: f32,
    /// Center of the visible range, in seconds.
    center: f64,
    /// Range the current filmstrip thumbnails cover.
    thumb_range: (f64, f64),
    /// Debounce deadline for regenerating thumbnails.
    thumb_after: Option<Instant>,
    /// While a trim handle is dragged: when the pointer last moved, and where.
    hold: Option<(Instant, f32)>,
    /// View to return to when a hold-zoomed handle drag ends.
    restore: Option<(f32, f64)>,
}

/// A question waiting in the center modal.
#[derive(Clone, PartialEq)]
enum Confirm {
    /// A file dropped or picked while another video is open: replace it or open a new window?
    Open(PathBuf),
    /// Closing the window (or quitting) with edits that were never exported.
    Quit,
    /// Closing the video with edits that were never exported.
    CloseVideo,
}

/// The edits an export carries: trim, crop, strip (with its areas and padding) and speed.
/// Export settings (quality, format, mute) aren't edits.
type Edits = ((f64, f64), Option<Rect>, Option<(Vec<Rect>, f64)>, f64);

/// A progress value drawn smoothly between periodic reports (the scan line, progress bars). In
/// between reports it moves at the measured speed, but never more than one report interval
/// (at least [`Glide::LEAD`]) past the last report, so a slowdown makes it wait rather than run
/// ahead, and it never moves back.
#[derive(Default)]
struct Glide {
    shown: f64,
    /// Units per second, smoothed.
    rate: f64,
    /// Seconds between reports, smoothed.
    interval: f64,
    /// Last report: when, and the value.
    report: Option<(Instant, f64)>,
    frame: Option<Instant>,
}

impl Glide {
    const LEAD: f64 = 0.2;

    fn reported(&mut self, value: f64) {
        let now = Instant::now();
        if let Some((t, s)) = self.report {
            let dt = (now - t).as_secs_f64();
            if dt > 0.02 {
                let r = ((value - s) / dt).max(0.0);
                let ema = |old: f64, new: f64| {
                    if old == 0.0 {
                        new
                    } else {
                        old * 0.7 + new * 0.3
                    }
                };
                self.rate = ema(self.rate, r);
                self.interval = ema(self.interval, dt);
            }
        }
        self.report = Some((now, value));
    }

    /// Advance for this frame and return the position to draw.
    fn tick(&mut self) -> f64 {
        let now = Instant::now();
        let dt = self.frame.map_or(0.0, |t| (now - t).as_secs_f64());
        self.frame = Some(now);
        if let Some((t, s)) = self.report {
            let lead = Self::LEAD.max(self.interval);
            let ahead = (self.rate * (now - t).as_secs_f64()).min(self.rate * lead);
            let target = s + ahead;
            if target > self.shown {
                // Eased, so speed changes and late reports blend in instead of jumping.
                self.shown += (target - self.shown) * (1.0 - (-12.0 * dt).exp());
            }
        }
        self.shown
    }
}

/// Which mode panel is open. Closing a panel keeps its effect: the crop stays applied, Strip stays on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Panel {
    Crop,
    Strip,
}

/// Which popover is open.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pop {
    Export,
    Info,
}

/// Where the popovers hang from, recorded while the header is laid out.
struct Anchors {
    export: Rect,
    info: Rect,
    header_bottom: f32,
    /// Width of the IN/OUT/LEN box last frame.
    selection_width: f32,
}

impl Default for Anchors {
    fn default() -> Self {
        Self {
            export: Rect::NOTHING,
            info: Rect::NOTHING,
            header_bottom: 52.0,
            selection_width: 360.0,
        }
    }
}

impl Timeline {
    fn view(&self, dur: f64) -> (f64, f64) {
        zoom::timeline_view(dur, self.zoom, self.center)
    }
}

pub struct App {
    player: Player,
    loading: Option<PathBuf>,
    video: Option<Video>,
    trim: (f64, f64),
    /// Normalized crop rect while crop mode is on.
    crop: Option<Rect>,
    preset: usize,
    precise: bool,
    /// Indexes into [`ffmpeg::FORMATS`] and [`ffmpeg::QUALITIES`].
    format: usize,
    quality: usize,
    mute: bool,
    /// Playback speed. The export is retimed by the same factor, so what plays is what saves.
    speed: f64,
    strip: Strip,
    timeline: Timeline,
    /// Video stage zoom and the normalized center of the zoomed view.
    video_zoom: f32,
    video_center: Pos2,
    drag: Option<Drag>,
    /// Offset from the pointer to the grabbed handle, so it doesn't jump to the pointer.
    grab: Vec2,
    thumbs: Vec<Option<TextureHandle>>,
    thumbs_rx: Option<Receiver<(usize, egui::ColorImage)>>,
    /// The applied crop the filmstrip shows (it doesn't follow the box while Crop is open).
    thumb_crop: Option<Rect>,
    job: Option<ffmpeg::Running>,
    progress: f32,
    /// Message and whether it is an error.
    status: Option<(String, bool)>,
    /// The message on screen and when it appeared, for fading confirmations out.
    status_since: Option<(String, Instant)>,
    /// Window buttons Glint draws itself (none on macOS, which keeps its native ones).
    buttons: chrome::Layout,
    pop: Option<Pop>,
    /// The export bar, drawn smoothly between ffmpeg's progress reports.
    export_glide: Glide,
    panel: Option<Panel>,
    /// Strip bar fit: the bar's shape (scanning, scanned, with areas) and its full-label width.
    strip_fit: (u8, f32),
    /// Width the transport row needs with everything shown; narrower rows go compact.
    deck_need: f32,
    confirm: Option<Confirm>,
    /// Edits as of loading the video or the last successful export; anything else is unsaved.
    saved: Option<Edits>,
    /// Edits the running export was started with.
    exporting: Option<Edits>,
    /// The user chose to quit without exporting: let the window close.
    quitting: bool,
    anchors: Anchors,
    /// File info for the info popover, read from mpv when it first opens.
    meta: Option<Vec<meta::Section>>,
    /// Finished strip scans for this session, per file, with the ignored areas and crop they used.
    /// Reopening Strip or the file reuses them; Rescan, or a different crop, scans again.
    scans: HashMap<PathBuf, (Vec<Rect>, Option<Rect>, strip::Scan)>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext, file: Option<PathBuf>) -> Result<Self, String> {
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        // Space Mono for English, Fira Code for Ukrainian (Space Mono has no Cyrillic).
        // Fira Code also covers Cyrillic file names in the English UI. Both OFL, see assets/fonts.
        for (name, data) in [
            (
                "SpaceMono",
                &include_bytes!("../assets/fonts/SpaceMono-Regular.ttf")[..],
            ),
            (
                "SpaceMonoBold",
                &include_bytes!("../assets/fonts/SpaceMono-Bold.ttf")[..],
            ),
            (
                "FiraCode",
                &include_bytes!("../assets/fonts/FiraCode-Regular.ttf")[..],
            ),
            (
                "FiraCodeBold",
                &include_bytes!("../assets/fonts/FiraCode-Bold.ttf")[..],
            ),
        ] {
            fonts
                .font_data
                .insert(name.into(), egui::FontData::from_static(data).into());
        }
        let (regular, heavy) = match *i18n::LANG {
            i18n::Lang::Uk => (["FiraCode"], ["FiraCodeBold"]),
            i18n::Lang::En => (["SpaceMono"], ["SpaceMonoBold"]),
        };
        // Text fonts first, then Phosphor icons: egui's built-in emoji font reuses some of
        // Phosphor's code points, and Phosphor itself has a space glyph, so it can't lead.
        let mut order: Vec<&str> = regular.to_vec();
        for name in ["FiraCode", "phosphor"] {
            if !order.contains(&name) {
                order.push(name);
            }
        }
        for family in [egui::FontFamily::Monospace, egui::FontFamily::Proportional] {
            let list = fonts.families.entry(family).or_default();
            list.retain(|n| !order.contains(&n.as_str()));
            for (i, name) in order.iter().enumerate() {
                list.insert(i, (*name).into());
            }
        }
        let mut bold: Vec<String> = Vec::new();
        for name in heavy.iter().copied().chain(["FiraCodeBold", "phosphor"]) {
            if !bold.iter().any(|n| n == name) {
                bold.push(name.into());
            }
        }
        fonts
            .families
            .insert(egui::FontFamily::Name("bold".into()), bold);
        cc.egui_ctx.set_fonts(fonts);
        theme(&cc.egui_ctx);
        #[cfg(target_os = "macos")]
        chrome::use_srgb(cc);
        let mut app = Self {
            player: Player::new(cc)?,
            loading: None,
            video: None,
            trim: (0.0, 0.0),
            crop: None,
            preset: 0,
            precise: false,
            format: 0,
            quality: 0,
            mute: false,
            speed: 1.0,
            strip: Strip {
                pad: 0.5,
                ..Default::default()
            },
            timeline: Timeline {
                zoom: 1.0,
                ..Default::default()
            },
            video_zoom: 1.0,
            video_center: pos2(0.5, 0.5),
            drag: None,
            grab: Vec2::ZERO,
            thumbs: Vec::new(),
            thumbs_rx: None,
            thumb_crop: None,
            job: None,
            progress: 0.0,
            status: None,
            status_since: None,
            buttons: chrome::Layout::detect(),
            pop: None,
            export_glide: Glide::default(),
            panel: None,
            strip_fit: (u8::MAX, 0.0),
            deck_need: 0.0,
            confirm: None,
            saved: None,
            exporting: None,
            quitting: false,
            anchors: Anchors::default(),
            meta: None,
            scans: HashMap::new(),
        };
        if let Some(f) = file {
            app.open(f);
        }
        Ok(app)
    }

    fn open(&mut self, path: PathBuf) {
        let (enabled, pad) = (self.strip.enabled, self.strip.pad);
        self.cancel_scan();
        self.strip = Strip {
            enabled,
            pad,
            ..Default::default()
        };
        self.timeline = Timeline {
            zoom: 1.0,
            ..Default::default()
        };
        self.video_zoom = 1.0;
        self.video_center = pos2(0.5, 0.5);
        self.speed = 1.0;
        self.drag = None;
        self.grab = Vec2::ZERO;
        self.thumbs = vec![None; THUMBS];
        self.thumbs_rx = None;
        self.meta = None;
        self.pop = None;
        let _ = self.player.mpv.set_property("speed", 1.0);
        self.player.cmd("loadfile", &[&path.to_string_lossy()]);
        self.loading = Some(path);
        self.status = None;
    }

    /// Back to the start screen. A running export keeps going and still reports when done.
    fn close(&mut self, ctx: &egui::Context) {
        let (enabled, pad) = (self.strip.enabled, self.strip.pad);
        self.cancel_scan();
        self.strip = Strip {
            enabled,
            pad,
            ..Default::default()
        };
        self.player.cmd("stop", &[]);
        self.video = None;
        self.loading = None;
        self.crop = None;
        self.panel = None;
        self.drag = None;
        self.thumbs.clear();
        self.thumbs_rx = None;
        self.timeline.thumb_after = None;
        self.meta = None;
        self.pop = None;
        ctx.send_viewport_cmd(egui::ViewportCommand::Title("Glint".into()));
    }

    fn open_dialog(&mut self) {
        if let Some(p) = rfd::FileDialog::new().pick_file() {
            self.request_open(p);
        }
    }

    /// Open `path`, or, if a video is already open, ask first so its edits aren't lost.
    fn request_open(&mut self, path: PathBuf) {
        if self.video.is_some() || self.loading.is_some() {
            self.pop = None;
            self.confirm = Some(Confirm::Open(path));
        } else {
            self.open(path);
        }
    }

    /// Start another Glint for `path`. Inside an AppImage the running binary lives in a temporary
    /// mount, so relaunch the AppImage file itself.
    fn open_in_new_window(&mut self, path: PathBuf) {
        let exe = std::env::var_os("APPIMAGE")
            .map(PathBuf::from)
            .or_else(|| std::env::current_exe().ok());
        let started = exe.map(|exe| std::process::Command::new(exe).arg(&path).spawn());
        if !matches!(started, Some(Ok(_))) {
            self.status = Some((t!("open.new_window_failed").into(), true));
        }
    }

    /// Frame size and display aspect after rotation.
    fn geometry(&self) -> ([u32; 2], f32) {
        let p = &self.player.props;
        let (w, h) = (p.width.max(1) as u32, p.height.max(1) as u32);
        let aspect = p.aspect.unwrap_or(w as f64 / h as f64) as f32;
        if p.rotate % 180 == 90 {
            ([h, w], 1.0 / aspect)
        } else {
            ([w, h], aspect)
        }
    }

    fn loaded(&mut self, ctx: &egui::Context) {
        let Some(path) = self.loading.take() else {
            return;
        };
        let dur = self.player.get::<f64>("duration").unwrap_or(0.0);
        let (size, aspect) = self.geometry();
        let codec = self
            .player
            .get::<String>("current-tracks/video/codec")
            .unwrap_or_default();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!("{name} — Glint")));
        let ext = path.extension().unwrap_or_default();
        self.format = ffmpeg::FORMATS
            .iter()
            .position(|f| ext.eq_ignore_ascii_case(f))
            .unwrap_or(0);

        self.meta = None;
        self.video = Some(Video {
            path,
            dur,
            size,
            aspect,
            codec,
        });
        self.trim = (0.0, dur);
        self.deck_need = 0.0;
        self.crop = None;
        self.panel = self.panel.filter(|p| *p == Panel::Strip);
        self.timeline = Timeline {
            zoom: 1.0,
            center: dur / 2.0,
            ..Default::default()
        };
        self.video_zoom = 1.0;
        self.video_center = pos2(0.5, 0.5);
        self.thumb_crop = None;
        self.load_thumbs(ctx);
        if self.strip.enabled {
            self.ensure_scan(ctx);
        }
        self.saved = Some(self.edits());
    }

    fn edits(&self) -> Edits {
        let strip = self
            .strip
            .enabled
            .then(|| (self.strip.zones.clone(), self.strip.pad));
        (self.trim, self.crop, strip, self.speed)
    }

    /// Edits made since loading or the last export, which closing would lose.
    fn unsaved(&self) -> bool {
        self.video.is_some() && self.saved.as_ref() != Some(&self.edits())
    }

    /// Close the video, asking first if that would lose unexported edits.
    fn request_close(&mut self, ctx: &egui::Context) {
        if self.unsaved() {
            self.pop = None;
            self.confirm = Some(Confirm::CloseVideo);
        } else {
            self.close(ctx);
        }
    }

    /// Ask for the current visible timeline range's filmstrip. One ffmpeg run fills all tiles.
    fn load_thumbs(&mut self, ctx: &egui::Context) {
        let Some(v) = &self.video else { return };
        let (a, b) = self.timeline.view(v.dur);
        let (tx, rx) = channel();
        let crop = self.thumb_crop;
        ffmpeg::thumbnails(v.path.clone(), (a, b), THUMBS, THUMB, crop, tx, ctx.clone());
        self.thumbs = vec![None; THUMBS];
        self.thumbs_rx = Some(rx);
        self.timeline.thumb_range = (a, b);
        self.timeline.thumb_after = None;
    }

    /// Debounced filmstrip refresh after the timeline view changes.
    fn schedule_thumbs(&mut self) {
        self.timeline.thumb_after = Some(Instant::now() + Duration::from_millis(140));
    }

    /// Apply a new timeline view, refreshing thumbnails only if the visible range changed.
    fn set_timeline(&mut self, zoom_level: f32, center: f64, dur: f64) {
        let before = self.timeline.view(dur);
        self.timeline.zoom = zoom_level;
        self.timeline.center = center;
        let after = self.timeline.view(dur);
        if (before.0 - after.0).abs() > 1e-6 || (before.1 - after.1).abs() > 1e-6 {
            self.schedule_thumbs();
        }
    }

    fn zoom_timeline(&mut self, factor: f64, dur: f64) {
        let (z, c) = zoom::timeline_zoom(dur, self.timeline.zoom, factor, self.pos(), 0.5);
        self.set_timeline(z, c, dur);
    }

    fn zoom_timeline_at(&mut self, factor: f64, anchor: f64, fraction: f64, dur: f64) {
        let (z, c) = zoom::timeline_zoom(dur, self.timeline.zoom, factor, anchor, fraction);
        self.set_timeline(z, c, dur);
    }

    fn fit_timeline(&mut self, dur: f64) {
        let (z, c) = zoom::timeline_fit(dur);
        self.set_timeline(z, c, dur);
    }

    fn fit_timeline_selection(&mut self, dur: f64) {
        let (z, c) = zoom::timeline_fit_selection(dur, self.trim.0, self.trim.1);
        self.set_timeline(z, c, dur);
    }

    fn pan_timeline(&mut self, t: f64, dur: f64) {
        self.set_timeline(self.timeline.zoom, t, dur);
    }

    /// While cropping, never zoom or pan past the crop box.
    fn contain_crop(&mut self) {
        if let Some(c) = self.crop {
            (self.video_zoom, self.video_center) =
                zoom::contain(self.video_zoom, self.video_center, c);
        }
    }

    fn video_src(&self) -> Rect {
        zoom::video_src(self.video_zoom, self.video_center)
    }

    /// The part of the frame on screen, in whole-frame coordinates: video zoom works inside an
    /// applied crop (Crop panel closed), or inside the whole frame while it is open.
    fn shown_src(&self) -> Rect {
        let vsrc = self.video_src();
        match self.crop {
            Some(c) if self.panel != Some(Panel::Crop) => Rect::from_min_size(
                c.min + vsrc.min.to_vec2() * c.size(),
                vsrc.size() * c.size(),
            ),
            _ => vsrc,
        }
    }

    fn zoom_video(&mut self, factor: f32, anchor: Option<(Pos2, Rect)>) {
        let (z, c) = zoom::video_zoom(self.video_zoom, self.video_center, factor, anchor);
        self.video_zoom = z;
        self.video_center = c;
    }

    fn pan_video(&mut self, delta: Vec2, view: Rect) {
        self.video_center = zoom::video_pan(self.video_center, delta, view.size(), self.video_zoom);
    }

    fn reset_video_zoom(&mut self) {
        self.video_zoom = 1.0;
        self.video_center = pos2(0.5, 0.5);
    }

    fn set_speed(&mut self, speed: f64) {
        self.speed = speed.clamp(SPEED_MIN, SPEED_MAX);
        let _ = self.player.mpv.set_property("speed", self.speed);
    }

    /// Approximate output duration after trim, strip and retiming. `None` means the strip scan is missing.
    fn output_length(&self) -> Option<f64> {
        let trim = (self.trim.1 - self.trim.0).max(0.0);
        let source = if self.strip.enabled {
            let scan = self.strip.scan.as_ref()?;
            let keep = strip::keep(&scan.active, self.strip.pad, self.trim.0, self.trim.1);
            keep.iter().map(|&(a, b)| (b - a).max(0.0)).sum()
        } else {
            trim
        };
        Some(source / self.speed.max(0.001))
    }

    /// Whether the export can stream-copy (mirrors `ffmpeg::run`). It may still fall back.
    fn stream_copy(&self) -> bool {
        let crop = match (self.crop, &self.video) {
            (Some(c), Some(v)) => {
                let [w, h] = v.size;
                crop::to_pixels(c, w, h) != [w, h, 0, 0]
            }
            _ => false,
        };
        !crop
            && !self.precise
            && self.quality == 0
            && !self.strip.enabled
            && (self.speed - 1.0).abs() < 1e-4
    }

    fn nudge_speed(&mut self, up: bool) {
        let current = self.speed;
        let next = if up {
            SPEEDS
                .iter()
                .copied()
                .find(|&s| s > current + 1e-6)
                .unwrap_or(SPEED_MAX)
        } else {
            SPEEDS
                .iter()
                .rev()
                .copied()
                .find(|&s| s < current - 1e-6)
                .unwrap_or(SPEED_MIN)
        };
        self.set_speed(next);
    }

    /// Normalized width/height lock for the current crop preset.
    fn lock(&self) -> Option<f32> {
        let aspect = self.video.as_ref()?.aspect;
        crop::PRESETS[self.preset]
            .1
            .map(|r| if r == 0.0 { 1.0 } else { crop::lock(r, aspect) })
    }

    /// Apply a preset. Free keeps the current rectangle and only removes the aspect lock.
    fn set_crop_preset(&mut self, preset: usize) {
        self.preset = preset;
        let lock = self.lock();
        if let Some(current) = self.crop {
            self.crop = Some(crop_for_preset(preset, current, lock));
        }
    }

    fn pos(&self) -> f64 {
        self.player.props.pos
    }

    fn paused(&self) -> bool {
        self.player.props.paused
    }

    fn toggle_play(&mut self) {
        let paused = self.paused();
        // At end of file mpv ignores unpausing, and the last frame can start well before
        // the out point on low-frame-rate clips, so restart from IN there too.
        let outside = !(self.trim.0..self.trim.1 - 0.05).contains(&self.pos());
        if paused && (outside || self.player.props.eof) {
            self.player.seek(self.trim.0);
        }
        let _ = self.player.mpv.set_property("pause", !paused);
    }

    fn set_in(&mut self, t: f64) {
        // `max`/`min` rather than `clamp`: a clip shorter than MIN_LEN (or of unknown length)
        // would give clamp an inverted range, which panics.
        self.trim.0 = t.min(self.trim.1 - MIN_LEN).max(0.0);
        self.player.seek(self.trim.0);
    }

    fn set_out(&mut self, t: f64) {
        let dur = self.video.as_ref().map_or(0.0, |v| v.dur);
        self.trim.1 = t.max(self.trim.0 + MIN_LEN).min(dur);
        self.player.seek(self.trim.1);
    }

    fn skip(&mut self, by: f64) {
        let dur = self.video.as_ref().map_or(0.0, |v| v.dur);
        self.player.seek((self.pos() + by).clamp(0.0, dur));
    }

    fn export(&mut self, ctx: &egui::Context) {
        let Some(v) = &self.video else { return };
        if self.job.is_some() {
            return;
        }
        if self.strip.enabled {
            if self.strip.job.is_some() {
                self.status = Some((t!("status.wait_scan").into(), true));
                return;
            }
            if self.strip.scan.is_none() {
                self.status = Some((t!("status.scan_first").into(), true));
                return;
            }
        }
        let keep = if self.strip.enabled {
            self.kept()
        } else {
            None
        };
        if keep.as_ref().is_some_and(Vec::is_empty) {
            self.status = Some((t!("status.nothing_left").into(), true));
            return;
        }
        let ext = ffmpeg::FORMATS[self.format];
        let stem = v.path.file_stem().unwrap_or_default().to_string_lossy();
        let mut tag = if self.strip.enabled {
            "stripped".to_string()
        } else if self.quality == ffmpeg::COMPRESS {
            "compressed".to_string()
        } else {
            "edited".to_string()
        };
        if (self.speed - 1.0).abs() > 1e-4 {
            tag.push('-');
            tag.push_str(&speed_label(self.speed));
        }
        let mut dialog = rfd::FileDialog::new()
            .set_file_name(format!("{stem}-{tag}.{ext}"))
            .add_filter(ext.to_uppercase(), &[ext]);
        if let Some(dir) = v.path.parent() {
            dialog = dialog.set_directory(dir);
        }
        let Some(mut dst) = dialog.save_file() else {
            return;
        };
        if dst.extension().is_none() {
            dst.set_extension(ext);
        }
        if std::fs::canonicalize(&dst).ok() == std::fs::canonicalize(&v.path).ok() {
            self.status = Some((t!("status.same_file").into(), true));
            return;
        }
        let [fw, fh] = v.size;
        let crop = self
            .crop
            .map(|c| crop::to_pixels(c, fw, fh))
            .filter(|&c| c != [fw, fh, 0, 0]);
        let job = ffmpeg::Job {
            src: v.path.clone(),
            dst,
            start: self.trim.0,
            end: self.trim.1,
            crop,
            precise: self.precise,
            codec: v.codec.clone(),
            quality: self.quality,
            mute: self.mute,
            speed: self.speed,
            keep,
        };
        self.job = Some(ffmpeg::export(job, ctx.clone()));
        self.export_glide = Glide::default();
        self.exporting = Some(self.edits());
        self.progress = 0.0;
        self.status = None;
        let _ = self.player.mpv.set_property("pause", true);
    }

    fn poll(&mut self, ctx: &egui::Context) {
        let (loaded, failed) = self.player.poll();
        if loaded {
            self.loaded(ctx);
        }
        // Rotation and aspect are only known once the first frame is decoded.
        let geometry = self.video.is_some().then(|| self.geometry());
        if let (Some(v), Some(g)) = (&mut self.video, geometry) {
            (v.size, v.aspect) = g;
        }
        if let Some(e) = failed
            && let Some(p) = self.loading.take()
        {
            // mpv already dropped the previous file; showing it would leave a dead editor.
            if self.video.take().is_some() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Title("Glint".into()));
            }
            self.status = Some((t!("open.failed", path = p.display(), error = e), true));
        }
        if let Some(rx) = &self.thumbs_rx {
            for (i, img) in rx.try_iter() {
                self.thumbs[i] =
                    Some(ctx.load_texture(format!("thumb{i}"), img, Default::default()));
            }
        }
        if let Some(at) = self.timeline.thumb_after {
            let now = Instant::now();
            if now >= at {
                self.load_thumbs(ctx);
            } else {
                ctx.request_repaint_after(at.saturating_duration_since(now));
            }
        }
        if let Some(job) = &self.job {
            for msg in job.rx.try_iter() {
                match msg {
                    ffmpeg::Msg::Progress(p) => {
                        self.progress = p;
                        self.export_glide.reported(p as f64);
                    }
                    ffmpeg::Msg::Done(Ok(bytes)) => {
                        self.saved = self.exporting.take();
                        let msg = self.saved_message(bytes, job.compress);
                        self.status = Some((msg, false));
                        self.job = None;
                        break;
                    }
                    ffmpeg::Msg::Done(Err(e)) => {
                        self.status = Some((t!("status.export_failed", error = e), true));
                        self.job = None;
                        break;
                    }
                }
            }
        }
        if let Some(job) = &self.strip.job {
            let mut done = false;
            for msg in job.rx.try_iter() {
                match msg {
                    strip::Msg::Progress(p) => self.strip.progress = p,
                    strip::Msg::Partial {
                        from,
                        intervals,
                        scanned,
                    } => {
                        self.strip.partial.truncate(from);
                        self.strip.partial.extend(intervals);
                        self.strip.scanned = scanned;
                        self.strip.front.reported(scanned);
                    }
                    strip::Msg::Done(Ok(scan)) => {
                        if scan.active.is_empty() {
                            self.status = Some((t!("status.no_activity").into(), false));
                        }
                        if let Some(v) = &self.video {
                            self.scans.insert(
                                v.path.clone(),
                                (self.strip.zones.clone(), self.strip.scan_crop, scan.clone()),
                            );
                        }
                        self.strip.scan = Some(scan);
                        self.strip.progress = 1.0;
                        done = true;
                        break;
                    }
                    strip::Msg::Done(Err(e)) => {
                        self.status = Some((t!("status.scan_failed", error = e), true));
                        done = true;
                        break;
                    }
                }
            }
            if done {
                self.strip.job = None;
                self.strip.partial.clear();
            }
        }
        // Stop at the out point, like Quick Look's trim preview.
        if self.video.is_some() && !self.paused() && self.pos() >= self.trim.1 {
            let _ = self.player.mpv.set_property("pause", true);
        }
    }

    /// Plain "Saved", or for compression how much smaller the file got. The percentage is
    /// only shown for a whole-file export: a trimmed clip is smaller than the source anyway.
    fn saved_message(&self, bytes: u64, compress: bool) -> String {
        if !compress || bytes == 0 {
            return t!("status.saved").into();
        }
        let Some(v) = &self.video else {
            return t!("status.saved").into();
        };
        let src = std::fs::metadata(&v.path).map_or(0, |m| m.len());
        let whole = self.trim.0 <= 0.01 && self.trim.1 >= v.dur - 0.01;
        if whole && src > bytes {
            let pct = (src - bytes) as f64 / src as f64 * 100.0;
            t!(
                "status.saved_smaller",
                pct = format!("{pct:.0}"),
                size = fmt_size(bytes)
            )
        } else {
            t!("status.saved_size", size = fmt_size(bytes))
        }
    }

    /// Show a scan for the current file and ignored areas: the one already loaded, a cached one,
    /// or a new one if neither exists. A scan already running is left alone.
    fn ensure_scan(&mut self, ctx: &egui::Context) {
        let current = self.strip.scan_crop == self.crop;
        if current && (self.strip.scan.is_some() || self.strip.job.is_some()) {
            return;
        }
        // Made for another crop: it no longer describes what will be exported.
        self.cancel_scan();
        self.strip.scan = None;
        let Some(v) = &self.video else { return };
        if let Some((zones, crop, scan)) = self.scans.get(&v.path)
            && *zones == self.strip.zones
            && *crop == self.crop
        {
            self.strip.scan_crop = self.crop;
            self.strip.scan = Some(scan.clone());
            self.strip.progress = 1.0;
            return;
        }
        self.start_scan(ctx);
    }

    /// Scan now, replacing any result (the Rescan button).
    fn start_scan(&mut self, ctx: &egui::Context) {
        let Some(v) = &self.video else { return };
        if self.strip.job.is_some() {
            return;
        }
        if self.job.is_some() {
            self.status = Some((t!("status.wait_export").into(), true));
            return;
        }
        let (src, dur) = (v.path.clone(), v.dur);
        // Only the cropped part ends up in the export, so changes outside it don't count.
        let area = strip::Area {
            aspect: v.aspect,
            size: v.size,
            crop: self.crop,
            zones: self.strip.zones.clone(),
        };
        self.strip.scan_crop = self.crop;
        self.strip.scan = None;
        self.strip.progress = 0.0;
        self.strip.partial.clear();
        self.strip.scanned = 0.0;
        self.strip.front = Glide::default();
        self.status = None;
        self.strip.job = Some(strip::scan(src, dur, area, ctx.clone()));
    }

    fn cancel_scan(&mut self) {
        if let Some(job) = self.strip.job.take() {
            job.cancel();
        }
        self.strip.progress = 0.0;
        self.strip.partial.clear();
        self.strip.scanned = 0.0;
        self.strip.front = Glide::default();
    }

    /// Open the Strip panel (turning Strip on), or close it, which keeps Strip on.
    fn toggle_strip(&mut self, ctx: &egui::Context) {
        self.end_zone_edit();
        if self.panel == Some(Panel::Strip) {
            self.panel = None;
            return;
        }
        if self.panel == Some(Panel::Crop) {
            self.panel = None;
            self.crop_applied(ctx);
        }
        self.panel = Some(Panel::Strip);
        self.strip.enabled = true;
        // Rescans only if the crop (or the file) changed since the last scan.
        self.ensure_scan(ctx);
    }

    /// Turn Strip off (the bar's own button). Its scan stays saved for this session.
    fn strip_off(&mut self) {
        self.end_zone_edit();
        self.strip.enabled = false;
        if self.panel == Some(Panel::Strip) {
            self.panel = None;
        }
    }

    fn end_zone_edit(&mut self) {
        self.strip.edit = false;
        self.strip.selected = None;
        self.strip.drag = None;
        self.drag = None;
    }

    fn toggle_zone_edit(&mut self) {
        if !self.strip.enabled {
            return;
        }
        self.strip.edit = !self.strip.edit;
        self.strip.selected = None;
        self.strip.drag = None;
        self.drag = None;
        // Editing zones makes a running scan obsolete.
        if self.strip.edit && self.strip.job.is_some() {
            self.cancel_scan();
            self.strip.scan = None;
            self.status = Some((t!("status.scan_cancelled").into(), false));
        }
    }

    fn add_zone(&mut self) {
        if !self.strip.enabled {
            return;
        }
        // Middle of what's on screen, so it's visible when zoomed in or cropped.
        let shown = self.shown_src();
        let zone = Rect::from_center_size(shown.center(), shown.size() * 0.2);
        self.strip.zones.push(zone);
        self.strip.selected = Some(self.strip.zones.len() - 1);
        self.strip.edit = true;
        self.zones_changed();
    }

    fn clear_zones(&mut self) {
        if self.strip.zones.is_empty() {
            return;
        }
        self.strip.zones.clear();
        self.strip.selected = None;
        self.zones_changed();
    }

    fn remove_selected_zone(&mut self) {
        let Some(i) = self.strip.selected else {
            return;
        };
        if i < self.strip.zones.len() {
            self.strip.zones.remove(i);
            self.strip.selected = None;
            self.zones_changed();
        }
    }

    /// Any zone change invalidates a scan: the detected activity was measured with the old mask.
    fn zones_changed(&mut self) {
        let had = self.strip.scan.is_some() || self.strip.job.is_some();
        self.cancel_scan();
        self.strip.scan = None;
        if had {
            self.status = Some((t!("status.zones_changed").into(), false));
        }
    }

    /// Intervals of the current trim that survive stripping. `None` until a scan has run.
    fn kept(&self) -> Option<Vec<(f64, f64)>> {
        let scan = self.strip.scan.as_ref()?;
        Some(strip::keep(
            &scan.active,
            self.strip.pad,
            self.trim.0,
            self.trim.1,
        ))
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if let Some(p) = ctx.input(|i| i.raw.dropped_files.first().map(|f| f.path().to_path_buf()))
        {
            self.request_open(p);
        }
        // A question in the modal takes the keyboard until it is answered.
        if self.confirm.is_some() {
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                self.confirm = None;
            }
            return;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::O)) {
            self.open_dialog();
        }
        if self.video.is_none() {
            return;
        }
        // Same as the Export button: open the settings; pressed again, save.
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::S)) {
            if self.pop == Some(Pop::Export) {
                self.pop = None;
                self.export(ctx);
            } else {
                self.pop = Some(Pop::Export);
            }
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::I)) {
            self.close_panel(ctx);
            self.toggle_pop(Pop::Info);
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::W)) {
            self.request_close(ctx);
            return;
        }
        // Typing in a time field: leave these keys to the text edit.
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let dur = self.video.as_ref().map_or(0.0, |v| v.dur);
        let key = |k| ctx.input_mut(|i| i.consume_key(Modifiers::NONE, k));
        if key(Key::Space) {
            self.toggle_play();
        }
        if key(Key::ArrowLeft) {
            self.player.cmd("frame-back-step", &[]);
        }
        if key(Key::ArrowRight) {
            self.player.cmd("frame-step", &[]);
        }
        if key(Key::I) {
            self.set_in(self.pos());
        }
        if key(Key::O) {
            self.set_out(self.pos());
        }
        if key(Key::C) {
            self.toggle_crop(ctx);
        }
        if key(Key::Z) && self.panel == Some(Panel::Strip) {
            self.toggle_zone_edit();
        }
        if key(Key::Plus)
            || key(Key::Equals)
            || ctx.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::Plus))
        {
            self.zoom_video(1.25, None);
        }
        if key(Key::Minus) {
            self.zoom_video(0.8, None);
        }
        if key(Key::Num0) {
            self.reset_video_zoom();
        }
        if key(Key::CloseBracket) {
            self.zoom_timeline(1.5, dur);
        }
        if key(Key::OpenBracket) {
            self.zoom_timeline(1.0 / 1.5, dur);
        }
        if key(Key::Comma) || ctx.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::Comma)) {
            self.nudge_speed(false);
        }
        if key(Key::Period) || ctx.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::Period)) {
            self.nudge_speed(true);
        }
        if self.strip.edit && (key(Key::Delete) || key(Key::Backspace)) {
            self.remove_selected_zone();
        }
        if key(Key::Escape) {
            if self.pop.is_some() {
                self.pop = None;
            } else if self.strip.edit {
                self.strip.edit = false;
                self.strip.selected = None;
            } else {
                self.close_panel(ctx);
            }
        }
    }

    /// Open the Crop panel (whole frame, selection box), or close it, which applies the crop:
    /// the preview then shows only the cropped part.
    fn toggle_crop(&mut self, ctx: &egui::Context) {
        self.drag = None;
        if self.panel == Some(Panel::Crop) {
            self.panel = None;
            // A box over the whole frame crops nothing: don't keep it as an applied crop.
            if self.crop.is_some_and(crop::is_full) {
                self.crop = None;
            }
            self.crop_applied(ctx);
            return;
        }
        self.end_zone_edit();
        self.panel = Some(Panel::Crop);
        if self.crop.is_none() {
            self.crop = Some(self.lock().map_or_else(crop::full, crop::fit));
        }
    }

    /// Close the open panel, keeping its effect.
    fn close_panel(&mut self, ctx: &egui::Context) {
        match self.panel {
            Some(Panel::Crop) => self.toggle_crop(ctx),
            Some(Panel::Strip) => self.toggle_strip(ctx),
            None => {}
        }
    }

    /// Remove the crop (the crop bar's own button).
    fn remove_crop(&mut self, ctx: &egui::Context) {
        self.crop = None;
        if self.panel == Some(Panel::Crop) {
            self.panel = None;
        }
        self.crop_applied(ctx);
    }

    /// The crop changed: a strip scan of another crop no longer fits. With Strip on, scan again.
    fn crop_applied(&mut self, ctx: &egui::Context) {
        // The filmstrip shows what will be exported.
        if self.thumb_crop != self.crop {
            self.thumb_crop = self.crop;
            self.load_thumbs(ctx);
        }
        let stale = (self.strip.scan.is_some() || self.strip.job.is_some())
            && self.strip.scan_crop != self.crop;
        if stale && self.strip.enabled {
            self.ensure_scan(ctx);
        } else if stale {
            self.cancel_scan();
            self.strip.scan = None;
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        #[cfg(target_os = "macos")]
        chrome::place_traffic_lights(frame, 52.0);
        #[cfg(target_os = "macos")]
        chrome::route_quit_to_close();
        #[cfg(not(target_os = "macos"))]
        let _ = frame;
        self.poll(&ctx);
        // Closing the window or quitting with unexported edits: ask first.
        if ctx.input(|i| i.viewport().close_requested()) && !self.quitting && self.unsaved() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pop = None;
            self.confirm = Some(Confirm::Quit);
        }
        self.shortcuts(&ctx);

        let header = egui::Panel::top("header")
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .inner_margin(egui::Margin::symmetric(16, 6)),
            )
            .show(ui, |ui| self.header(ui));
        let r = header.response.rect;
        self.anchors.header_bottom = r.bottom();
        ui.painter()
            .hline(r.x_range(), r.bottom(), Stroke::new(1.0, LINE));

        if self.video.is_some() {
            let deck = egui::Panel::bottom("deck")
                .show_separator_line(false)
                .frame(egui::Frame::new().fill(PANEL).inner_margin(egui::Margin {
                    left: 16,
                    right: 16,
                    top: 10,
                    bottom: 14,
                }))
                .show(ui, |ui| self.deck(ui));
            let r = deck.response.rect;
            ui.painter()
                .hline(r.x_range(), r.top(), Stroke::new(1.0, LINE));
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG))
            .show(ui, |ui| {
                if self.video.is_some() {
                    canvas(ui, ui.max_rect());
                    self.stage(ui);
                } else {
                    self.empty(ui);
                }
                self.status_pill(ui);
            });
        self.popover(&ctx);
        self.open_modal(&ctx);
        chrome::resize_edges(&ctx);
    }
}

/// Always dark. Forcing the theme matters: with the system preference, a light-mode OS gets
/// black text on the black stage.
fn theme(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    // Dark native title bar too, so it doesn't glow white above the video.
    ctx.send_viewport_cmd(egui::ViewportCommand::SetTheme(egui::SystemTheme::Dark));
    ctx.style_mut_of(egui::Theme::Dark, |s| {
        let v = &mut s.visuals;
        v.panel_fill = BG;
        v.window_fill = PANEL;
        v.extreme_bg_color = CARD;
        v.faint_bg_color = CARD;
        v.window_stroke = Stroke::new(1.0, LINE);
        v.window_corner_radius = 12.into();
        v.menu_corner_radius = 8.into();
        v.window_shadow = shadow();
        v.popup_shadow = shadow();
        v.selection.bg_fill = GREEN.gamma_multiply(0.18);
        v.selection.stroke = Stroke::new(1.0, GREEN);
        v.hyperlink_color = GREEN;
        v.override_text_color = None;
        v.weak_text_color = Some(MUTED);
        let w = &mut v.widgets;
        for (wv, fill, stroke, text) in [
            (&mut w.noninteractive, Color32::TRANSPARENT, LINE, TEXT),
            (&mut w.inactive, CARD, LINE, TEXT),
            (
                &mut w.hovered,
                CARD_HI,
                GREEN.gamma_multiply(0.5),
                Color32::WHITE,
            ),
            (&mut w.active, GREEN.gamma_multiply(0.18), GREEN, GREEN),
            (&mut w.open, CARD_HI, GREEN.gamma_multiply(0.7), GREEN),
        ] {
            wv.bg_fill = fill;
            wv.weak_bg_fill = fill;
            wv.bg_stroke = Stroke::new(1.0, stroke);
            wv.fg_stroke = Stroke::new(1.0, text);
            wv.corner_radius = 6.into();
            wv.expansion = 0.0;
        }
        let sp = &mut s.spacing;
        sp.item_spacing = vec2(6.0, 6.0);
        sp.button_padding = vec2(10.0, 4.0);
        sp.menu_margin = egui::Margin::same(6);
        sp.interact_size.y = 28.0;
        for (style, size) in [
            (egui::TextStyle::Small, 10.0),
            (egui::TextStyle::Body, 12.0),
            (egui::TextStyle::Button, 12.0),
            (egui::TextStyle::Monospace, 12.0),
            (egui::TextStyle::Heading, 16.0),
        ] {
            s.text_styles.insert(style, FontId::monospace(size));
        }
    });
}

fn bold(size: f32) -> FontId {
    FontId::new(size, egui::FontFamily::Name("bold".into()))
}

fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

/// Deep drop shadow under popovers.
fn shadow() -> egui::Shadow {
    egui::Shadow {
        offset: [0, 24],
        blur: 60,
        spread: 0,
        color: Color32::from_black_alpha(180),
    }
}

/// Soft green light behind an element, like a phosphor glow.
fn glow(color: Color32, blur: u8) -> egui::Shadow {
    egui::Shadow {
        offset: [0, 0],
        blur,
        spread: 0,
        color,
    }
}

/// The video canvas: black with a faint green center and a quiet dot grid.
fn canvas(ui: &Ui, rect: Rect) {
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0.0, BG);
    let core = Rect::from_center_size(rect.center(), rect.size() * 0.45);
    p.add(glow(Color32::from_rgb(4, 17, 10), 255).as_shape(core, 200));
    let dot = GREEN.gamma_multiply(0.07);
    let mut y = rect.min.y + 12.0;
    while y < rect.max.y {
        let mut x = rect.min.x + 12.0;
        while x < rect.max.x {
            p.circle_filled(pos2(x, y), 1.0, dot);
            x += 24.0;
        }
        y += 24.0;
    }
}

/// A few streams of digital rain falling behind the start screen, fading toward the drop target.
/// Deterministic in `time`, so it needs no state: each stream picks a new column every pass.
fn rain(p: &egui::Painter, rect: Rect, clear: Rect, time: f64) {
    const GLYPHS: &[char] = &[
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F', ':', '.',
        '=', '*', '+', '<', '>', '/',
    ];
    const STREAMS: u64 = 7;
    const LINE: f32 = 19.0;
    // splitmix64: a cheap, well-spread hash of small integers.
    let hash = |mut x: u64| {
        x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
        x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        x ^ (x >> 31)
    };
    let font = mono(15.0);
    let c = rect.center();
    let cols = ((rect.width() / 22.0) as u64).max(1);
    for s in 0..STREAMS {
        let speed = 60.0 + (hash(s * 3) % 70) as f32;
        let len = 8 + hash(s * 3 + 1) % 12;
        let tail = len as f32 * LINE;
        let travel = rect.height() + tail;
        let phase = time as f32 * speed + (hash(s * 3 + 2) % 10_000) as f32;
        let pass = (phase / travel) as u64;
        let head = rect.min.y + phase % travel;
        let x = rect.min.x + 11.0 + (hash(s * 7919 + pass) % cols) as f32 * 22.0;
        // Glyphs sit on a grid and flicker to a new character a few times a second.
        let flicker = (time * 6.0) as u64;
        for k in 0..len {
            let y = head - k as f32 * LINE;
            if !rect.y_range().contains(y) || clear.contains(pos2(x, y + LINE / 2.0)) {
                continue;
            }
            // Clear around the drop target, full strength toward the edges.
            let d = (((x - c.x) / (rect.width() / 2.0)).powi(2)
                + ((y - c.y) / (rect.height() / 2.0)).powi(2))
            .sqrt();
            let mask = ((d - 0.28) / 0.42).clamp(0.0, 1.0);
            let fade = 1.0 - k as f32 / len as f32;
            let a = 0.28 * fade * mask;
            if a < 0.005 {
                continue;
            }
            let row = ((y - rect.min.y) / LINE) as u64;
            let g = GLYPHS[(hash(x as u64 * 131 + row * 7 + flicker / (1 + k % 3))
                % GLYPHS.len() as u64) as usize];
            let color = if k == 0 {
                Color32::from_rgb(217, 255, 233)
            } else {
                GREEN
            };
            p.text(
                pos2(x, y),
                Align2::CENTER_TOP,
                g,
                font.clone(),
                color.gamma_multiply(a),
            );
        }
    }
}

/// What sits under `p` in the ignore-zone editor: a zone corner, a zone body, or nothing.
fn zone_hit(zones: &[Rect], vr: Rect, src: Rect, p: Pos2) -> Option<(usize, Drag)> {
    for (i, z) in zones.iter().enumerate().rev() {
        let r = Rect::from_min_max(
            zoom::source_to_screen(src, vr, z.min),
            zoom::source_to_screen(src, vr, z.max),
        );
        for (hx, hy) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
            let corner = pos2(
                if hx < 0 { r.min.x } else { r.max.x },
                if hy < 0 { r.min.y } else { r.max.y },
            );
            if corner.distance(p) < 16.0 {
                return Some((i, Drag::Crop(hx, hy)));
            }
        }
        if r.contains(p) {
            return Some((i, Drag::Move));
        }
    }
    None
}

/// Drag moves the window; double-click maximizes or restores it, like a native titlebar.
fn title_bar(resp: &Response) {
    if resp.drag_started() {
        resp.ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if resp.double_clicked() {
        chrome::press(&resp.ctx, chrome::Button::Maximize);
    }
}

/// Minimize/maximize/close, in the order given (reverse it inside a right-to-left layout).
fn window_buttons<'a>(ui: &mut Ui, buttons: impl Iterator<Item = &'a chrome::Button>) {
    let max = chrome::maximized(ui.ctx());
    for &b in buttons {
        let (glyph, label) = chrome::look(b, max);
        if icon_button(ui, glyph, label, false).clicked() {
            chrome::press(ui.ctx(), b);
        }
    }
}

/// Size and colors of a hand-drawn button. One painter for every button keeps them consistent.
#[derive(Clone, Copy)]
struct Look {
    height: f32,
    radius: f32,
    text: f32,
    icon: f32,
    /// Horizontal padding; icon-only buttons are square instead.
    pad: f32,
    /// Label color when idle (icons stay muted).
    idle: Color32,
    /// Space between icon and label.
    gap: f32,
}

/// Header mode toggles.
const MODE: Look = Look {
    height: 32.0,
    radius: 6.0,
    text: 11.5,
    icon: 13.5,
    pad: 13.0,
    idle: MUTED,
    gap: 6.0,
};
/// Chips and buttons in the floating bars.
const CHIP: Look = Look {
    height: 26.0,
    radius: 5.0,
    text: 11.0,
    icon: 14.0,
    pad: 10.0,
    idle: MUTED,
    gap: 7.0,
};
/// Labeled action in a floating bar ("Ignore area").
const GHOST: Look = Look { idle: TEXT, ..CHIP };
/// Icon buttons in the header and transport.
const ICON: Look = Look {
    height: 32.0,
    radius: 6.0,
    text: 12.0,
    icon: 17.0,
    pad: 0.0,
    idle: MUTED,
    gap: 7.0,
};
/// Height of every group and button in the transport row (speed, zoom, IN/OUT/LEN, icons).
const DECK_H: f32 = 30.0;
/// Icon buttons in the transport row.
const DECK_ICON: Look = Look {
    height: DECK_H,
    ..ICON
};
/// Icon buttons inside floating bars.
const ICON_SMALL: Look = Look {
    height: 26.0,
    icon: 15.0,
    ..ICON
};

/// Icon and/or label button. Selected: green text on a faint green fill with a green edge.
fn button(ui: &mut Ui, glyph: &str, label: &str, selected: bool, look: Look) -> Response {
    let p = ui.painter();
    let icon_g = (!glyph.is_empty())
        .then(|| p.layout_no_wrap(glyph.into(), mono(look.icon), Color32::PLACEHOLDER));
    let text_g = (!label.is_empty())
        .then(|| p.layout_no_wrap(label.into(), mono(look.text), Color32::PLACEHOLDER));
    let gap = if icon_g.is_some() && text_g.is_some() {
        look.gap
    } else {
        0.0
    };
    let content = icon_g.as_ref().map_or(0.0, |g| g.size().x)
        + gap
        + text_g.as_ref().map_or(0.0, |g| g.size().x);
    let width = if text_g.is_none() {
        look.height.max(content + 8.0)
    } else {
        content + look.pad * 2.0
    };
    let (rect, resp) = ui.allocate_exact_size(vec2(width, look.height), Sense::click());
    let hovered = resp.hovered();
    let (fill, stroke, icon_c, text_c) = if selected {
        (
            GREEN.gamma_multiply(0.11),
            Stroke::new(1.0, GREEN.gamma_multiply(0.5)),
            GREEN,
            GREEN,
        )
    } else if hovered {
        (CARD_HI, Stroke::NONE, TEXT, TEXT)
    } else {
        (Color32::TRANSPARENT, Stroke::NONE, MUTED, look.idle)
    };
    let p = ui.painter();
    p.rect(rect, look.radius, fill, stroke, egui::StrokeKind::Inside);
    let mut x = rect.center().x - content / 2.0;
    if let Some(g) = icon_g {
        let w = g.size().x;
        p.galley_with_override_text_color(pos2(x, rect.center().y - g.size().y / 2.0), g, icon_c);
        x += w + gap;
    }
    if let Some(g) = text_g {
        p.galley_with_override_text_color(pos2(x, rect.center().y - g.size().y / 2.0), g, text_c);
    }
    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let name = if label.is_empty() { glyph } else { label };
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, name));
    resp
}

/// Icon-only button with a tooltip and a screen-reader label.
fn icon_button(ui: &mut Ui, glyph: &str, label: &str, selected: bool) -> Response {
    icon_button_look(ui, glyph, label, selected, ICON)
}

fn icon_button_look(ui: &mut Ui, glyph: &str, label: &str, selected: bool, look: Look) -> Response {
    let resp = button(ui, glyph, "", selected, look);
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, label)
    });
    resp.on_hover_text(label)
}

/// Filled green call to action with a soft glow. `active` rings it while its popover is open.
fn primary(ui: &mut Ui, glyph: &str, label: &str, size: Vec2, active: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    p.add(glow(GREEN.gamma_multiply(0.3), 22).as_shape(rect, 8));
    if active {
        p.rect_stroke(
            rect.expand(3.0),
            10.0,
            Stroke::new(3.0, GREEN.gamma_multiply(0.25)),
            egui::StrokeKind::Inside,
        );
    }
    let fill = if resp.is_pointer_button_down_on() {
        GREEN.gamma_multiply(0.8)
    } else if resp.hovered() {
        Color32::from_rgb(120, 255, 180)
    } else {
        GREEN
    };
    p.rect_filled(rect, 8.0, fill);
    let icon_g = p.layout_no_wrap(glyph.into(), mono(14.0), INK);
    let text_g = p.layout_no_wrap(label.into(), bold(12.5), INK);
    let w = icon_g.size().x + 7.0 + text_g.size().x;
    let x = rect.center().x - w / 2.0;
    let iw = icon_g.size().x;
    p.galley(
        pos2(x, rect.center().y - icon_g.size().y / 2.0),
        icon_g,
        INK,
    );
    p.galley(
        pos2(x + iw + 7.0, rect.center().y - text_g.size().y / 2.0),
        text_g,
        INK,
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    resp
}

/// Green-outlined contextual action ("Rescan").
fn accent(ui: &mut Ui, glyph: &str, label: &str) -> Response {
    let p = ui.painter();
    let text = if label.is_empty() {
        glyph.to_string()
    } else {
        format!("{glyph}  {label}")
    };
    let g = p.layout_no_wrap(text, mono(11.0), GREEN);
    let (rect, resp) =
        ui.allocate_exact_size(vec2((g.size().x + 20.0).max(30.0), 26.0), Sense::click());
    let fill = if resp.hovered() {
        GREEN.gamma_multiply(0.18)
    } else {
        GREEN.gamma_multiply(0.1)
    };
    let p = ui.painter();
    p.rect(
        rect,
        6.0,
        fill,
        Stroke::new(1.0, GREEN.gamma_multiply(0.6)),
        egui::StrokeKind::Inside,
    );
    p.galley(rect.center() - g.size() / 2.0, g, GREEN);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    resp
}

/// Round green play/pause button. The glyphs are drawn, so they sit dead center.
fn play_button(ui: &mut Ui, paused: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(38.0, 38.0), Sense::click());
    let p = ui.painter();
    let c = rect.center();
    p.add(glow(GREEN.gamma_multiply(0.4), 18).as_shape(rect, 19));
    let fill = if resp.hovered() {
        Color32::from_rgb(120, 255, 180)
    } else {
        GREEN
    };
    p.circle_filled(c, 19.0, fill);
    if paused {
        // Optically centered: a triangle's visual center sits right of its box center.
        let pts = vec![
            c + vec2(-4.5, -7.0),
            c + vec2(7.5, 0.0),
            c + vec2(-4.5, 7.0),
        ];
        p.add(egui::Shape::convex_polygon(pts, INK, Stroke::NONE));
    } else {
        for dx in [-4.0, 4.0] {
            p.rect_filled(
                Rect::from_center_size(c + vec2(dx, 0.0), vec2(4.0, 13.0)),
                1.0,
                INK,
            );
        }
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let label = if paused {
        t!("tip.play")
    } else {
        t!("tip.pause")
    };
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    resp.on_hover_text(label)
}

/// Small animated switch.
fn switch(ui: &mut Ui, on: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(38.0, 22.0), Sense::click());
    let t = ui.ctx().animate_bool(resp.id, on);
    let p = ui.painter();
    let (fill, edge) = if on {
        (GREEN.gamma_multiply(0.85), GREEN)
    } else {
        (CARD, LINE)
    };
    p.rect(
        rect,
        11.0,
        fill,
        Stroke::new(1.0, edge),
        egui::StrokeKind::Inside,
    );
    let cx = rect.left() + 11.0 + t * (rect.width() - 22.0);
    p.circle_filled(pos2(cx, rect.center().y), 7.0, if on { INK } else { MUTED });
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Checkbox,
            true,
            on,
            if on {
                t!("common.on")
            } else {
                t!("common.off")
            },
        )
    });
    resp
}

/// Equal-width segmented choice across the available width. Returns the clicked index.
fn segmented(ui: &mut Ui, id: &str, options: &[String], selected: usize) -> Option<usize> {
    let (outer, _) = ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::hover());
    let p = ui.painter();
    p.rect(
        outer,
        8.0,
        CARD,
        Stroke::new(1.0, LINE),
        egui::StrokeKind::Inside,
    );
    let inner = outer.shrink(3.0);
    let w = (inner.width() - 2.0 * (options.len() as f32 - 1.0)) / options.len() as f32;
    let mut clicked = None;
    for (i, label) in options.iter().enumerate() {
        let r = Rect::from_min_size(
            pos2(inner.min.x + i as f32 * (w + 2.0), inner.min.y),
            vec2(w, inner.height()),
        );
        let resp = ui.interact(r, ui.id().with(id).with(i), Sense::click());
        let on = i == selected;
        let (fill, stroke, color) = if on {
            (
                GREEN.gamma_multiply(0.11),
                Stroke::new(1.0, GREEN.gamma_multiply(0.5)),
                GREEN,
            )
        } else if resp.hovered() {
            (CARD_HI, Stroke::NONE, TEXT)
        } else {
            (Color32::TRANSPARENT, Stroke::NONE, MUTED)
        };
        let p = ui.painter();
        p.rect(r, 6.0, fill, stroke, egui::StrokeKind::Inside);
        p.text(r.center(), Align2::CENTER_CENTER, label, mono(12.0), color);
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        resp.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, on, label)
        });
        if resp.clicked() {
            clicked = Some(i);
        }
    }
    clicked
}

/// Muted label with a dim green icon, above a setting.
fn field_label(ui: &mut Ui, glyph: &str, text: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.label(RichText::new(glyph).font(mono(15.0)).color(GREEN_DIM));
        ui.label(RichText::new(text).font(mono(12.0)).color(MUTED));
    });
}

/// Icon, label and a switch on the right. Returns true when toggled.
fn toggle_row(ui: &mut Ui, glyph: &str, title: &str, hint: &str, on: bool) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.label(RichText::new(glyph).font(mono(15.0)).color(GREEN_DIM));
        ui.label(RichText::new(title).font(mono(12.0)).color(TEXT))
            .on_hover_text(hint);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if switch(ui, on).on_hover_text(hint).clicked() {
                changed = true;
            }
        });
    });
    changed
}

/// Keyboard key drawn as a key cap.
fn keycap(p: &egui::Painter, at: Pos2, key: &str) -> f32 {
    let g = p.layout_no_wrap(key.into(), mono(11.0), TEXT);
    let r = Rect::from_min_size(at, vec2((g.size().x + 12.0).max(22.0), 22.0));
    p.rect(
        r,
        5.0,
        CARD,
        Stroke::new(1.0, LINE),
        egui::StrokeKind::Inside,
    );
    p.hline(
        r.x_range().shrink(3.0),
        r.max.y - 1.0,
        Stroke::new(1.0, LINE),
    );
    p.galley(r.center() - g.size() / 2.0, g, TEXT);
    r.width()
}

/// Framed row of controls (crop ratios, strip, video zoom), `height` tall overall. Returns its
/// width.
fn bar_frame(ui: &mut Ui, height: f32, add: impl FnOnce(&mut Ui)) -> f32 {
    let margin = ((height - 26.0) / 2.0).max(0.0);
    egui::Frame::new()
        .fill(GLASS)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(7)
        .inner_margin(margin)
        .show(ui, |ui| {
            // Without this the frame grows to the row's full height.
            ui.set_height(26.0);
            ui.spacing_mut().item_spacing.x = 2.0;
            add(ui);
        })
        .response
        .rect
        .width()
}

/// Fill the first `frac` of `bar`. Painted at full size and clipped, so near 0% it is a sliver
/// of the rounded end, not a rounded blob wider than the progress it shows.
fn progress_fill(p: &egui::Painter, bar: Rect, frac: f32, radius: f32, color: Color32) {
    let mut done = bar;
    done.max.x = bar.min.x + bar.width() * frac.clamp(0.0, 1.0);
    if done.width() > 0.0 {
        p.with_clip_rect(done.intersect(p.clip_rect()))
            .rect_filled(bar, radius, color);
    }
}

/// Thin vertical separator.
fn vline(ui: &mut Ui, height: f32) {
    let (r, _) = ui.allocate_exact_size(vec2(1.0, height), Sense::hover());
    ui.painter()
        .vline(r.center().x, r.y_range(), Stroke::new(1.0, LINE));
}

/// The Glint mark: the app icon's trim box in miniature, with grip handles and a playhead.
fn logo(ui: &mut Ui) -> Response {
    let (r, resp) = ui.allocate_exact_size(vec2(30.0, 24.0), Sense::hover());
    let p = ui.painter();
    let b = Rect::from_center_size(r.center(), vec2(28.0, 16.0));
    p.add(glow(GREEN.gamma_multiply(0.3), 10).as_shape(b, 4));
    p.rect_filled(b, 4.0, GREEN);
    let inner = Rect::from_min_max(b.min + vec2(5.0, 2.5), b.max - vec2(5.0, 2.5));
    p.rect_filled(inner, 1.0, CARD);
    for i in 1..3 {
        let x = inner.min.x + inner.width() * i as f32 / 3.0;
        p.vline(
            x,
            inner.y_range().shrink(1.5),
            Stroke::new(1.0, GREEN.gamma_multiply(0.3)),
        );
    }
    let grips = b.y_range().shrink(5.0);
    for x in [b.min.x + 2.5, b.max.x - 2.5] {
        p.vline(x, grips, Stroke::new(1.0, INK));
    }
    let x = inner.min.x + inner.width() * 0.4;
    p.vline(x, b.y_range().expand(3.0), Stroke::new(1.5, Color32::WHITE));
    resp
}

/// File name over its codec/size/duration line, painted with tighter line spacing than labels
/// so both fit the header row. Long names are cut with an ellipsis.
fn file_block(ui: &mut Ui, name: &str, meta: &[(&str, String)]) {
    let p = ui.painter();
    let mut job = egui::text::LayoutJob::single_section(
        name.into(),
        egui::TextFormat::simple(bold(13.0), TEXT),
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width(320.0);
    let name_g = p.layout_job(job);
    let parts: Vec<_> = meta
        .iter()
        .flat_map(|(glyph, text)| {
            [
                p.layout_no_wrap((*glyph).into(), mono(11.0), GREEN_DIM),
                p.layout_no_wrap(text.clone(), mono(11.0), MUTED),
            ]
        })
        .collect();
    let meta_w: f32 = parts.iter().map(|g| g.size().x).sum::<f32>()
        + 5.0 * parts.len() as f32
        + 8.0 * (meta.len() as f32 - 1.0);
    let w = name_g.size().x.max(meta_w);
    let (r, _) = ui.allocate_exact_size(vec2(w, HEADER_ROW.min(32.0)), Sense::hover());
    let p = ui.painter();
    let name_h = name_g.size().y;
    p.galley(pos2(r.min.x, r.center().y - 1.0 - name_h), name_g, TEXT);
    let mut x = r.min.x;
    for (i, g) in parts.into_iter().enumerate() {
        let gw = g.size().x;
        p.galley(pos2(x, r.center().y + 1.0), g, MUTED);
        x += gw + if i % 2 == 1 { 13.0 } else { 5.0 };
    }
}

/// Small green dot on a mode button whose effect is applied while its panel is closed.
fn applied_dot(ui: &Ui, button: Rect, on: bool) {
    if on {
        ui.painter()
            .circle_filled(button.right_top() + vec2(-6.0, 6.0), 2.5, GREEN);
    }
}

/// Diagonal red hatching for stretches strip will cut.
fn hatch(p: &egui::Painter, r: Rect) {
    let p = p.with_clip_rect(r.intersect(p.clip_rect()));
    p.rect_filled(r, 0.0, Color32::from_rgba_premultiplied(40, 0, 8, 140));
    let s = Stroke::new(2.5, RED.gamma_multiply(0.42));
    let mut x = r.min.x - r.height();
    while x < r.max.x {
        p.line_segment([pos2(x, r.max.y), pos2(x + r.height(), r.min.y)], s);
        x += 8.0;
    }
    p.vline(r.min.x, r.y_range(), Stroke::new(1.0, RED));
    p.vline(r.max.x, r.y_range(), Stroke::new(1.0, RED));
}

/// Editable time: drag to nudge, click to type. Returns true when the value changed.
fn time_field(
    ui: &mut Ui,
    t: &mut f64,
    max: f64,
    prefix: impl Into<egui::WidgetText>,
    hint: &str,
) -> bool {
    ui.add(
        egui::DragValue::new(t)
            .range(0.0..=max)
            .speed(0.01)
            .prefix(prefix.into())
            .custom_formatter(|t, _| fmt_time(t))
            .custom_parser(parse_time)
            // Half-typed values like "1" would otherwise move the trim.
            .update_while_editing(false),
    )
    .on_hover_text(hint)
    .changed()
}

/// `trim` moved to start at `t`, same length, kept inside `0..=dur`.
fn slide(trim: (f64, f64), t: f64, dur: f64) -> (f64, f64) {
    let len = trim.1 - trim.0;
    let a = t.min(dur - len).max(0.0);
    (a, a + len)
}

pub(crate) fn fmt_time(t: f64) -> String {
    let ms = (t.max(0.0) * 1000.0).round() as u64;
    let (h, m, s, f) = (ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}.{f:03}")
    } else {
        format!("{m}:{s:02}.{f:03}")
    }
}

/// Display name of crop preset `i`; ratios like 16:9 read the same in every language.
fn preset_label(i: usize) -> &'static str {
    match crop::PRESETS[i].0 {
        "Free" => t!("crop.free"),
        "Original" => t!("crop.original"),
        ratio => ratio,
    }
}

/// Preset application: Free keeps the current crop; aspect presets fit a new centered rect.
fn crop_for_preset(preset: usize, current: Rect, lock: Option<f32>) -> Rect {
    if crop::PRESETS[preset].1.is_none() {
        current
    } else {
        lock.map_or_else(crop::full, crop::fit)
    }
}

/// Codec as people write it: "H.264", "HEVC", "ProRes".
fn codec_label(codec: &str) -> String {
    match codec {
        "h264" => "H.264".into(),
        "hevc" | "h265" => "HEVC".into(),
        "prores" => "ProRes".into(),
        "mpeg4" => "MPEG-4".into(),
        "mpeg2video" => "MPEG-2".into(),
        other => other.to_uppercase(),
    }
}

/// "100%", "250%".
fn zoom_label(z: f32) -> String {
    format!("{:.0}%", z * 100.0)
}

/// "0.5x", "1x", "1.25x".
fn speed_label(speed: f64) -> String {
    let mut text = format!("{speed:.2}");
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    format!("{text}x")
}

/// "1536" → "1.5 KB".
pub(crate) fn fmt_size(n: u64) -> String {
    let units = [t!("size.b"), t!("size.kb"), t!("size.mb"), t!("size.gb")];
    let mut v = n as f64;
    let mut unit = 0;
    while v >= 1024.0 && unit < units.len() - 1 {
        v /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{n} {}", units[0])
    } else {
        format!("{v:.1} {}", units[unit])
    }
}

/// Seconds from "s", "m:s" or "h:m:s", each with optional decimals.
fn parse_time(s: &str) -> Option<f64> {
    let parts: Vec<_> = s.trim().split(':').collect();
    if parts.len() > 3 {
        return None;
    }
    let t = parts.iter().try_fold(0.0, |acc, p| {
        Some(acc * 60.0 + p.trim().parse::<f64>().ok()?)
    })?;
    (t.is_finite() && t >= 0.0).then_some(t)
}

impl App {
    /// Start screen: a clickable drop target over faint digital rain, and the shortcuts below it.
    fn empty(&mut self, ui: &mut Ui) {
        let full = ui.max_rect();
        let p = ui.painter_at(full);
        p.rect_filled(full, 0.0, BG);

        let hot = ui.input(|i| !i.raw.hovered_files.is_empty());
        let loading = self.loading.is_some();
        // 16:9 when there is room; on short windows the box gives up height so the legend fits.
        let w = (full.width() - 120.0).clamp(320.0, 560.0);
        // The footer line takes the bottom 32 pt; the box and legend center in the rest.
        let footer = 32.0;
        let h = (w * 9.0 / 16.0)
            .min(full.height() - footer - 40.0 - 26.0 - LEGEND_H)
            .max(170.0);
        let block = h + 26.0 + LEGEND_H;
        let r = Rect::from_min_size(
            pos2(
                full.center().x - w / 2.0,
                full.center().y - footer / 2.0 - block / 2.0,
            ),
            vec2(w, h),
        );
        // The rain stays out of the drop box and the shortcut legend under it.
        let clear =
            Rect::from_min_max(r.min, pos2(r.max.x, r.max.y + 26.0 + LEGEND_H)).expand(18.0);
        let time = ui.input(|i| i.time);
        rain(&p, full, clear, time);
        // ~30 fps is plenty for falling glyphs, and only while the start screen shows.
        ui.ctx().request_repaint_after(Duration::from_millis(33));
        // A file dragged over it, or the pointer: light the target up, easing in and out.
        let lit = hot || (!loading && ui.rect_contains_pointer(r));
        let k = ui
            .ctx()
            .animate_bool_with_time(ui.id().with("drop_glow"), lit, 0.2);
        let mix = |a: f32, b: f32| a + (b - a) * k;
        p.add(glow(GREEN.gamma_multiply(mix(0.06, 0.14)), 80).as_shape(r, 14));
        p.rect_filled(r, 14.0, Color32::from_rgba_premultiplied(3, 9, 6, 235));
        let edge = if hot {
            GREEN
        } else {
            Color32::from_rgb(29, 74, 49)
        };
        let inset = r.shrink(14.0);
        for seg in [
            [pos2(inset.min.x, r.min.y), pos2(inset.max.x, r.min.y)],
            [pos2(inset.min.x, r.max.y), pos2(inset.max.x, r.max.y)],
            [pos2(r.min.x, inset.min.y), pos2(r.min.x, inset.max.y)],
            [pos2(r.max.x, inset.min.y), pos2(r.max.x, inset.max.y)],
        ] {
            p.extend(egui::Shape::dashed_line(
                &seg,
                Stroke::new(1.0, edge),
                4.0,
                4.0,
            ));
        }
        // Viewfinder corners with the same 14 pt rounding as the card.
        for (hx, hy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0f32)] {
            let corner = pos2(
                if hx < 0.0 { r.min.x } else { r.max.x },
                if hy < 0.0 { r.min.y } else { r.max.y },
            );
            let center = corner - vec2(hx, hy) * 14.0;
            let mut pts = vec![corner - vec2(0.0, hy * 26.0)];
            for i in 0..=8 {
                let a = i as f32 / 8.0 * std::f32::consts::FRAC_PI_2;
                pts.push(center + vec2(hx * a.cos(), hy * a.sin()) * 14.0);
            }
            pts.push(corner - vec2(hx * 26.0, 0.0));
            p.line(pts, Stroke::new(3.0, GREEN));
        }

        let tile = Rect::from_center_size(r.center() - vec2(0.0, 30.0), vec2(72.0, 72.0));
        let tile_glow = glow(GREEN.gamma_multiply(mix(0.15, 0.35)), mix(30.0, 40.0) as u8);
        p.add(tile_glow.as_shape(tile, 18));
        p.rect(
            tile,
            18.0,
            CARD,
            Stroke::new(1.0, LINE),
            egui::StrokeKind::Inside,
        );
        if loading {
            ui.put(
                tile.shrink(22.0),
                egui::Spinner::new().size(28.0).color(GREEN),
            );
        } else {
            let glyph = if hot {
                icon::DOWNLOAD_SIMPLE
            } else {
                icon::FILM_SLATE
            };
            p.text(
                tile.center(),
                Align2::CENTER_CENTER,
                glyph,
                mono(34.0),
                GREEN,
            );
        }
        let title = match (loading, hot) {
            (true, _) => t!("open.opening"),
            (_, true) => t!("open.release"),
            _ => t!("open.drop"),
        };
        let ty = tile.max.y + 28.0;
        p.text(
            pos2(r.center().x, ty),
            Align2::CENTER_CENTER,
            title,
            bold(17.0),
            if hot { GREEN } else { Color32::WHITE },
        );
        if !loading && !hot {
            p.text(
                pos2(r.center().x, ty + 24.0),
                Align2::CENTER_CENTER,
                t!("open.browse"),
                mono(12.0),
                MUTED,
            );
        }

        let click = ui.interact(r, ui.id().with("drop"), Sense::click());
        if click.clicked() && !loading {
            self.open_dialog();
        }
        if click.hovered() && !loading {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        click.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, t!("open.drop"))
        });

        // Shortcut legend: key caps, then an icon for what they do.
        let short = |m, k| ui.ctx().format_shortcut(&egui::KeyboardShortcut::new(m, k));
        let legend: [(Vec<String>, &str); 8] = [
            (vec![short(Modifiers::COMMAND, Key::O)], icon::FOLDER_OPEN),
            (vec![t!("key.space").into()], icon::PLAY),
            (vec!["←".into(), "→".into()], icon::FILM_STRIP),
            (vec!["I".into(), "O".into()], icon::SCISSORS),
            (vec!["C".into()], icon::CROP),
            (vec![",".into(), ".".into()], icon::GAUGE),
            (vec![short(Modifiers::COMMAND, Key::S)], icon::EXPORT),
            (vec![short(Modifiers::COMMAND, Key::W)], icon::X_CIRCLE),
        ];
        // Version and credit, on a black backing so the rain doesn't run through the text.
        let credit = p.layout_no_wrap(
            format!(
                "v{}  ·  {}",
                env!("CARGO_PKG_VERSION"),
                t!("credit", name = "zgv2")
            ),
            mono(11.0),
            MUTED,
        );
        let at = Rect::from_center_size(pos2(full.center().x, full.max.y - 16.0), credit.size());
        p.rect_filled(at.expand2(vec2(14.0, 6.0)), 6.0, BG);
        p.galley(at.min, credit, MUTED);

        // Two rows of four in equal-width columns, narrower than the drop box. In each cell the
        // key caps are right-aligned to a shared edge, so the icons line up across rows.
        let measure =
            |s: &str| (p.layout_no_wrap(s.into(), mono(11.0), TEXT).size().x + 12.0).max(22.0);
        let keys_w = |keys: &[String]| {
            keys.iter().map(|k| measure(k)).sum::<f32>() + 4.0 * (keys.len() as f32 - 1.0)
        };
        let key_col = legend.iter().map(|(k, _)| keys_w(k)).fold(0.0, f32::max);
        let cell = key_col + 8.0 + 14.0;
        let gap = 24.0;
        let total = LEGEND_PER_ROW as f32 * cell + (LEGEND_PER_ROW - 1) as f32 * gap;
        for (n, (keys, glyph)) in legend.iter().enumerate() {
            let (row, col) = (n / LEGEND_PER_ROW, n % LEGEND_PER_ROW);
            let x0 = r.center().x - total / 2.0 + col as f32 * (cell + gap);
            let y = r.max.y + 26.0 + row as f32 * 30.0;
            let mut x = x0 + key_col - keys_w(keys);
            for k in keys {
                x += keycap(&p, pos2(x, y), k) + 4.0;
            }
            p.text(
                pos2(x0 + key_col + 8.0, y + 11.0),
                Align2::LEFT_CENTER,
                *glyph,
                mono(14.0),
                GREEN_DIM,
            );
        }
    }

    fn status_pill(&mut self, ui: &mut Ui) {
        let Some((msg, err)) = self.status.clone() else {
            self.status_since = None;
            return;
        };
        // Confirmations fade out after a few seconds; errors stay until dismissed.
        let now = Instant::now();
        if self.status_since.as_ref().is_none_or(|(m, _)| *m != msg) {
            self.status_since = Some((msg.clone(), now));
        }
        let age = self
            .status_since
            .as_ref()
            .map_or(Duration::ZERO, |(_, t)| now - *t);
        let mut opacity = 1.0;
        if !err {
            if age >= STATUS_LIFE {
                self.status = None;
                self.status_since = None;
                return;
            }
            let left = STATUS_LIFE - age;
            opacity = (left.as_secs_f32() / STATUS_FADE.as_secs_f32()).min(1.0);
            ui.ctx().request_repaint_after(if left > STATUS_FADE {
                left - STATUS_FADE
            } else {
                Duration::from_millis(16)
            });
        }
        let central = ui.max_rect();
        let area = egui::Area::new("status".into())
            // Bottom center, above the zoom bar's row: the top belongs to the crop and strip bars.
            .fixed_pos(pos2(central.center().x, central.max.y - 64.0))
            .pivot(Align2::CENTER_BOTTOM)
            .order(egui::Order::Foreground);
        area.show(ui.ctx(), |ui| {
            ui.set_opacity(opacity);
            let (glyph, color) = if err {
                (icon::WARNING, RED)
            } else {
                (icon::CHECK, GREEN)
            };
            // Never wider than the window; long messages wrap.
            ui.set_max_width(480.0f32.min(central.width() - 32.0));
            egui::Frame::new()
                .fill(PANEL)
                .stroke(Stroke::new(1.0, color.gamma_multiply(0.7)))
                .corner_radius(8.0)
                .inner_margin(egui::Margin {
                    left: 12,
                    right: 4,
                    top: 3,
                    bottom: 3,
                })
                .shadow(shadow())
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(glyph).color(color).font(mono(15.0)));
                        // Wraps before the dismiss button; short messages stay compact.
                        let width = ui.available_width() - ICON_SMALL.height - 8.0;
                        ui.scope(|ui| {
                            ui.set_max_width(width.max(40.0));
                            ui.add(
                                egui::Label::new(RichText::new(msg).font(mono(12.0)).color(TEXT))
                                    .wrap(),
                            );
                        });
                        if icon_button_look(ui, icon::X, t!("common.dismiss"), false, ICON_SMALL)
                            .clicked()
                        {
                            self.status = None;
                        }
                    });
                });
        });
    }

    /// The video, fitted to the panel, with the crop overlay on top.
    fn stage(&mut self, ui: &mut Ui) {
        let (aspect, source) = self
            .video
            .as_ref()
            .map_or((16.0 / 9.0, [0, 0]), |v| (v.aspect, v.size));
        // An applied crop (panel closed) frames the preview to just the cropped part; while the
        // Crop panel is open the whole frame shows, with the selection box.
        let editing_crop = self.panel == Some(Panel::Crop);
        let view = if editing_crop { None } else { self.crop };
        let shown = view.map_or(aspect, |c| aspect * c.width() / c.height().max(1e-6));
        let avail = ui.max_rect().shrink(12.0);
        let size = if avail.width() / avail.height() > shown {
            vec2(avail.height() * shown, avail.height())
        } else {
            vec2(avail.width(), avail.width() / shown)
        };
        let vr = Rect::from_center_size(avail.center(), size);
        // Zoom keys and buttons run elsewhere; enforce the crop bound before drawing.
        if editing_crop {
            self.contain_crop();
        }
        let src = self.shown_src();
        self.player.paint(ui, vr, src, source, aspect);

        let resp = ui.interact(
            vr.expand(20.0),
            ui.id().with("stage"),
            Sense::click_and_drag(),
        );
        // Pinch or Ctrl/⌘+wheel, and plain wheel or two-finger scroll: up zooms in.
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        let zoom_delta = ui.input(|i| i.zoom_delta()) * (scroll * 0.0015).exp();
        if resp.hovered() && (zoom_delta - 1.0).abs() > 1e-3 {
            let p = resp.hover_pos().unwrap_or(vr.center());
            self.zoom_video(zoom_delta, Some((p, vr)));
            if editing_crop {
                self.contain_crop();
            }
        }
        if self.strip.edit {
            self.zone_stage(ui, vr, src, resp);
            return;
        }
        let (true, Some(c)) = (editing_crop, self.crop) else {
            if resp.drag_started() {
                self.drag = (self.video_zoom > 1.001).then_some(Drag::Pan);
            }
            if resp.dragged() && self.drag == Some(Drag::Pan) {
                self.pan_video(resp.drag_delta(), vr);
            }
            if resp.drag_stopped() {
                self.drag = None;
            }
            if resp.clicked() {
                self.toggle_play();
            }
            if self.drag == Some(Drag::Pan) {
                ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            } else if self.video_zoom > 1.001 && resp.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::Grab);
            }
            self.draw_zones(ui, vr, src, false);
            return;
        };
        let to_screen = |p: Pos2| zoom::source_to_screen(src, vr, p);
        let sr = Rect::from_min_max(to_screen(c.min), to_screen(c.max));
        let k = self.lock();

        // What sits under the pointer: a corner, an edge (free aspect only), the inside, or nothing.
        let hit = |p: Pos2| -> Option<Drag> {
            for (hx, hy) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
                let corner = pos2(
                    if hx < 0 { sr.min.x } else { sr.max.x },
                    if hy < 0 { sr.min.y } else { sr.max.y },
                );
                if corner.distance(p) < 20.0 {
                    return Some(Drag::Crop(hx, hy));
                }
            }
            if k.is_none() && sr.expand(10.0).contains(p) {
                let near = |a: f32, b: f32| (a - b).abs() < 10.0;
                for (hx, hy, hit) in [
                    (-1, 0, near(p.x, sr.min.x)),
                    (1, 0, near(p.x, sr.max.x)),
                    (0, -1, near(p.y, sr.min.y)),
                    (0, 1, near(p.y, sr.max.y)),
                ] {
                    if hit {
                        return Some(Drag::Crop(hx, hy));
                    }
                }
            }
            sr.contains(p).then_some(Drag::Move)
        };

        // Hit-test where the press began: by the time egui reports a drag the pointer
        // has moved past its threshold, often off a small handle.
        let origin = ui.input(|i| i.pointer.press_origin());
        if resp.drag_started() {
            let shift = ui.input(|i| i.modifiers.shift);
            let hit = (!shift).then(|| origin.and_then(hit)).flatten();
            self.drag = hit.or_else(|| (self.video_zoom > 1.001).then_some(Drag::Pan));
            if let (Some(Drag::Crop(hx, hy)), Some(o)) = (self.drag, origin) {
                let pick = |h: i8, lo: f32, hi: f32, p: f32| match h {
                    -1 => lo,
                    1 => hi,
                    _ => p,
                };
                let handle = pos2(
                    pick(hx, sr.min.x, sr.max.x, o.x),
                    pick(hy, sr.min.y, sr.max.y, o.y),
                );
                self.grab = handle - o;
            }
        }
        if resp.dragged()
            && let Some(p) = resp.interact_pointer_pos()
        {
            if self.drag == Some(Drag::Pan) {
                self.pan_video(resp.drag_delta(), vr);
            } else {
                let np = zoom::screen_to_source(src, vr, p + self.grab);
                self.crop = Some(match self.drag {
                    Some(Drag::Crop(hx, hy)) => crop::drag(c, hx, hy, np, k),
                    Some(Drag::Move) => crop::shift(c, resp.drag_delta() / vr.size() * src.size()),
                    _ => c,
                });
            }
        }
        if resp.drag_stopped() {
            self.drag = None;
        }
        let shift = ui.input(|i| i.modifiers.shift);
        let hovered = self.drag.or_else(|| resp.hover_pos().and_then(hit));
        if self.drag == Some(Drag::Pan) {
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        } else if shift && self.video_zoom > 1.001 {
            ui.ctx().set_cursor_icon(CursorIcon::Grab);
        } else if let Some(d) = hovered {
            ui.ctx().set_cursor_icon(match d {
                Drag::Crop(-1, -1) | Drag::Crop(1, 1) => CursorIcon::ResizeNwSe,
                Drag::Crop(-1, 1) | Drag::Crop(1, -1) => CursorIcon::ResizeNeSw,
                Drag::Crop(_, 0) => CursorIcon::ResizeHorizontal,
                Drag::Crop(..) => CursorIcon::ResizeVertical,
                Drag::Pan => {
                    if self.drag.is_some() {
                        CursorIcon::Grabbing
                    } else {
                        CursorIcon::Grab
                    }
                }
                _ if self.drag.is_some() => CursorIcon::Grabbing,
                _ => CursorIcon::Grab,
            });
        } else if self.video_zoom > 1.001 && resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::Grab);
        }

        let p = ui.painter_at(vr);
        let clip = sr.intersect(vr);
        if clip.is_positive() {
            for r in [
                Rect::from_min_max(vr.min, pos2(vr.max.x, clip.min.y)),
                Rect::from_min_max(pos2(vr.min.x, clip.max.y), vr.max),
                Rect::from_min_max(pos2(vr.min.x, clip.min.y), pos2(clip.min.x, clip.max.y)),
                Rect::from_min_max(pos2(clip.max.x, clip.min.y), pos2(vr.max.x, clip.max.y)),
            ] {
                p.rect_filled(r, 0.0, DIM);
            }
            p.rect_stroke(sr, 0.0, Stroke::new(1.0, GREEN), egui::StrokeKind::Inside);
        } else {
            // The crop is entirely outside the zoomed view; nothing is kept visibly.
            p.rect_filled(vr, 0.0, DIM);
        }
        if clip.is_positive() {
            let thin = Stroke::new(1.0, GREEN.gamma_multiply(0.28));
            for f in [1.0 / 3.0, 2.0 / 3.0] {
                p.vline(sr.lerp_inside(vec2(f, 0.0)).x, sr.y_range(), thin);
                p.hline(sr.x_range(), sr.lerp_inside(vec2(0.0, f)).y, thin);
            }
        }
        let thick = Stroke::new(3.0, GREEN);
        let arm = 18.0f32.min(sr.width() / 2.0).min(sr.height() / 2.0);
        for (hx, hy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0f32)] {
            let c = pos2(
                if hx < 0.0 { sr.min.x } else { sr.max.x },
                if hy < 0.0 { sr.min.y } else { sr.max.y },
            );
            if vr.expand(2.0).contains(c) {
                p.line_segment([c, c - vec2(hx * arm, 0.0)], thick);
                p.line_segment([c, c - vec2(0.0, hy * arm)], thick);
            }
        }
        if k.is_none() && clip.is_positive() {
            let (cx, cy) = (sr.center().x, sr.center().y);
            p.hline(cx - 9.0..=cx + 9.0, sr.min.y, thick);
            p.hline(cx - 9.0..=cx + 9.0, sr.max.y, thick);
            p.vline(sr.min.x, cy - 9.0..=cy + 9.0, thick);
            p.vline(sr.max.x, cy - 9.0..=cy + 9.0, thick);
        }
        self.draw_zones(ui, vr, src, false);
    }

    /// Header slot right of Crop/Strip: the file when no panel is open, else the open panel's bar.
    fn header_center(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        match self.panel {
            None => self.file_info(ui, ctx),
            Some(Panel::Crop) => {
                let mut chosen = None;
                let mut reset = false;
                bar_frame(ui, 32.0, |ui| {
                    for i in 0..crop::PRESETS.len() {
                        if button(ui, "", preset_label(i), self.preset == i, CHIP).clicked() {
                            chosen = Some(i);
                        }
                    }
                    ui.add_space(4.0);
                    vline(ui, 20.0);
                    ui.add_space(4.0);
                    reset = icon_button_look(
                        ui,
                        icon::ARROW_COUNTER_CLOCKWISE,
                        t!("crop.reset"),
                        false,
                        ICON_SMALL,
                    )
                    .clicked();
                });
                if let Some(i) = chosen {
                    self.set_crop_preset(i);
                }
                if reset {
                    self.remove_crop(ctx);
                }
            }
            Some(Panel::Strip) => {
                // Full labels when they fit, else the compact bar. The full width is measured
                // when drawn; an overflowing frame is discarded and redrawn compact.
                let kind = if self.strip.job.is_some() {
                    1
                } else {
                    u8::from(self.strip.scan.is_some()) + 2 * u8::from(!self.strip.zones.is_empty())
                };
                let avail = ui.available_width();
                let compact = self.strip_fit.0 == kind && self.strip_fit.1 > avail;
                let mut off = false;
                let w = bar_frame(ui, 32.0, |ui| {
                    self.strip_controls(ui, ctx, compact);
                    ui.add_space(4.0);
                    vline(ui, 20.0);
                    ui.add_space(4.0);
                    off = icon_button_look(ui, icon::PROHIBIT, t!("strip.off"), false, ICON_SMALL)
                        .clicked();
                });
                if !compact {
                    self.strip_fit = (kind, w);
                    if w > avail + 0.5 {
                        ctx.request_discard("strip bar too wide");
                    }
                }
                if off {
                    self.strip_off();
                }
            }
        }
    }

    /// File name and facts, with the info and close buttons.
    fn file_info(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let Some(v) = &self.video else { return };
        let name = v.path.file_name().map_or_else(
            || t!("file.untitled").to_string(),
            |n| n.to_string_lossy().into(),
        );
        let meta = [
            (icon::FILM_STRIP, codec_label(&v.codec)),
            (icon::FRAME_CORNERS, format!("{}×{}", v.size[0], v.size[1])),
            (icon::TIMER, fmt_time(v.dur)),
        ];
        let path = v.path.display().to_string();
        let block = ui.scope(|ui| file_block(ui, &name, &meta));
        // The file block doubles as the titlebar: drag moves, double-click maximizes.
        let drag = ui.interact(
            block.response.rect,
            ui.id().with("file_drag"),
            Sense::click_and_drag(),
        );
        title_bar(&drag);
        drag.on_hover_text(path);
        ui.add_space(4.0);
        let key = ctx.format_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::I));
        let head = Look {
            height: HEADER_ROW.min(ICON.height),
            ..ICON
        };
        let info = icon_button_look(
            ui,
            icon::INFO,
            &format!("{} ({key})", t!("tip.info")),
            self.pop == Some(Pop::Info),
            head,
        );
        self.anchors.info = info.rect;
        if info.clicked() {
            self.toggle_pop(Pop::Info);
        }
        let close = ctx.format_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::W));
        if icon_button_look(
            ui,
            icon::X_CIRCLE,
            &t!("tip.close_video", key = close),
            false,
            head,
        )
        .clicked()
        {
            self.request_close(ctx);
        }
    }

    /// Video zoom: out, level (click to fit), in, and a fit button unless `compact`.
    fn video_zoom_controls(&mut self, ui: &mut Ui, compact: bool) {
        bar_frame(ui, DECK_H, |ui| {
            if icon_button_look(ui, icon::MINUS, t!("tip.zoom_out"), false, ICON_SMALL).clicked() {
                self.zoom_video(0.8, None);
            }
            let zoomed = self.video_zoom > 1.001;
            let (r, resp) = ui.allocate_exact_size(vec2(44.0, 26.0), Sense::click());
            ui.painter().text(
                r.center(),
                Align2::CENTER_CENTER,
                zoom_label(self.video_zoom),
                mono(11.0),
                if zoomed || resp.hovered() {
                    GREEN
                } else {
                    MUTED
                },
            );
            if resp
                .on_hover_text(format!("{} · {}", t!("tip.zoom"), t!("tip.zoom_fit")))
                .clicked()
            {
                self.reset_video_zoom();
            }
            if icon_button_look(ui, icon::PLUS, t!("tip.zoom_in"), false, ICON_SMALL).clicked() {
                self.zoom_video(1.25, None);
            }
            if !compact
                && icon_button_look(ui, icon::CORNERS_IN, t!("tip.zoom_fit"), false, ICON_SMALL)
                    .clicked()
            {
                self.reset_video_zoom();
            }
        });
    }

    /// Strip bar: ignored areas, padding, then the scan state and its action.
    /// `compact`: icon-only buttons, short bars, no total, for narrow windows.
    fn strip_controls(&mut self, ui: &mut Ui, ctx: &egui::Context, compact: bool) {
        let label = |l: &'static str| if compact { "" } else { l };
        let bar_w = if compact { 40.0 } else { 90.0 };
        if button(
            ui,
            icon::EYE_SLASH,
            label(t!("strip.add_zone")),
            false,
            GHOST,
        )
        .on_hover_text(t!("tip.add_zone"))
        .clicked()
        {
            self.add_zone();
        }
        if !self.strip.zones.is_empty() {
            let tip = format!(
                "{} · {}",
                t!("strip.edit_zones"),
                tn!("strip.zones", self.strip.zones.len())
            );
            if icon_button_look(ui, icon::PENCIL_SIMPLE, &tip, self.strip.edit, ICON_SMALL)
                .clicked()
            {
                self.toggle_zone_edit();
            }
            if icon_button_look(ui, icon::TRASH, t!("strip.clear"), false, ICON_SMALL).clicked() {
                self.clear_zones();
            }
        }
        ui.add_space(6.0);
        vline(ui, 20.0);
        ui.add_space(6.0);
        ui.label(
            RichText::new(icon::ARROWS_HORIZONTAL)
                .font(mono(15.0))
                .color(MUTED),
        )
        .on_hover_text(t!("tip.pad"));
        ui.add_space(4.0);
        ui.scope(|ui| {
            let w = &mut ui.visuals_mut().widgets;
            w.inactive.bg_fill = CARD;
            w.inactive.weak_bg_fill = CARD;
            w.inactive.bg_stroke = Stroke::new(1.0, LINE);
            w.inactive.corner_radius = 6.into();
            w.hovered.corner_radius = 6.into();
            w.active.corner_radius = 6.into();
            // Text must fit inside the 24 pt box, or the frame's top edge gets clipped.
            ui.style_mut().override_font_id = Some(mono(11.0));
            ui.spacing_mut().button_padding = vec2(8.0, 2.0);
            ui.spacing_mut().interact_size.y = 24.0;
            ui.add_sized(
                vec2(72.0, 24.0),
                egui::DragValue::new(&mut self.strip.pad)
                    .range(0.0..=30.0)
                    .speed(0.01)
                    .fixed_decimals(2)
                    .suffix(format!(" {}", t!("unit.s"))),
            )
            .on_hover_text(t!("tip.pad"));
        });
        ui.add_space(6.0);
        vline(ui, 20.0);
        ui.add_space(6.0);

        if self.strip.job.is_some() {
            let (r, _) = ui.allocate_exact_size(vec2(bar_w, 26.0), Sense::hover());
            let bar = Rect::from_center_size(r.center(), vec2(bar_w, 5.0));
            let p = ui.painter();
            p.rect_filled(bar, 3.0, CARD_HI);
            // Same pace as the timeline's scan line.
            let dur = self.video.as_ref().map_or(1.0, |v| v.dur).max(1e-6);
            let frac = (self.strip.front.shown / dur).clamp(0.0, 1.0) as f32;
            progress_fill(p, bar, frac, 3.0, GREEN);
            ui.add_space(6.0);
            let pct = format!("{:.0}", frac * 100.0);
            let text = if compact {
                format!("{pct}%")
            } else {
                t!("strip.scanning", pct = pct).to_string()
            };
            ui.label(RichText::new(text).font(mono(11.0)).color(GREEN));
            if icon_button_look(ui, icon::X, t!("tip.cancel_scan"), false, ICON_SMALL).clicked() {
                self.cancel_scan();
            }
            return;
        }
        let rescan = self.strip.scan.is_some();
        if let Some(scan) = &self.strip.scan {
            let keep = strip::keep(&scan.active, self.strip.pad, self.trim.0, self.trim.1);
            let kept: f64 = keep.iter().map(|(a, b)| b - a).sum();
            let total = (self.trim.1 - self.trim.0).max(0.0);
            ui.label(RichText::new(icon::SCISSORS).font(mono(15.0)).color(RED));
            ui.add_space(4.0);
            if kept <= 0.0 {
                ui.label(
                    RichText::new(t!("strip.nothing"))
                        .font(mono(11.0))
                        .color(RED),
                );
            } else {
                let (r, resp) = ui.allocate_exact_size(vec2(bar_w, 26.0), Sense::hover());
                let bar = Rect::from_center_size(r.center(), vec2(bar_w, 5.0));
                let p = ui.painter();
                p.rect_filled(bar, 3.0, RED.gamma_multiply(0.35));
                let frac = (kept / total.max(1e-9)) as f32;
                progress_fill(p, bar, frac, 3.0, GREEN);
                let pct = format!("{:.0}", kept / total.max(1e-9) * 100.0);
                resp.on_hover_text(t!("strip.keep", pct = pct, time = fmt_time(kept)));
                ui.add_space(4.0);
                ui.label(RichText::new(fmt_time(kept)).font(mono(11.0)).color(TEXT));
                if !compact {
                    ui.label(
                        RichText::new(format!("/ {}", fmt_time(total)))
                            .font(mono(11.0))
                            .color(MUTED),
                    );
                }
            }
            ui.add_space(6.0);
        }
        let (glyph, label) = if rescan {
            (icon::ARROWS_CLOCKWISE, t!("strip.rescan"))
        } else {
            (icon::MAGNIFYING_GLASS, t!("strip.scan"))
        };
        let tip = if compact { label } else { t!("tip.scan") };
        if accent(ui, glyph, if compact { "" } else { label })
            .on_hover_text(tip)
            .clicked()
        {
            self.start_scan(ctx);
        }
    }

    /// Red rectangles whose changes the scanner ignores. With `editing`, draw handles and hints.
    fn draw_zones(&self, ui: &Ui, vr: Rect, src: Rect, editing: bool) {
        if self.panel != Some(Panel::Strip) {
            return;
        }
        let p = ui.painter_at(vr);
        for (i, z) in self.strip.zones.iter().enumerate() {
            let r = Rect::from_min_max(
                zoom::source_to_screen(src, vr, z.min),
                zoom::source_to_screen(src, vr, z.max),
            );
            let selected = editing && self.strip.selected == Some(i);
            let fill = if selected { 0.22 } else { 0.12 };
            p.rect_filled(r, 0.0, RED.gamma_multiply(fill));
            let stroke = if selected { 2.0 } else { 1.0 };
            p.rect_stroke(r, 0.0, Stroke::new(stroke, RED), egui::StrokeKind::Inside);
            if selected {
                for c in [
                    r.left_top(),
                    r.right_top(),
                    r.left_bottom(),
                    r.right_bottom(),
                ] {
                    p.rect_filled(Rect::from_center_size(c, vec2(8.0, 8.0)), 0.0, RED);
                }
            }
        }
    }

    /// Drag/resize ignore zones over the video.
    /// Drag/resize ignore zones over the video; drag empty space to pan when zoomed.
    fn zone_stage(&mut self, ui: &mut Ui, vr: Rect, src: Rect, resp: Response) {
        let origin = ui.input(|i| i.pointer.press_origin());
        if resp.drag_started() {
            let shift = ui.input(|i| i.modifiers.shift);
            let hit = (!shift)
                .then(|| origin.and_then(|p| zone_hit(&self.strip.zones, vr, src, p)))
                .flatten();
            self.strip.drag = hit;
            self.drag = if hit.is_none() && self.video_zoom > 1.001 {
                Some(Drag::Pan)
            } else {
                None
            };
            self.strip.zone_start = None;
            self.strip.grab = Vec2::ZERO;
            if hit.is_none() {
                self.strip.selected = None;
            }
            if let Some((i, drag)) = hit {
                self.strip.selected = Some(i);
                self.strip.zone_start = Some(self.strip.zones[i]);
                if let (Drag::Crop(hx, hy), Some(o)) = (drag, origin) {
                    let z = self.strip.zones[i];
                    let r = Rect::from_min_max(
                        zoom::source_to_screen(src, vr, z.min),
                        zoom::source_to_screen(src, vr, z.max),
                    );
                    let pick = |h: i8, lo: f32, hi: f32, p: f32| match h {
                        -1 => lo,
                        1 => hi,
                        _ => p,
                    };
                    let corner = pos2(
                        pick(hx, r.min.x, r.max.x, o.x),
                        pick(hy, r.min.y, r.max.y, o.y),
                    );
                    self.strip.grab = corner - o;
                }
            }
        }
        if resp.dragged()
            && let Some(p) = resp.interact_pointer_pos()
        {
            if self.drag == Some(Drag::Pan) {
                self.pan_video(resp.drag_delta(), vr);
            } else if let Some((i, drag)) = self.strip.drag {
                let np = zoom::screen_to_source(src, vr, p + self.strip.grab);
                if let Some(z) = self.strip.zones.get_mut(i) {
                    *z = match drag {
                        Drag::Crop(hx, hy) => crop::drag(*z, hx, hy, np, None),
                        Drag::Move => crop::shift(*z, resp.drag_delta() / vr.size() * src.size()),
                        _ => *z,
                    };
                }
            }
        }
        if resp.drag_stopped() {
            if let (Some((i, _)), Some(start)) = (self.strip.drag, self.strip.zone_start)
                && self.strip.zones.get(i) != Some(&start)
            {
                self.zones_changed();
            }
            self.strip.drag = None;
            self.strip.zone_start = None;
            self.drag = None;
        }
        if resp.clicked()
            && let Some(p) = resp.interact_pointer_pos()
        {
            self.strip.selected = zone_hit(&self.strip.zones, vr, src, p).map(|(i, _)| i);
        }
        let hovered = self.strip.drag.or_else(|| {
            resp.hover_pos()
                .and_then(|p| zone_hit(&self.strip.zones, vr, src, p))
        });
        let shift = ui.input(|i| i.modifiers.shift);
        if self.drag == Some(Drag::Pan) {
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        } else if shift && self.video_zoom > 1.001 {
            ui.ctx().set_cursor_icon(CursorIcon::Grab);
        } else {
            match hovered {
                Some((_, Drag::Crop(-1, -1) | Drag::Crop(1, 1))) => {
                    ui.ctx().set_cursor_icon(CursorIcon::ResizeNwSe)
                }
                Some((_, Drag::Crop(-1, 1) | Drag::Crop(1, -1))) => {
                    ui.ctx().set_cursor_icon(CursorIcon::ResizeNeSw)
                }
                Some((_, Drag::Move)) if self.strip.drag.is_some() => {
                    ui.ctx().set_cursor_icon(CursorIcon::Grabbing)
                }
                Some((_, Drag::Move)) => ui.ctx().set_cursor_icon(CursorIcon::Grab),
                _ if self.video_zoom > 1.001 && resp.hovered() => {
                    ui.ctx().set_cursor_icon(CursorIcon::Grab)
                }
                _ => {}
            }
        }
        self.draw_zones(ui, vr, src, true);
    }

    /// Header: window controls, mark and file, info/close, modes, and the Export action.
    fn header(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        // The whole header is the titlebar: drag moves, double-click maximizes. Registered
        // before the controls, so they sit on top and keep their own clicks.
        let bar = ui.max_rect().expand2(vec2(16.0, 6.0));
        title_bar(&ui.interact(bar, ui.id().with("titlebar"), Sense::click_and_drag()));
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), HEADER_ROW),
            Layout::left_to_right(Align::Center),
            |ui| {
                // macOS full-size content view: leave room for the native traffic lights.
                if cfg!(target_os = "macos") {
                    ui.add_space(76.0);
                }
                if !self.buttons.left.is_empty() {
                    window_buttons(ui, self.buttons.left.iter());
                    ui.add_space(10.0);
                }
                let right = self.buttons.right.clone();
                let title = ui.scope(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    logo(ui);
                    if self.video.is_none() {
                        ui.label(RichText::new("G L I N T").font(bold(15.0)).color(GREEN));
                    }
                });
                // The mark doubles as the titlebar: drag moves, double-click maximizes.
                let drag = ui.interact(
                    title.response.rect,
                    ui.id().with("title_drag"),
                    Sense::click_and_drag(),
                );
                title_bar(&drag);
                if let Some(v) = &self.video {
                    drag.on_hover_text(v.path.display().to_string());
                    ui.add_space(8.0);
                    self.mode_group(ui);
                    ui.add_space(10.0);
                    vline(ui, 24.0);
                    ui.add_space(10.0);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if !right.is_empty() {
                            window_buttons(ui, right.iter().rev());
                            ui.add_space(6.0);
                            vline(ui, 24.0);
                            ui.add_space(6.0);
                        }
                        self.export_action(ui, &ctx);
                        ui.add_space(12.0);
                        // What's left between Crop/Strip and Export; never drawn past it.
                        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                            ui.set_clip_rect(ui.max_rect().intersect(ui.clip_rect()));
                            self.header_center(ui, &ctx);
                        });
                    });
                } else {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        window_buttons(ui, right.iter().rev());
                    });
                }
            },
        );
    }

    /// Export button, or the running export's progress with a cancel button.
    fn export_action(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        if self.job.is_some() {
            if icon_button(ui, icon::X, t!("export.cancel"), false).clicked()
                && let Some(job) = self.job.take()
            {
                job.cancel();
            }
            let (r, _) = ui.allocate_exact_size(vec2(150.0, 34.0), Sense::hover());
            let p = ui.painter();
            p.rect(
                r,
                8.0,
                CARD,
                Stroke::new(1.0, LINE),
                egui::StrokeKind::Inside,
            );
            let frac = self.export_glide.tick().min(1.0) as f32;
            progress_fill(p, r.shrink(3.0), frac, 6.0, GREEN.gamma_multiply(0.22));
            ui.ctx().request_repaint();
            p.text(
                r.center(),
                Align2::CENTER_CENTER,
                format!("{}  {:.0}%", icon::EXPORT, frac * 100.0),
                mono(12.0),
                GREEN,
            );
            return;
        }
        let save = ctx.format_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::S));
        let label = t!("export.button");
        let w = ui
            .painter()
            .layout_no_wrap(label.into(), bold(12.5), INK)
            .size()
            .x
            + 60.0;
        let resp = primary(
            ui,
            icon::EXPORT,
            label,
            vec2(w, HEADER_ROW.min(34.0)),
            self.pop == Some(Pop::Export),
        )
        .on_hover_text(t!("tip.export", key = save));
        self.anchors.export = resp.rect;
        if resp.clicked() {
            self.toggle_pop(Pop::Export);
        }
    }

    fn mode_group(&mut self, ui: &mut Ui) {
        let look = Look {
            height: (HEADER_ROW - 4.0).min(MODE.height),
            ..MODE
        };
        egui::Frame::new()
            .fill(CARD)
            .stroke(Stroke::new(1.0, LINE))
            .corner_radius(8.0)
            .inner_margin(egui::Margin::same(2))
            .show(ui, |ui| {
                // Without this the frame grows to the header row's full height.
                ui.set_height(look.height);
                ui.spacing_mut().item_spacing.x = 2.0;
                let ctx = ui.ctx().clone();
                let crop_open = self.panel == Some(Panel::Crop);
                let resp = button(ui, icon::CROP, t!("mode.crop"), crop_open, look);
                applied_dot(ui, resp.rect, self.crop.is_some() && !crop_open);
                if resp.clicked() {
                    self.toggle_crop(&ctx);
                }
                let strip_open = self.panel == Some(Panel::Strip);
                let resp = button(ui, icon::MAGIC_WAND, t!("mode.strip"), strip_open, look);
                applied_dot(ui, resp.rect, self.strip.enabled && !strip_open);
                if resp.clicked() {
                    self.toggle_strip(&ctx);
                }
            });
    }

    /// "Open another video": replace the current one, or open it in a new window.
    /// The center modal: replace-or-new-window for a second file, or unexported edits on close.
    fn open_modal(&mut self, ctx: &egui::Context) {
        let Some(confirm) = self.confirm.clone() else {
            return;
        };
        let screen = ctx.content_rect();
        egui::Area::new("modal_scrim".into())
            .fixed_pos(screen.min)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let (r, _) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter()
                    .rect_filled(r, 0.0, Color32::from_black_alpha(150));
            });
        let (glyph, title, hint, second, first) = match &confirm {
            Confirm::Open(path) => {
                let name = path.file_name().map_or_else(
                    || path.display().to_string(),
                    |n| n.to_string_lossy().into(),
                );
                (
                    icon::FILM_SLATE,
                    t!("open.another", name = name).to_string(),
                    t!("open.another_hint"),
                    (icon::SWAP, t!("open.replace")),
                    (icon::APP_WINDOW, t!("open.new_window")),
                )
            }
            Confirm::Quit | Confirm::CloseVideo => (
                icon::WARNING,
                t!("unsaved.title").to_string(),
                t!("unsaved.hint"),
                if confirm == Confirm::Quit {
                    (icon::SIGN_OUT, t!("unsaved.quit"))
                } else {
                    (icon::X_CIRCLE, t!("unsaved.close"))
                },
                (icon::EXPORT, t!("export.button")),
            ),
        };
        let mut choice = None;
        let shown = egui::Area::new("open_modal".into())
            .fixed_pos(screen.center())
            .pivot(Align2::CENTER_CENTER)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0, LINE))
                    .corner_radius(12.0)
                    .inner_margin(20)
                    .shadow(shadow())
                    .show(ui, |ui| {
                        ui.set_width(380.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 10.0;
                            ui.label(RichText::new(glyph).font(mono(20.0)).color(GREEN));
                            ui.add(
                                egui::Label::new(RichText::new(title).font(bold(13.0)).color(TEXT))
                                    .truncate(),
                            );
                        });
                        ui.add_space(6.0);
                        ui.label(RichText::new(hint).font(mono(11.5)).color(MUTED));
                        ui.add_space(14.0);
                        // One right-to-left row so all three buttons share a center line.
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            let w = ui
                                .painter()
                                .layout_no_wrap(first.1.into(), bold(12.5), INK)
                                .size()
                                .x
                                + 56.0;
                            if primary(ui, first.0, first.1, vec2(w, 34.0), false).clicked() {
                                choice = Some(2);
                            }
                            if accent(ui, second.0, second.1).clicked() {
                                choice = Some(1);
                            }
                            let cancel = ui
                                .painter()
                                .layout_no_wrap(t!("common.cancel").into(), mono(GHOST.text), TEXT)
                                .size()
                                .x
                                + GHOST.pad * 2.0;
                            ui.add_space((ui.available_width() - cancel).max(0.0));
                            if button(ui, "", t!("common.cancel"), false, GHOST).clicked() {
                                choice = Some(0);
                            }
                        });
                    });
            });
        ctx.move_to_top(shown.response.layer_id);
        let Some(choice) = choice else { return };
        self.confirm = None;
        match (confirm, choice) {
            (Confirm::Open(path), 1) => self.open(path),
            (Confirm::Open(path), 2) => self.open_in_new_window(path),
            (Confirm::Quit, 1) => {
                self.quitting = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            (Confirm::CloseVideo, 1) => self.close(ctx),
            (Confirm::Quit | Confirm::CloseVideo, 2) => self.pop = Some(Pop::Export),
            _ => {}
        }
    }

    fn toggle_pop(&mut self, pop: Pop) {
        self.pop = (self.pop != Some(pop)).then_some(pop);
    }

    /// The open popover over a dimmed window, with its arrow on the button that opened it.
    fn popover(&mut self, ctx: &egui::Context) {
        let Some(pop) = self.pop else { return };
        if self.video.is_none() {
            self.pop = None;
            return;
        }
        let screen = ctx.content_rect();
        let scrim = egui::Area::new("scrim".into())
            .fixed_pos(screen.min)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let (r, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter()
                    .rect_filled(r, 0.0, Color32::from_black_alpha(115));
                resp
            })
            .inner;
        if scrim.clicked() {
            self.pop = None;
            return;
        }
        let (anchor, width) = match pop {
            Pop::Export => (self.anchors.export, 360.0),
            Pop::Info => (self.anchors.info, 420.0),
        };
        let left = match pop {
            Pop::Export => anchor.max.x - width,
            Pop::Info => anchor.center().x - 120.0,
        }
        .clamp(
            screen.min.x + 12.0,
            (screen.max.x - width - 12.0).max(screen.min.x + 12.0),
        );
        let top = self.anchors.header_bottom + 6.0;
        let shown = egui::Area::new(egui::Id::new("popover"))
            .fixed_pos(pos2(left, top))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0, LINE))
                    .corner_radius(12.0)
                    .inner_margin(18)
                    .shadow(shadow())
                    .show(ui, |ui| {
                        ui.set_width(width - 36.0);
                        // Room left above the window's bottom edge, inside the 18 pt padding.
                        let max_h = (screen.max.y - top - 12.0 - 36.0).max(120.0);
                        match pop {
                            Pop::Export => self.export_pop(ui, ctx, max_h),
                            // The info popover scrolls below its title row (~50 pt).
                            Pop::Info => self.info_pop(ui, ctx, max_h - 50.0),
                        }
                    });
            });
        ctx.move_to_top(shown.response.layer_id);
        let r = shown.response.rect;
        let cx = anchor.center().x.clamp(r.min.x + 18.0, r.max.x - 18.0);
        let p = ctx.layer_painter(shown.response.layer_id);
        let (a, tip, b) = (
            // The base sits below the frame's 1 px top border, so no line shows across it.
            pos2(cx - 7.0, r.min.y + 1.5),
            pos2(cx, r.min.y - 6.5),
            pos2(cx + 7.0, r.min.y + 1.5),
        );
        p.add(egui::Shape::convex_polygon(
            vec![a, tip, b],
            PANEL,
            Stroke::NONE,
        ));
        p.line_segment([a, tip], Stroke::new(1.0, LINE));
        p.line_segment([tip, b], Stroke::new(1.0, LINE));
    }

    fn pop_head(&mut self, ui: &mut Ui, glyph: &str, title: &str, extra: impl FnOnce(&mut Ui)) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 9.0;
            ui.label(RichText::new(glyph).font(mono(18.0)).color(GREEN));
            ui.add(egui::Label::new(RichText::new(title).font(bold(14.0)).color(TEXT)).truncate());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                if icon_button(ui, icon::X, t!("common.close"), false).clicked() {
                    self.pop = None;
                }
                extra(ui);
            });
        });
    }

    fn export_pop(&mut self, ui: &mut Ui, ctx: &egui::Context, max_h: f32) {
        ui.spacing_mut().item_spacing.y = 8.0;
        self.pop_head(ui, icon::EXPORT, t!("export.title"), |_| {});
        // Title and Save stay put; the settings between them scroll on short windows.
        egui::ScrollArea::vertical()
            .max_height((max_h - 50.0 - 52.0).max(60.0))
            .show(ui, |ui| {
                ui.add_space(6.0);
                field_label(ui, icon::PACKAGE, t!("export.format"));
                let formats: Vec<String> =
                    ffmpeg::FORMATS.iter().map(|f| f.to_uppercase()).collect();
                if let Some(i) = segmented(ui, "format", &formats, self.format) {
                    self.format = i;
                }
                ui.add_space(8.0);
                field_label(ui, icon::SLIDERS_HORIZONTAL, t!("export.quality"));
                let qualities: Vec<String> = ffmpeg::QUALITIES
                    .iter()
                    .map(|q| i18n::tr(q).to_string())
                    .collect();
                if let Some(i) = segmented(ui, "quality", &qualities, self.quality) {
                    self.quality = i;
                }
                ui.add_space(8.0);
                if toggle_row(
                    ui,
                    icon::CROSSHAIR,
                    t!("export.precise"),
                    t!("export.precise_hint"),
                    self.precise,
                ) {
                    self.precise = !self.precise;
                }
                if toggle_row(
                    ui,
                    icon::SPEAKER_SLASH,
                    t!("export.mute"),
                    t!("export.mute_hint"),
                    self.mute,
                ) {
                    self.mute = !self.mute;
                    let _ = self.player.mpv.set_property("mute", self.mute);
                }
                ui.add_space(4.0);
                let (line, _) =
                    ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
                ui.painter()
                    .hline(line.x_range(), line.center().y, Stroke::new(1.0, LINE));
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(5.0, 6.0);
                    let item = |ui: &mut Ui, glyph: &str, text: String, color: Color32| {
                        ui.label(RichText::new(glyph).font(mono(13.0)).color(GREEN));
                        ui.label(RichText::new(text).font(mono(12.0)).color(color));
                        ui.add_space(10.0);
                    };
                    let retime = (self.speed - 1.0).abs() > 1e-4;
                    match self.output_length() {
                        Some(len) if retime => item(
                            ui,
                            icon::TIMER,
                            format!("{} · {}", fmt_time(len), speed_label(self.speed)),
                            TEXT,
                        ),
                        Some(len) => item(ui, icon::TIMER, fmt_time(len), TEXT),
                        None => item(ui, icon::TIMER, t!("export.need_scan").into(), MUTED),
                    }
                    if let (Some(c), Some(v)) = (self.crop, &self.video) {
                        let [w, h, _, _] = crop::to_pixels(c, v.size[0], v.size[1]);
                        item(ui, icon::CROP, format!("{w}×{h}"), TEXT);
                    }
                    let method = if self.stream_copy() {
                        t!("export.copy")
                    } else {
                        t!("export.reencode")
                    };
                    item(ui, icon::CPU, method.into(), TEXT);
                });
                if self.strip.enabled || (self.speed - 1.0).abs() > 1e-4 {
                    ui.label(
                        RichText::new(t!("export.retime_note"))
                            .font(mono(11.0))
                            .color(MUTED),
                    );
                }
            });
        ui.add_space(6.0);
        let w = ui.available_width();
        if primary(
            ui,
            icon::DOWNLOAD_SIMPLE,
            t!("export.save"),
            vec2(w, 40.0),
            false,
        )
        .clicked()
        {
            self.pop = None;
            self.export(ctx);
        }
    }

    fn info_pop(&mut self, ui: &mut Ui, ctx: &egui::Context, max_h: f32) {
        let Some(v) = &self.video else { return };
        let name = v.path.file_name().map_or_else(
            || t!("file.untitled").to_string(),
            |n| n.to_string_lossy().into(),
        );
        if self.meta.is_none() {
            self.meta = Some(meta::read(&self.player, &v.path, v.dur, v.size));
        }
        let text = meta::to_text(&name, self.meta.as_deref().unwrap_or_default());
        self.pop_head(ui, icon::INFO, &name, |ui| {
            if icon_button(ui, icon::COPY, t!("meta.copy"), false).clicked() {
                ctx.copy_text(text);
            }
        });
        let sections = self.meta.as_deref().unwrap_or_default();
        // Grids read the minimum row height when created; the default 28 pt control height
        // would space the rows far apart.
        ui.spacing_mut().interact_size.y = 16.0;
        egui::ScrollArea::vertical()
            .max_height(max_h.max(120.0))
            .show(ui, |ui| {
                for (n, sec) in sections.iter().enumerate() {
                    ui.add_space(8.0);
                    let (line, _) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
                    ui.painter()
                        .hline(line.x_range(), line.center().y, Stroke::new(1.0, LINE));
                    ui.add_space(6.0);
                    field_label(ui, sec.icon, &sec.title);
                    egui::Grid::new(("meta", n))
                        .num_columns(2)
                        .min_col_width(110.0)
                        .spacing([10.0, 5.0])
                        .show(ui, |ui| {
                            for (k, val) in &sec.rows {
                                ui.label(RichText::new(k).font(mono(12.0)).color(MUTED));
                                ui.label(RichText::new(val).font(mono(12.0)).color(TEXT));
                                ui.end_row();
                            }
                        });
                }
            });
    }

    /// Transport row over the trim bar.
    fn deck(&mut self, ui: &mut Ui) {
        let dur = self.video.as_ref().map_or(0.0, |v| v.dur);
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), 40.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                // Everything when it fits; else no total time and no zoom-fit button. The full
                // width is measured when drawn; an overflowing frame is discarded and redrawn.
                let row = ui.available_width();
                let compact = self.deck_need > row;
                if play_button(ui, self.paused()).clicked() {
                    self.toggle_play();
                }
                ui.add_space(4.0);
                if icon_button_look(
                    ui,
                    icon::CLOCK_COUNTER_CLOCKWISE,
                    t!("tip.back"),
                    false,
                    DECK_ICON,
                )
                .clicked()
                {
                    self.skip(-5.0);
                }
                if icon_button_look(
                    ui,
                    icon::CLOCK_CLOCKWISE,
                    t!("tip.forward"),
                    false,
                    DECK_ICON,
                )
                .clicked()
                {
                    self.skip(5.0);
                }
                ui.add_space(8.0);
                ui.label(
                    RichText::new(fmt_time(self.pos()))
                        .font(bold(19.0))
                        .color(Color32::WHITE),
                );
                if !compact {
                    ui.label(
                        RichText::new(format!("/ {}", fmt_time(dur)))
                            .font(mono(11.0))
                            .color(MUTED),
                    );
                }
                ui.add_space(10.0);
                self.speed_control(ui);
                ui.add_space(6.0);
                self.video_zoom_controls(ui, compact);
                let left = ui.min_rect().width();
                let right = ui
                    .with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        self.timeline_zoom_controls(ui, dur);
                        ui.add_space(10.0);
                        self.selection_control(ui, dur);
                        ui.min_rect().width()
                    })
                    .inner;
                if !compact {
                    self.deck_need = left + right + 12.0;
                    if self.deck_need > row + 0.5 {
                        ui.ctx().request_discard("transport row too wide");
                    }
                }
            },
        );
        ui.add_space(10.0);
        self.trim_bar(ui);
    }

    fn timeline_zoom_controls(&mut self, ui: &mut Ui, dur: f64) {
        let z = self.timeline.zoom;
        let (v0, v1) = self.timeline.view(dur);
        let view = format!("{} · {} – {}", zoom_label(z), fmt_time(v0), fmt_time(v1));
        // Right to left: the rightmost control comes first.
        if icon_button_look(
            ui,
            icon::ARROWS_OUT_LINE_HORIZONTAL,
            &t!("tip.timeline_fit", view = view),
            z <= 1.001,
            DECK_ICON,
        )
        .clicked()
        {
            self.fit_timeline(dur);
        }
        if icon_button_look(
            ui,
            icon::ARROWS_IN_LINE_HORIZONTAL,
            &t!("tip.timeline_selection", view = view),
            false,
            DECK_ICON,
        )
        .clicked()
        {
            self.fit_timeline_selection(dur);
        }
        if icon_button_look(
            ui,
            icon::MAGNIFYING_GLASS_PLUS,
            &t!("tip.timeline_in", view = view),
            false,
            DECK_ICON,
        )
        .clicked()
        {
            self.zoom_timeline(1.5, dur);
        }
        if icon_button_look(
            ui,
            icon::MAGNIFYING_GLASS_MINUS,
            &t!("tip.timeline_out", view = view),
            false,
            DECK_ICON,
        )
        .clicked()
        {
            self.zoom_timeline(1.0 / 1.5, dur);
        }
    }

    fn speed_control(&mut self, ui: &mut Ui) {
        let speed = self.speed;
        let p = ui.painter();
        let parts = [
            (icon::GAUGE.to_string(), mono(14.0), GREEN),
            (
                speed_label(speed),
                mono(12.0),
                if (speed - 1.0).abs() > 1e-4 {
                    GREEN
                } else {
                    TEXT
                },
            ),
        ];
        let galleys: Vec<_> = parts
            .into_iter()
            .map(|(s, f, c)| (p.layout_no_wrap(s, f, c), c))
            .collect();
        let w = galleys.iter().map(|(g, _)| g.size().x).sum::<f32>() + 6.0 + 24.0;
        // The menu is as wide as the button, unless its longest entry needs more.
        let widest = SPEEDS
            .iter()
            .map(|&s| p.layout_no_wrap(speed_label(s), mono(12.0), TEXT).size().x)
            .fold(0.0, f32::max);
        let menu_w = w.max(widest + 2.0 * 8.0 + 6.0);
        let (r, resp) = ui.allocate_exact_size(vec2(w, DECK_H), Sense::click());
        let edge = if resp.hovered() {
            GREEN.gamma_multiply(0.5)
        } else {
            LINE
        };
        let p = ui.painter();
        p.rect(
            r,
            7.0,
            CARD,
            Stroke::new(1.0, edge),
            egui::StrokeKind::Inside,
        );
        let mut x = r.min.x + 12.0;
        for (g, c) in galleys {
            let gw = g.size().x;
            p.galley(pos2(x, r.center().y - g.size().y / 2.0), g, c);
            x += gw + 6.0;
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        let resp = resp.on_hover_text(t!("tip.speed"));
        let frame = egui::Frame::new()
            .fill(PANEL)
            .stroke(Stroke::new(1.0, LINE))
            .corner_radius(7)
            .inner_margin(3);
        egui::Popup::menu(&resp)
            .width(menu_w)
            .gap(4.0)
            .frame(frame)
            .show(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                for &s in SPEEDS {
                    let on = (speed - s).abs() < 1e-6;
                    let (row, item) =
                        ui.allocate_exact_size(vec2(menu_w - 6.0, 22.0), Sense::click());
                    let p = ui.painter();
                    if on || item.hovered() {
                        let fill = if on {
                            GREEN.gamma_multiply(0.11)
                        } else {
                            CARD_HI
                        };
                        p.rect_filled(row, 5.0, fill);
                    }
                    p.text(
                        pos2(row.min.x + 8.0, row.center().y),
                        Align2::LEFT_CENTER,
                        speed_label(s),
                        mono(12.0),
                        if on { GREEN } else { TEXT },
                    );
                    if item.clicked() {
                        self.set_speed(s);
                        ui.close();
                    }
                }
            });
    }

    /// IN / OUT / LEN fields in one box: icon, then a draggable, typeable time.
    fn selection_control(&mut self, ui: &mut Ui, dur: f64) {
        let (mut a, mut b) = self.trim;
        let (mut new_in, mut new_out, mut new_len) = (None, None, None);
        // The deck's right side lays out right to left; give the box its own left-to-right
        // row, sized from the last frame (egui's usual two-pass trick).
        let width = self.anchors.selection_width;
        let shown = ui.allocate_ui_with_layout(
            vec2(width, DECK_H),
            Layout::left_to_right(Align::Center),
            |ui| {
                egui::Frame::new()
                    .fill(CARD)
                    .stroke(Stroke::new(1.0, LINE))
                    .corner_radius(7.0)
                    .inner_margin(egui::Margin::symmetric(4, 0))
                    .show(ui, |ui| {
                        ui.set_height(DECK_H);
                        // A fixed-height row: `horizontal` centers items on the row height so
                        // far, so items before the separators sat higher than the rest.
                        let row = vec2(ui.available_width(), DECK_H);
                        ui.allocate_ui_with_layout(
                            row,
                            Layout::left_to_right(Align::Center),
                            |ui| {
                                ui.spacing_mut().interact_size.y = 22.0;
                                let v = ui.visuals_mut();
                                v.widgets.inactive.bg_fill = Color32::TRANSPARENT;
                                v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                                v.widgets.inactive.bg_stroke = Stroke::NONE;
                                ui.spacing_mut().item_spacing.x = 4.0;
                                let field_icon = |ui: &mut Ui, glyph: &str, tip: &str| {
                                    ui.add_space(6.0);
                                    ui.label(
                                        RichText::new(glyph).font(mono(14.0)).color(GREEN_DIM),
                                    )
                                    .on_hover_text(tip);
                                };
                                field_icon(ui, icon::SIGN_IN, t!("tip.in"));
                                if time_field(ui, &mut a, dur, "", t!("tip.in")) {
                                    new_in = Some(a);
                                }
                                vline(ui, DECK_H);
                                field_icon(ui, icon::SIGN_OUT, t!("tip.out"));
                                if time_field(ui, &mut b, dur, "", t!("tip.out")) {
                                    new_out = Some(b);
                                }
                                vline(ui, DECK_H);
                                field_icon(ui, icon::RULER, t!("tip.length"));
                                let mut len = b - a;
                                if time_field(ui, &mut len, dur - a, "", t!("tip.length")) {
                                    new_len = Some(len);
                                }
                            },
                        );
                    })
                    .response
                    .rect
                    .width()
            },
        );
        if (shown.inner - width).abs() > 0.5 {
            self.anchors.selection_width = shown.inner;
            ui.ctx().request_repaint();
        }
        if let Some(a) = new_in {
            self.set_in(a);
        }
        if let Some(b) = new_out {
            self.set_out(b);
        }
        if let Some(len) = new_len {
            self.set_out(a + len);
        }
    }

    /// Filmstrip with Quick Look–style trim handles and a playhead.
    /// Under a zoomed timeline: which part of the video is in view, as a scrollbar thumb (with the
    /// playhead as a tick). Click or drag to move the view.
    fn timeline_scroll(&mut self, ui: &Ui, strip: Rect, dur: f64) {
        if self.timeline.zoom <= 1.001 {
            return;
        }
        // In the deck's bottom margin, so showing it doesn't move anything.
        let track = Rect::from_x_y_ranges(strip.x_range(), strip.max.y + 4.0..=strip.max.y + 12.0);
        let resp = ui.interact(
            track,
            ui.id().with("timeline_scroll"),
            Sense::click_and_drag(),
        );
        let at = |t: f64| track.min.x + (t / dur) as f32 * track.width();
        let (v0, v1) = self.timeline.view(dur);
        // Pressing beside the thumb centers the view there; dragging then moves it by the
        // pointer's movement, so the thumb stays where it was grabbed.
        let pressed = resp.contains_pointer() && ui.input(|i| i.pointer.primary_pressed());
        if pressed
            && let Some(p) = resp.hover_pos()
            && !(at(v0)..=at(v1)).contains(&p.x)
        {
            let f = ((p.x - track.min.x) / track.width()).clamp(0.0, 1.0) as f64;
            self.pan_timeline(f * dur, dur);
        } else if resp.dragged() {
            let d = resp.drag_delta().x as f64 / track.width() as f64 * dur;
            self.pan_timeline((v0 + v1) / 2.0 + d, dur);
        }
        let (v0, v1) = self.timeline.view(dur);
        let hot = resp.hovered() || resp.dragged();
        let h = if hot { 4.0 } else { 2.0 };
        let y = track.center().y;
        let rail = |a: f32, b: f32| Rect::from_x_y_ranges(a..=b, y - h / 2.0..=y + h / 2.0);
        let p = ui.painter();
        p.rect_filled(rail(track.min.x, track.max.x), 2.0, LINE);
        let (a, b) = (at(v0), at(v1).max(at(v0) + 6.0));
        p.rect_filled(rail(a, b), 2.0, if hot { GREEN } else { GREEN_DIM });
        let x = at(self.pos());
        p.vline(x, y - 4.0..=y + 4.0, Stroke::new(1.0, TEXT));
        if hot {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        resp.on_hover_text(format!(
            "{} · {} – {}",
            zoom_label(self.timeline.zoom),
            fmt_time(v0),
            fmt_time(v1)
        ));
    }

    fn trim_bar(&mut self, ui: &mut Ui) {
        let dur = self.video.as_ref().map_or(0.0, |v| v.dur).max(0.001);
        let (v0, v1) = self.timeline.view(dur);
        let span = (v1 - v0).max(1e-6);
        let aspect = THUMB[0] as f32 / THUMB[1] as f32;
        let (r, resp) = ui.allocate_exact_size(
            vec2(ui.available_width().max(60.0), TRIM_H),
            Sense::click_and_drag(),
        );
        // Handles sit outside the kept range, with a little breathing room from the window edge.
        let strip = r.shrink2(vec2(HANDLE + 8.0, 0.0));
        self.timeline_scroll(ui, strip, dur);
        let x = |t: f64| strip.min.x + ((t - v0) / span) as f32 * strip.width();
        let t_at = |px: f32| {
            let f = ((px - strip.min.x) / strip.width()).clamp(0.0, 1.0) as f64;
            (v0 + f * span).clamp(0.0, dur)
        };
        let (xi, xo) = (x(self.trim.0), x(self.trim.1));
        let xi_vis = strip.x_range().contains(xi);
        let xo_vis = strip.x_range().contains(xo);

        // When the timeline is zoomed, one or both trim edges can sit outside the view.
        // Pin those handles to the nearest view edge so they never become unreachable.
        let same_side_off = !xi_vis
            && !xo_vis
            && ((xi < strip.min.x && xo < strip.min.x) || (xi > strip.max.x && xo > strip.max.x));
        let in_pin = !xi_vis && !same_side_off;
        let out_pin = !xo_vis && !same_side_off;
        let pin_rect = |x: f32| {
            let (lo, hi) = if x < strip.min.x {
                (strip.min.x, strip.min.x + HANDLE)
            } else {
                (strip.max.x - HANDLE, strip.max.x)
            };
            Rect::from_x_y_ranges(lo..=hi, r.y_range())
        };
        let in_hr = if xi_vis {
            Rect::from_x_y_ranges(xi - HANDLE..=xi, r.y_range())
        } else if in_pin {
            pin_rect(xi)
        } else {
            Rect::NOTHING
        };
        let out_hr = if xo_vis {
            Rect::from_x_y_ranges(xo..=xo + HANDLE, r.y_range())
        } else if out_pin {
            pin_rect(xo)
        } else {
            Rect::NOTHING
        };
        let in_show = in_hr.is_positive();
        let out_show = out_hr.is_positive();

        // Wheel over the strip zooms around the pointer; horizontal scroll pans a zoomed view.
        if resp.hovered() && dur > 0.0 {
            // Consume the smoothed wheel delta so one notch does not keep zooming as it decays.
            let (pan, scroll) = ui.input_mut(|i| {
                let d = i.smooth_scroll_delta;
                i.smooth_scroll_delta = Vec2::ZERO;
                (d.x, d.y)
            });
            // A trackpad swipe carries a little of the other axis; only the larger one counts,
            // so a sideways swipe pans without zooming.
            if pan.abs() > scroll.abs() {
                let c = (v0 + v1) / 2.0 - pan as f64 / strip.width() as f64 * span;
                self.pan_timeline(c, dur);
            } else if scroll.abs() > 0.01 {
                let p = resp.hover_pos().unwrap_or(r.center());
                let f = ((p.x - strip.min.x) / strip.width()).clamp(0.0, 1.0) as f64;
                let anchor = v0 + f * span;
                let factor = (scroll as f64 * 0.005).exp().clamp(0.5, 2.0);
                self.zoom_timeline_at(factor, anchor, f, dur);
            }
        }

        // What sits under `p`: a visible handle, the selection's top band, or the strip.
        let hit = |p: Pos2| {
            let (di, dout) = (
                (p.x - in_hr.center().x).abs(),
                (p.x - out_hr.center().x).abs(),
            );
            if in_show && di < HANDLE * 2.0 && di <= dout {
                Drag::In
            } else if out_show && dout < HANDLE * 2.0 {
                Drag::Out
            } else if xi_vis
                && xo_vis
                && (p.y < r.min.y + GRIP || p.y > r.max.y - GRIP)
                && (xi..=xo).contains(&p.x)
            {
                Drag::Slide
            } else {
                Drag::Scrub
            }
        };

        if resp.drag_started() || resp.clicked() {
            // Where the press began; see `stage`.
            let o = ui
                .input(|i| i.pointer.press_origin())
                .or(resp.interact_pointer_pos())
                .unwrap_or(r.min);
            let d = hit(o);
            self.drag = Some(d);
            self.grab = vec2(
                match d {
                    Drag::In | Drag::Slide if xi_vis => xi - o.x,
                    Drag::Out if xo_vis => xo - o.x,
                    _ => 0.0,
                },
                0.0,
            );
            let _ = self.player.mpv.set_property("pause", true);
        }
        if (resp.dragged() || resp.clicked())
            && let Some(p) = resp.interact_pointer_pos()
        {
            let t = t_at(p.x + self.grab.x).clamp(0.0, dur);
            match self.drag {
                Some(Drag::In) => self.set_in(t),
                Some(Drag::Out) => self.set_out(t),
                Some(Drag::Scrub) => self.player.seek(t.clamp(self.trim.0, self.trim.1)),
                Some(Drag::Slide) => {
                    self.trim = slide(self.trim, t, dur);
                    self.player.seek(self.trim.0);
                }
                _ => {}
            }
        }
        // Holding a handle still zooms in around it for frame-level trimming; letting go zooms back.
        if resp.dragged()
            && matches!(self.drag, Some(Drag::In | Drag::Out))
            && let Some(p) = resp.interact_pointer_pos()
        {
            let now = Instant::now();
            match self.timeline.hold {
                Some((_, x)) if (p.x - x).abs() <= 2.0 => {}
                _ => self.timeline.hold = Some((now, p.x)),
            }
            let since = self.timeline.hold.map_or(now, |h| h.0);
            if self.timeline.restore.is_none() {
                if now.duration_since(since) >= HOLD {
                    let (t, xe) = if self.drag == Some(Drag::In) {
                        (self.trim.0, xi)
                    } else {
                        (self.trim.1, xo)
                    };
                    let f = ((xe - strip.min.x) / strip.width()).clamp(0.0, 1.0) as f64;
                    self.timeline.restore = Some((self.timeline.zoom, self.timeline.center));
                    self.zoom_timeline_at(HOLD_ZOOM, t, f, dur);
                } else {
                    ui.ctx()
                        .request_repaint_after(HOLD - now.duration_since(since));
                }
            }
        }
        if resp.drag_stopped() || resp.clicked() {
            self.drag = None;
            self.timeline.hold = None;
            if let Some((z, c)) = self.timeline.restore.take() {
                self.set_timeline(z, c, dur);
            }
        }
        match self.drag.or_else(|| resp.hover_pos().map(hit)) {
            Some(Drag::In | Drag::Out) => ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal),
            Some(Drag::Slide) if self.drag.is_some() => {
                ui.ctx().set_cursor_icon(CursorIcon::Grabbing)
            }
            Some(Drag::Slide) => ui.ctx().set_cursor_icon(CursorIcon::Grab),
            _ => {}
        }
        resp.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Slider, true, t!("a11y.trim"))
        });

        let p = ui.painter_at(r);
        p.rect_filled(r, 5.0, CARD);

        let (ta, tb) = self.timeline.thumb_range;
        let range_ready = (ta - v0).abs() < 1e-6 && (tb - v1).abs() < 1e-6;
        if range_ready {
            let slot = strip.width() / THUMBS as f32;
            for (i, tex) in self.thumbs.iter().enumerate() {
                let Some(tex) = tex else { continue };
                let sr = Rect::from_min_size(
                    pos2(strip.min.x + i as f32 * slot, r.min.y),
                    vec2(slot, r.height()),
                );
                // Center-crop the thumbnail to the slot's aspect.
                let s = (sr.aspect_ratio() / aspect).min(1.0);
                let t = (aspect / sr.aspect_ratio()).min(1.0);
                let uv = Rect::from_center_size(pos2(0.5, 0.5), vec2(s, t));
                p.image(tex.id(), sr, uv, THUMB_TINT);
            }
        }
        if !range_ready || self.thumbs.iter().all(Option::is_none) {
            p.text(
                r.center(),
                Align2::CENTER_CENTER,
                t!("timeline.sampling"),
                mono(10.0),
                MUTED,
            );
        }

        let sx0 = xi.clamp(r.min.x, r.max.x);
        let sx1 = xo.clamp(r.min.x, r.max.x);

        // Dim the parts of the visible range outside the selection.
        if self.trim.0 > v0 {
            let xe = xi.clamp(r.min.x, r.max.x);
            p.rect_filled(Rect::from_min_max(r.min, pos2(xe, r.max.y)), 0.0, DIM);
        }
        if self.trim.1 < v1 {
            let xs = xo.clamp(r.min.x, r.max.x);
            p.rect_filled(Rect::from_min_max(pos2(xs, r.min.y), r.max), 0.0, DIM);
        }

        // Inactive stretches that strip will cut, so the selection predicts the export. While a
        // scan runs, they fill in as it goes, minus the last `pad` before the scan front: activity
        // just ahead could still keep that part.
        let found = match (&self.strip.scan, &self.strip.job) {
            (Some(scan), _) => Some((&scan.active, self.trim.1)),
            // Up to the drawn line, but only as far as the scanner has confirmed.
            (None, Some(_)) => Some((
                &self.strip.partial,
                self.trim
                    .1
                    .min(self.strip.front.shown.min(self.strip.scanned) - self.strip.pad),
            )),
            _ => None,
        };
        if self.strip.enabled
            && let Some((active, end)) = found
            && end > self.trim.0
        {
            let keep = strip::keep(active, self.strip.pad, self.trim.0, end);
            let mut cursor = self.trim.0;
            let mut gaps = Vec::new();
            for (a, b) in keep {
                if a > cursor {
                    gaps.push((cursor, a));
                }
                cursor = cursor.max(b);
            }
            if cursor < end {
                gaps.push((cursor, end));
            }
            for (a, b) in gaps {
                let a = a.max(v0);
                let b = b.min(v1);
                if b > a {
                    let gr = Rect::from_x_y_ranges(x(a)..=x(b), r.y_range());
                    hatch(&p, gr);
                }
            }
        }
        // Where the running scan has read to.
        if self.strip.enabled && self.strip.job.is_some() {
            let drawn = self.strip.front.tick();
            let front = drawn.clamp(self.trim.0, self.trim.1);
            if (v0..=v1).contains(&front) && drawn > 0.0 {
                let xf = x(front);
                // A green wake behind the line, fading out to the left; its length and glow
                // breathe like a wave while the scan runs.
                let wave = (ui.input(|i| i.time) * 3.0).sin() as f32;
                let len = 56.0 + 14.0 * wave;
                let peak = 0.40 + 0.10 * wave;
                let mut wake = egui::Mesh::default();
                const STEPS: u32 = 8;
                for i in 0..=STEPS {
                    let f = i as f32 / STEPS as f32;
                    // Eased: mostly faint, brightening toward the line.
                    let c = GREEN.gamma_multiply(peak * f * f);
                    let wx = (xf - len * (1.0 - f)).max(strip.min.x);
                    wake.colored_vertex(pos2(wx, r.min.y), c);
                    wake.colored_vertex(pos2(wx, r.max.y), c);
                    if i > 0 {
                        let k = 2 * i;
                        wake.add_triangle(k - 2, k - 1, k);
                        wake.add_triangle(k - 1, k, k + 1);
                    }
                }
                p.add(wake);
                p.vline(
                    xf,
                    r.y_range(),
                    Stroke::new(1.5, GREEN.gamma_multiply(0.85)),
                );
                ui.ctx().request_repaint();
            }
        }

        // Quick Look–style trim box: solid bars along the top and bottom of the selection,
        // joined to rounded handles with grip lines. Off-view edges get hollow pinned handles.
        if sx1 > sx0 {
            let bar_x = (sx0.max(strip.min.x))..=(sx1.min(strip.max.x));
            for y in [r.min.y..=r.min.y + FRAME, r.max.y - FRAME..=r.max.y] {
                p.rect_filled(Rect::from_x_y_ranges(bar_x.clone(), y), 0.0, GREEN);
            }
        }
        for (hr, inner, visible, pinned) in [
            (
                in_hr,
                egui::CornerRadius {
                    nw: 7,
                    sw: 7,
                    ne: 0,
                    se: 0,
                },
                in_show,
                in_pin,
            ),
            (
                out_hr,
                egui::CornerRadius {
                    nw: 0,
                    sw: 0,
                    ne: 7,
                    se: 7,
                },
                out_show,
                out_pin,
            ),
        ] {
            if !visible {
                continue;
            }
            let grips = [hr.center().x - 2.5, hr.center().x + 2.5];
            let ys = hr.y_range().shrink(hr.height() * 0.28);
            if pinned {
                let hr = hr.shrink(1.0);
                p.rect_filled(hr, 4.0, CARD);
                p.rect_stroke(hr, 4.0, Stroke::new(1.5, GREEN), egui::StrokeKind::Inside);
                for gx in grips {
                    p.vline(gx, ys, Stroke::new(1.0, GREEN));
                }
            } else {
                p.rect_filled(hr, inner, GREEN);
                for gx in grips {
                    p.vline(gx, ys, Stroke::new(1.5, INK));
                }
            }
        }

        let ph = x(self.pos());
        if strip.x_range().contains(ph) {
            // Reaches a little past the strip, so it reads above the trim box.
            let p = ui.painter();
            let ys = r.y_range().expand(5.0);
            p.vline(ph, ys, Stroke::new(5.0, Color32::from_white_alpha(40)));
            p.vline(ph, ys, Stroke::new(2.0, Color32::WHITE));
        }
        if let Some(at) = self.timeline.thumb_after {
            ui.ctx()
                .request_repaint_after(at.saturating_duration_since(Instant::now()));
        }
        let hint = if in_pin || out_pin {
            t!("tip.trim_pinned")
        } else {
            t!("tip.trim")
        };
        let _ = resp.on_hover_text(hint);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slide_keeps_length() {
        assert_eq!(slide((2.0, 5.0), 4.0, 10.0), (4.0, 7.0));
        assert_eq!(slide((2.0, 5.0), 9.0, 10.0), (7.0, 10.0));
        assert_eq!(slide((2.0, 5.0), -1.0, 10.0), (0.0, 3.0));
    }

    #[test]
    fn free_preset_keeps_crop_rect() {
        let r = Rect::from_min_max(pos2(0.2, 0.3), pos2(0.6, 0.8));
        assert_eq!(crop_for_preset(0, r, None), r);
        assert_eq!(
            crop_for_preset(2, r, Some(16.0 / 9.0)),
            crop::fit(16.0 / 9.0)
        );
    }

    #[test]
    fn scan_front_glides_but_never_runs_ahead_or_back() {
        let mut f = Glide::default();
        f.reported(0.0);
        std::thread::sleep(Duration::from_millis(100));
        f.reported(10.0); // 100 s of video per second
        let mut last = f.tick();
        // No more reports: the scan stalled. The line keeps easing forward, but stops within
        // `LEAD` seconds of scanning past the last report.
        for _ in 0..30 {
            std::thread::sleep(Duration::from_millis(10));
            let x = f.tick();
            assert!(x >= last, "went back: {x} < {last}");
            last = x;
        }
        assert!(
            last > 10.0 && last <= 10.0 + 100.0 * Glide::LEAD + 1.0,
            "{last}"
        );
    }

    #[test]
    fn speed_text() {
        assert_eq!(speed_label(0.25), "0.25x");
        assert_eq!(speed_label(0.5), "0.5x");
        assert_eq!(speed_label(1.0), "1x");
        assert_eq!(speed_label(1.25), "1.25x");
        assert_eq!(speed_label(4.0), "4x");
        assert_eq!(speed_label(1.333), "1.33x");
    }

    #[test]
    fn size_format() {
        assert_eq!(fmt_size(999), "999 B");
        assert_eq!(fmt_size(1536), "1.5 KB");
        assert_eq!(fmt_size(5 * 1024 * 1024), "5.0 MB");
        assert_eq!(fmt_size(2 * 1024 * 1024 * 1024), "2.0 GB");
    }

    #[test]
    fn time_round_trip() {
        assert_eq!(fmt_time(0.0), "0:00.000");
        assert_eq!(fmt_time(59.9996), "1:00.000");
        assert_eq!(fmt_time(3723.0456), "1:02:03.046");
        assert_eq!(parse_time("1:02:03.046"), Some(3723.046));
        assert_eq!(parse_time(" 2:05.5 "), Some(125.5));
        assert_eq!(parse_time("90.25"), Some(90.25));
        for bad in ["", "x", "1:2:3:4", "nan", "-1", "1::2"] {
            assert_eq!(parse_time(bad), None, "{bad}");
        }
    }
}
