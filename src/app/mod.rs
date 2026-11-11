//! The editor window. The state lives here; each submodule adds one part of the UI or behavior.

use crate::i18n::{t, tn};
use crate::{chrome, crop, ffmpeg, i18n, meta, notify, player::Player, strip, zoom};
use eframe::egui::{
    self, Align, Align2, Color32, CursorIcon, FontId, Key, Layout, Modifiers, Pos2, Rect, Response,
    RichText, Sense, Stroke, TextureHandle, Ui, Vec2, pos2, vec2,
};
use egui_phosphor::regular as icon;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

mod action_buttons;
mod backdrop;
mod bars;
mod buttons;
mod commands;
mod controls;
mod crop_view;
mod deck;
mod deck_controls;
mod dropdown;
mod edit;
mod estimate;
mod export;
mod file;
mod focus;
mod format;
mod frame;
mod glide;
mod header;
mod header_modes;
mod history;
mod keys;
mod legend;
mod marks;
mod modal;
mod poll;
mod popover;
mod pops;
mod primary;
mod scan;
mod scan_range;
mod segmented;
mod session;
mod sheet;
mod shortcuts;
mod stage;
mod start;
mod state;
mod status;
mod status_bar;
mod strip_bar;
mod style;
mod theme;
mod trim;
mod trim_drag;
mod trim_paint;
mod trim_strip;
mod update_button;
mod videos;
mod view;
mod zones;

use action_buttons::*;
use backdrop::*;
use buttons::*;
use commands::pressed;
use controls::*;
use dropdown::*;
use estimate::*;
use focus::focus_ring;
use format::*;
pub(crate) use format::{fmt_size, fmt_time};
use glide::*;
use header::*;
use history::Snapshot;
use keys::*;
use legend::*;
use marks::*;
use modal::{enter_is_primary, modal_key};
use primary::*;
use scan_range::covers;
use segmented::segmented;
pub(crate) use session::Session;
use state::*;
use style::*;
use theme::*;
use trim::*;

pub struct App {
    player: Player,
    loading: Option<PathBuf>,
    video: Option<Video>,
    trim: (f64, f64),
    /// Normalized crop rect while crop mode is on.
    crop: Option<Rect>,
    preset: usize,
    precise: bool,
    /// Indexes into [`ffmpeg::FORMATS`], [`ffmpeg::QUALITIES`] and [`ffmpeg::SIZES`].
    format: usize,
    quality: usize,
    size: usize,
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
    /// Measures re-encoded output size for the export popover.
    sampler: Sampler,
    progress: f32,
    /// Message and whether it is an error.
    status: Option<(String, bool)>,
    /// The message on screen and when it appeared, for fading confirmations out.
    status_since: Option<(String, Instant)>,
    /// Window buttons Chyk draws itself (none on macOS, which keeps its native ones).
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
    /// The `?` shortcut sheet, with its filter text, while open.
    sheet: Option<String>,
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
    update: crate::update::Updater,
    /// Width the header's middle section took last frame; the update button shrinks to leave it.
    center_need: f32,
    /// Crop, Strip and Export draw their key caps (see `header_modes::caps_fit`).
    header_caps: bool,
    /// Edits to put back once the video from `--restore` has loaded.
    restore: Option<Session>,
    history: history::History<history::Snapshot>,
    /// Opened by the status bar's log button; the log window itself comes later.
    log_open: bool,
    /// When the running export began, and the name of its output.
    export_started: Option<(Instant, String)>,
}
