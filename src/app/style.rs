use super::*;

/// Phosphor green: everything active.
pub(super) const GREEN: Color32 = Color32::from_rgb(77, 255, 155);
/// Dim green for secondary icons.
pub(super) const GREEN_DIM: Color32 = Color32::from_rgb(31, 143, 85);
pub(super) const RED: Color32 = Color32::from_rgb(255, 92, 120);
pub(super) const BG: Color32 = Color32::BLACK;
/// Header, deck and popover fill.
pub(super) const PANEL: Color32 = Color32::from_rgb(3, 8, 5);
/// Raised controls.
pub(super) const CARD: Color32 = Color32::from_rgb(7, 19, 12);
pub(super) const CARD_HI: Color32 = Color32::from_rgb(12, 30, 19);
/// Hairline borders.
pub(super) const LINE: Color32 = Color32::from_rgb(18, 48, 31);
pub(super) const TEXT: Color32 = Color32::from_rgb(207, 238, 221);
pub(super) const MUTED: Color32 = Color32::from_rgb(95, 138, 113);
/// Text and glyphs on green.
pub(super) const INK: Color32 = Color32::from_rgb(0, 20, 10);
/// Floating bars over the video (86% opaque panel).
pub(super) const GLASS: Color32 = Color32::from_rgba_premultiplied(3, 9, 5, 219);
/// Outside the crop and the trim selection.
pub(super) const DIM: Color32 = Color32::from_black_alpha(163);
/// How long a confirmation (like "Saved") stays up, the last part of it fading out.
pub(super) const STATUS_LIFE: Duration = Duration::from_secs(4);
pub(super) const STATUS_FADE: Duration = Duration::from_millis(400);
/// Height of the shortcut legend (two rows of 22 pt key caps, 8 pt apart).
pub(super) const LEGEND_H: f32 = 52.0;
/// Header row height; with 6 pt margins the header is 52 pt, and on macOS the native traffic
/// lights are moved to its vertical center (`chrome::place_traffic_lights`).
pub(super) const HEADER_ROW: f32 = 40.0;
/// Filmstrip thumbnails are tinted toward the phosphor green.
pub(super) const THUMB_TINT: Color32 = Color32::from_rgb(170, 220, 190);
pub(super) const THUMBS: usize = 12;
/// Filmstrip tile size in pixels. ffmpeg center-crops frames to it.
pub(super) const THUMB: [usize; 2] = [96, 72];
/// Shortest selectable trim, in seconds.
pub(super) const MIN_LEN: f64 = 0.1;
/// Trim handle width, in points.
pub(super) const HANDLE: f32 = 14.0;
/// Filmstrip height, in points.
pub(super) const TRIM_H: f32 = 52.0;
/// The same in windows under 520 pt tall, so the video keeps its height.
pub(super) const TRIM_H_SMALL: f32 = 34.0;
/// Time ruler above the filmstrip, in points.
pub(super) const RULER_H: f32 = 24.0;
/// Height of the bands along the selection's top and bottom edges that slide it, in points.
pub(super) const GRIP: f32 = 9.0;
/// Thickness of the trim box's top and bottom bars, in points.
pub(super) const FRAME: f32 = 4.0;
/// Holding a trim handle still this long zooms the timeline in around it, like QuickTime.
pub(super) const HOLD: Duration = Duration::from_millis(450);
/// How far that hold zooms in.
pub(super) const HOLD_ZOOM: f64 = 10.0;
/// Playback speed presets and limits.
pub(super) const SPEEDS: &[f64] = &[0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0, 4.0];
pub(super) const SPEED_MIN: f64 = 0.25;
pub(super) const SPEED_MAX: f64 = 4.0;

/// Size and colors of a hand-drawn button. One painter for every button keeps them consistent.
#[derive(Clone, Copy)]
pub(super) struct Look {
    pub(super) height: f32,
    pub(super) radius: f32,
    pub(super) text: f32,
    pub(super) icon: f32,
    /// Horizontal padding; icon-only buttons are square instead.
    pub(super) pad: f32,
    /// Label color when idle (icons stay muted).
    pub(super) idle: Color32,
    /// Space between icon and label.
    pub(super) gap: f32,
}

/// Header mode toggles. With their group's 2 pt margin and 1 pt stroke the group is 34 pt,
/// like every other box in the header (Export, Update, the Crop/Strip bars).
pub(super) const MODE: Look = Look {
    height: 28.0,
    radius: 6.0,
    text: 11.5,
    icon: 13.5,
    pad: 13.0,
    idle: MUTED,
    gap: 6.0,
};
/// Chips and buttons in the floating bars.
pub(super) const CHIP: Look = Look {
    height: 26.0,
    radius: 5.0,
    text: 11.0,
    icon: 14.0,
    pad: 10.0,
    idle: MUTED,
    gap: 7.0,
};
/// Labeled action in a floating bar ("Ignore area").
pub(super) const GHOST: Look = Look { idle: TEXT, ..CHIP };
/// Icon buttons in the header and transport.
pub(super) const ICON: Look = Look {
    height: 32.0,
    radius: 6.0,
    text: 12.0,
    icon: 17.0,
    pad: 0.0,
    idle: MUTED,
    gap: 7.0,
};
/// The log window's kind filter.
pub(super) const SEG: Look = Look {
    height: 22.0,
    radius: 5.0,
    text: 11.0,
    icon: 11.0,
    pad: 8.0,
    idle: MUTED,
    gap: 0.0,
};
/// Height of every group and button in the transport row (speed, zoom, IN/OUT/LEN, icons).
pub(super) const DECK_H: f32 = 30.0;
/// Icon buttons in the transport row.
pub(super) const DECK_ICON: Look = Look {
    height: DECK_H,
    ..ICON
};
/// Icon buttons inside floating bars.
pub(super) const ICON_SMALL: Look = Look {
    height: 26.0,
    icon: 15.0,
    ..ICON
};
