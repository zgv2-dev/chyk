use super::*;

/// Grab `n` evenly spaced `w`×`h` frames from `start..end`, sent as `(index, image)` in any order.
/// Each tile is its own input seek, so long files don't get decoded end to end. Tiles more than
/// [`KEYFRAME_SPAN`] apart take the nearest keyframe (no decoding past it); closer ones seek
/// exactly. With `crop` (normalized), tiles show only that part of the frame. Stops early once
/// `tx`'s receiver is dropped. `clips` are the files and where each starts on the timeline (one
/// at 0 for a single video; joined clips laid end to end).
pub fn thumbnails(
    clips: Vec<(PathBuf, f64)>,
    (start, end): (f64, f64),
    n: usize,
    [w, h]: [usize; 2],
    crop: Option<eframe::egui::Rect>,
    tx: Sender<(usize, ColorImage)>,
    ctx: Context,
) {
    let id = log::begin(log::Kind::Thumbnails);
    let step = (end - start).max(0.001) / n.max(1) as f64;
    let coarse = step > KEYFRAME_SPAN;
    let next = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    for _ in 0..THUMB_JOBS.min(n) {
        let (clips, tx, ctx, next) = (clips.clone(), tx.clone(), ctx.clone(), next.clone());
        std::thread::spawn(move || {
            loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= n {
                    break;
                }
                let t = start + (i as f64 + 0.5) * step;
                let Some((src, rel)) = pick(&clips, t) else {
                    break;
                };
                let Some(img) = frame(id, src, rel, coarse, [w, h], crop) else {
                    continue;
                };
                if tx.send((i, img)).is_err() {
                    break;
                }
                crate::wake(&ctx);
            }
        });
    }
}

/// Tiles further apart than this (seconds) use the nearest keyframe instead of an exact seek.
pub(super) const KEYFRAME_SPAN: f64 = 2.0;
/// ffmpeg processes decoding filmstrip tiles at once.
pub(super) const THUMB_JOBS: usize = 4;

/// One `w`×`h` RGBA frame at `t`, center-cropped to fill, logged as a run of `entry`.
pub(super) fn frame(
    entry: u64,
    src: &Path,
    t: f64,
    keyframe: bool,
    [w, h]: [usize; 2],
    crop: Option<eframe::egui::Rect>,
) -> Option<ColorImage> {
    let crop = crop.map_or_else(String::new, |c| {
        format!(
            "crop=w=iw*{}:h=ih*{}:x=iw*{}:y=ih*{},",
            c.width(),
            c.height(),
            c.min.x,
            c.min.y
        )
    });
    let mut cmd = command("ffmpeg");
    cmd.args(["-v", "error", "-nostdin"]);
    if keyframe {
        cmd.args(["-skip_frame", "nokey", "-noaccurate_seek"]);
    }
    cmd.args(["-ss", &format!("{t:.3}"), "-i"]).arg(src).args([
        "-frames:v",
        "1",
        "-an",
        "-sn",
        "-vf",
        &format!("{crop}scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h}"),
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgba",
        "-",
    ]);
    let out = log::output(entry, &mut cmd).ok()?;
    (out.stdout.len() >= w * h * 4)
        .then(|| ColorImage::from_rgba_unmultiplied([w, h], &out.stdout[..w * h * 4]))
}

/// The clip at timeline time `t` and the time inside it. `clips` are `(path, timeline offset)` in
/// timeline order; before the first offset, the first clip.
pub(super) fn pick(clips: &[(PathBuf, f64)], t: f64) -> Option<(&Path, f64)> {
    let (src, off) = clips.iter().rev().find(|c| c.1 <= t).or(clips.first())?;
    Some((src, (t - off).max(0.0)))
}
