//! Pure zoom/pan math for the video stage and the timeline.

use eframe::egui::{Pos2, Rect, Vec2, pos2, vec2};

/// Video zoom stops at 16x.
pub const MAX_VIDEO: f32 = 16.0;
/// Timeline zoom stops at a quarter-second window.
pub const MIN_SPAN: f64 = 0.25;

fn centered(center: Pos2, zoom: f32) -> Pos2 {
    let half = 0.5 / zoom.clamp(1.0, MAX_VIDEO);
    pos2(
        center.x.clamp(half, 1.0 - half),
        center.y.clamp(half, 1.0 - half),
    )
}

/// Normalized part of the video visible at `zoom`, centered as close to `center` as fits.
pub fn video_src(zoom: f32, center: Pos2) -> Rect {
    let z = zoom.clamp(1.0, MAX_VIDEO);
    Rect::from_center_size(centered(center, z), vec2(1.0 / z, 1.0 / z))
}

/// Zoom around `anchor` (screen point and view rect), keeping that video point under the pointer.
pub fn video_zoom(
    zoom: f32,
    center: Pos2,
    factor: f32,
    anchor: Option<(Pos2, Rect)>,
) -> (f32, Pos2) {
    let z = (zoom * factor).clamp(1.0, MAX_VIDEO);
    if z <= 1.0 + 1e-4 {
        return (1.0, pos2(0.5, 0.5));
    }
    let Some((p, view)) = anchor else {
        return (z, centered(center, z));
    };
    if view.width() <= 0.0 || view.height() <= 0.0 {
        return (z, centered(center, z));
    }
    let old = video_src(zoom, center);
    let f = (p - view.min) / view.size();
    let a = pos2(
        old.min.x + f.x * old.width(),
        old.min.y + f.y * old.height(),
    );
    let c = pos2(a.x - (f.x - 0.5) / z, a.y - (f.y - 0.5) / z);
    (z, centered(c, z))
}

/// Limit zoom and pan so the whole `crop` rect stays in view: zooming stops once it fills the frame.
pub fn contain(zoom: f32, center: Pos2, crop: Rect) -> (f32, Pos2) {
    let z = zoom
        .min(1.0 / crop.width().max(1e-3))
        .min(1.0 / crop.height().max(1e-3))
        .clamp(1.0, MAX_VIDEO);
    let half = 0.5 / z;
    // max/min, not clamp: float error can invert a range that is exactly the crop's size.
    let fit = |c: f32, lo: f32, hi: f32| c.max(hi - half).min(lo + half);
    let c = pos2(
        fit(center.x, crop.min.x, crop.max.x),
        fit(center.y, crop.min.y, crop.max.y),
    );
    (z, centered(c, z))
}

/// Drag the picture by `delta` screen points.
pub fn video_pan(center: Pos2, delta: Vec2, view: Vec2, zoom: f32) -> Pos2 {
    if view.x <= 0.0 || view.y <= 0.0 {
        return center;
    }
    let z = zoom.clamp(1.0, MAX_VIDEO);
    let d = delta / view * vec2(1.0 / z, 1.0 / z);
    centered(pos2(center.x - d.x, center.y - d.y), z)
}

/// Normalized video point → screen point.
pub fn source_to_screen(src: Rect, view: Rect, p: Pos2) -> Pos2 {
    view.min
        + vec2(
            (p.x - src.min.x) / src.width() * view.width(),
            (p.y - src.min.y) / src.height() * view.height(),
        )
}

/// Screen point → normalized video point.
pub fn screen_to_source(src: Rect, view: Rect, p: Pos2) -> Pos2 {
    src.min
        + vec2(
            (p.x - view.min.x) / view.width() * src.width(),
            (p.y - view.min.y) / view.height() * src.height(),
        )
}

/// Visible timeline range `(start, end)`.
pub fn timeline_view(dur: f64, zoom: f32, center: f64) -> (f64, f64) {
    let dur = dur.max(0.0);
    if dur <= 0.0 {
        return (0.0, 0.0);
    }
    let span = (dur / zoom.max(1.0) as f64).max(MIN_SPAN).min(dur);
    let half = span / 2.0;
    let c = center.clamp(half, dur - half);
    (c - half, c + half)
}

/// Zoom the timeline around `anchor`, which sits at fraction `fraction` of the strip.
pub fn timeline_zoom(dur: f64, zoom: f32, factor: f64, anchor: f64, fraction: f64) -> (f32, f64) {
    let dur = dur.max(0.0);
    if dur <= 0.0 {
        return (1.0, 0.0);
    }
    let max_zoom = (dur / MIN_SPAN).max(1.0);
    let z = ((zoom as f64 * factor).clamp(1.0, max_zoom)) as f32;
    let span = (dur / z as f64).max(MIN_SPAN).min(dur);
    let start = anchor - fraction * span;
    let center = (start + span / 2.0).clamp(span / 2.0, dur - span / 2.0);
    (z, center)
}

/// Full-duration view.
pub fn timeline_fit(dur: f64) -> (f32, f64) {
    (1.0, dur.max(0.0) / 2.0)
}

/// Zoom to the current in/out selection.
pub fn timeline_fit_selection(dur: f64, a: f64, b: f64) -> (f32, f64) {
    let dur = dur.max(0.0);
    if dur <= 0.0 || b <= a {
        return timeline_fit(dur);
    }
    let len = (b - a).max(MIN_SPAN);
    let max_zoom = (dur / MIN_SPAN).max(1.0);
    ((dur / len).clamp(1.0, max_zoom) as f32, (a + b) / 2.0)
}

#[cfg(test)]
mod tests;
