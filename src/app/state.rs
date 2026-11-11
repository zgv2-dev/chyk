use super::view::ThumbKey;
use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Drag {
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

pub(super) struct Video {
    pub(super) path: PathBuf,
    pub(super) dur: f64,
    /// Frame size after rotation, in pixels. This is what ffmpeg's crop filter sees.
    pub(super) size: [u32; 2],
    /// Display aspect ratio after rotation.
    pub(super) aspect: f32,
    pub(super) codec: String,
    /// File size, for the export size estimate.
    pub(super) bytes: u64,
}

/// Detection state for stripping the stretches where nothing on screen changes.
#[derive(Default)]
pub(super) struct Strip {
    pub(super) enabled: bool,
    /// Seconds kept around each active interval.
    pub(super) pad: f64,
    /// Ignore zones, normalized to the displayed frame.
    pub(super) zones: Vec<Rect>,
    /// Whether zones are being drawn/edited on the video.
    pub(super) edit: bool,
    pub(super) selected: Option<usize>,
    pub(super) scan: Option<strip::Scan>,
    pub(super) job: Option<strip::Running>,
    pub(super) progress: f32,
    /// Crop the current scan (or the running one) looked inside; `None` = the whole frame.
    pub(super) scan_crop: Option<Rect>,
    /// Source times the current (or running) scan covers: the trim plus padding when it started.
    pub(super) scan_range: (f64, f64),
    /// The scan need when the user cancelled an extension: don't start it again by itself.
    pub(super) held: Option<(f64, f64)>,
    /// While scanning: active intervals found so far, and how far the scan has read (seconds).
    pub(super) partial: Vec<(f64, f64)>,
    pub(super) scanned: f64,
    /// The scan line as drawn, moving smoothly between the scanner's 100 ms reports.
    pub(super) front: Glide,
    /// Zone and handle being dragged.
    pub(super) drag: Option<(usize, Drag)>,
    /// Zone rect when the drag began, to notice a real change.
    pub(super) zone_start: Option<Rect>,
    /// Offset from the pointer to the grabbed corner.
    pub(super) grab: Vec2,
}

/// Zoom/pan state for the trim bar.
#[derive(Default)]
pub(super) struct Timeline {
    pub(super) zoom: f32,
    /// Center of the visible range, in seconds.
    pub(super) center: f64,
    /// Range the current filmstrip thumbnails cover.
    pub(super) thumb_range: (f64, f64),
    /// Debounce deadline for regenerating thumbnails.
    pub(super) thumb_after: Option<Instant>,
    /// Filmstrips shown before, by [`ThumbKey`], so a view seen again reuses its tiles; and the
    /// key of the one shown now.
    pub(super) strips: HashMap<ThumbKey, Vec<Option<TextureHandle>>>,
    pub(super) strip_key: Option<ThumbKey>,
    /// While a trim handle is dragged: when the pointer last moved, and where.
    pub(super) hold: Option<(Instant, f32)>,
    /// View to return to when a hold-zoomed handle drag ends.
    pub(super) restore: Option<(f32, f64)>,
}

/// A question waiting in the center modal.
#[derive(Clone, PartialEq)]
pub(super) enum Confirm {
    /// A file dropped or picked while another video is open: replace it or open a new window?
    Open(PathBuf),
    /// Closing the window (or quitting) with edits that were never exported.
    Quit,
    /// Closing the video with edits that were never exported.
    CloseVideo,
}

/// The edits an export carries: trim, crop, strip (with its areas and padding) and speed.
/// Export settings (quality, format, mute) aren't edits.
pub(super) type Edits = ((f64, f64), Option<Rect>, Option<(Vec<Rect>, f64)>, f64);

/// Which mode panel is open. Closing a panel keeps its effect: the crop stays applied, Strip stays on.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Panel {
    Crop,
    Strip,
}

/// Which popover is open.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Pop {
    Export,
    Info,
}

/// Where the popovers hang from, recorded while the header is laid out.
pub(super) struct Anchors {
    pub(super) export: Rect,
    pub(super) info: Rect,
    pub(super) header_bottom: f32,
    /// Width of the IN/OUT/LEN box last frame.
    pub(super) selection_width: f32,
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
    pub(super) fn view(&self, dur: f64) -> (f64, f64) {
        zoom::timeline_view(dur, self.zoom, self.center)
    }
}
