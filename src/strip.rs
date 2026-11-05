//! Finding the parts of a video where the picture actually changes, so the static parts can be cut.

use crate::ffmpeg;
use crate::i18n::t;
use eframe::egui::{Context, Pos2, Rect, pos2};
use std::io::{BufReader, ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

/// Frames compared per second. 10 Hz is plenty for screen recordings and keeps hours of video cheap.
pub const FPS: f64 = 10.0;
/// Pixels per grayscale frame used for comparison: 320×180 for a 16:9 frame. A crop gets the
/// same budget for its own area (never more than its real pixels), so small regions keep detail.
const SAMPLE_AREA: f32 = 320.0 * 180.0;
/// A pixel counts as changed when its gray value moves by more than this.
const DIFF: u8 = 10;
/// Fewer changed pixels than this are noise: compression artifacts, a blinking cursor.
/// At 320 px wide this still catches a single typed character on a 1080p screen.
const MIN_PIXELS: usize = 6;

#[derive(Clone)]
pub struct Scan {
    /// Absolute source times where something changed, merged and followed by one sample.
    pub active: Vec<(f64, f64)>,
}

pub enum Msg {
    Progress(f32),
    /// Scan results so far, for live feedback: replace intervals from index `from` on (the last
    /// one sent may have grown) with `intervals`; everything before `scanned` (seconds) is known.
    Partial {
        from: usize,
        intervals: Vec<(f64, f64)>,
        scanned: f64,
    },
    Done(Result<Scan, String>),
}

pub struct Running {
    pub rx: Receiver<Msg>,
    cancel: Arc<AtomicBool>,
}

impl Running {
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// Scan `src` in the background. `aspect` is the displayed (rotated) aspect ratio, so the
/// sampled frames line up with the rectangles drawn over the video.
/// What to compare: the frame's display `aspect` and pixel `size`, the applied `crop` (only it ends
/// up in the export), and ignored `zones`, all normalized to the whole frame.
pub struct Area {
    pub aspect: f32,
    pub size: [u32; 2],
    pub crop: Option<Rect>,
    pub zones: Vec<Rect>,
}

pub fn scan(src: PathBuf, dur: f64, area: Area, ctx: Context) -> Running {
    let (tx, rx) = channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let stop = cancel.clone();
    std::thread::spawn(move || {
        let res = run(&src, dur, &area, &tx, &stop, &ctx);
        if !stop.load(Ordering::Relaxed) {
            let _ = tx.send(Msg::Done(res));
            ctx.request_repaint();
        }
    });
    Running { rx, cancel }
}

fn run(
    src: &Path,
    dur: f64,
    area: &Area,
    tx: &Sender<Msg>,
    stop: &AtomicBool,
    ctx: &Context,
) -> Result<Scan, String> {
    let full = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));
    let c = area.crop.unwrap_or(full);
    let pixels_in = [
        (area.size[0] as f32 * c.width()) as usize,
        (area.size[1] as f32 * c.height()) as usize,
    ];
    let (w, h) = sample_size(area.aspect * c.width() / c.height().max(1e-6), pixels_in);
    // Zones relative to the cropped picture; ones outside it clamp to nothing.
    let zones: Vec<Rect> = area
        .zones
        .iter()
        .map(|z| {
            Rect::from_min_max(
                ((z.min - c.min) / c.size()).to_pos2(),
                ((z.max - c.min) / c.size()).to_pos2(),
            )
        })
        .collect();
    let pixels = compared_pixels(w, h, &zones);
    if pixels.is_empty() {
        return Err(t!("error.zones_cover").into());
    }

    // Cut the crop out before scaling, so it is sampled at its own resolution.
    let crop = area.crop.map_or_else(String::new, |c| {
        format!(
            "crop=w=iw*{}:h=ih*{}:x=iw*{}:y=ih*{},",
            c.width(),
            c.height(),
            c.min.x,
            c.min.y
        )
    });
    let filter = format!("fps={FPS},{crop}scale={w}:{h}:flags=fast_bilinear,format=gray");
    let mut child = ffmpeg::command("ffmpeg")
        // Frames no other frame depends on can be skipped: the 10 fps samples still land within a
        // frame or two of their time, and nothing else decodes differently. ~25% faster on HEVC.
        // More decoder threads than cores keeps them busy while the filter thread works (~13%).
        .args([
            "-hide_banner",
            "-nostdin",
            "-v",
            "error",
            "-skip_frame",
            "noref",
        ])
        .args(["-threads", &decode_threads().to_string(), "-i"])
        .arg(src)
        .args([
            "-an", "-sn", "-dn", "-map", "0:v:0", "-vf", &filter, "-f", "rawvideo", "-pix_fmt",
            "gray", "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| t!("error.run_ffmpeg", error = e))?;
    let mut stderr = child.stderr.take().unwrap();
    let errs = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s);
        s
    });

    let step = 1.0 / FPS;
    let frame = w * h;
    let mut prev = vec![0u8; frame];
    let mut cur = vec![0u8; frame];
    let mut reader = BufReader::new(child.stdout.take().unwrap());
    let mut active: Vec<(f64, f64)> = Vec::new();
    let mut idx = 0usize;
    let total = (dur * FPS).max(1.0);
    let mut last = Instant::now();
    // Intervals already sent as `Partial`; the last of them may still grow.
    let mut sent = 0usize;
    let res = loop {
        if stop.load(Ordering::Relaxed) {
            let _ = child.kill();
            break Err("cancelled".into());
        }
        match reader.read_exact(&mut cur) {
            Ok(()) => {}
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => break Ok(()),
            Err(e) => {
                let _ = child.kill();
                break Err(t!("error.read", error = e));
            }
        }
        if idx > 0 && changed_pixels(&prev, &cur, &pixels) >= MIN_PIXELS {
            // Keep the frame before the change too, so the transition itself isn't cut.
            let t = idx as f64 * step;
            let a = (t - step).max(0.0);
            let b = t + step;
            let b = if dur > 0.0 { b.min(dur) } else { b };
            if b > a {
                match active.last_mut() {
                    Some(last) if a <= last.1 + 1e-6 => last.1 = last.1.max(b),
                    _ => active.push((a, b)),
                }
            }
        }
        prev.copy_from_slice(&cur);
        idx += 1;
        // Throttle by time, not by percent: a three-hour scan should still tick every 100 ms.
        if last.elapsed() >= Duration::from_millis(100) {
            last = Instant::now();
            let progress = (idx as f64 / total).min(1.0) as f32;
            let _ = tx.send(Msg::Progress(progress));
            let from = sent.saturating_sub(1);
            let _ = tx.send(Msg::Partial {
                from,
                intervals: active[from..].to_vec(),
                scanned: idx as f64 * step,
            });
            sent = active.len();
            ctx.request_repaint();
        }
    };
    let ok = child.wait().is_ok_and(|s| s.success());
    let err = errs.join().unwrap_or_default();
    if stop.load(Ordering::Relaxed) {
        return Err("cancelled".into());
    }
    res?;
    if !ok {
        return Err(ffmpeg::error_tail(&err));
    }
    if idx == 0 {
        return Err(if err.is_empty() {
            t!("error.no_frames").into()
        } else {
            err
        });
    }
    Ok(Scan { active })
}

/// Decoder threads for a scan: twice the cores, at most 16.
fn decode_threads() -> usize {
    std::thread::available_parallelism()
        .map_or(8, |n| n.get() * 2)
        .min(16)
}

/// Active intervals expanded by `pad` seconds on both sides, merged, and clamped to `[lo, hi]`.
/// `active` must be sorted (the scanner produces it that way).
pub fn keep(active: &[(f64, f64)], pad: f64, lo: f64, hi: f64) -> Vec<(f64, f64)> {
    let pad = pad.max(0.0);
    let mut out: Vec<(f64, f64)> = Vec::new();
    for &(a, b) in active {
        let a = (a - pad).max(lo);
        let b = (b + pad).min(hi);
        if b <= a {
            continue;
        }
        match out.last_mut() {
            Some(last) if a <= last.1 + 1e-6 => last.1 = last.1.max(b),
            _ => out.push((a, b)),
        }
    }
    out
}

/// Pixel size used for comparisons: about `SAMPLE_AREA` pixels at display `aspect`, but no larger
/// than the source's own `[w, h]` pixels (0 = unknown), even dimensions.
fn sample_size(aspect: f32, source: [usize; 2]) -> (usize, usize) {
    let aspect = if aspect.is_finite() && aspect > 0.0 {
        aspect
    } else {
        16.0 / 9.0
    };
    let (w, h) = ((SAMPLE_AREA * aspect).sqrt(), (SAMPLE_AREA / aspect).sqrt());
    let fit = |have: usize, want: f32| if have == 0 { 1.0 } else { have as f32 / want };
    let s = fit(source[0], w).min(fit(source[1], h)).min(1.0);
    let even = |v: f32| (v.round() as usize).max(2) & !1;
    (even(w * s), even(h * s))
}

/// Flat pixel indices outside every ignore zone.
fn compared_pixels(w: usize, h: usize, zones: &[Rect]) -> Vec<usize> {
    let mut mask = vec![false; w * h];
    for z in zones {
        let x0 = (z.min.x.clamp(0.0, 1.0) * w as f32).floor() as usize;
        let y0 = (z.min.y.clamp(0.0, 1.0) * h as f32).floor() as usize;
        let x1 = ((z.max.x.clamp(0.0, 1.0) * w as f32).ceil() as usize).min(w);
        let y1 = ((z.max.y.clamp(0.0, 1.0) * h as f32).ceil() as usize).min(h);
        for y in y0..y1 {
            for x in x0..x1 {
                mask[y * w + x] = true;
            }
        }
    }
    (0..w * h).filter(|&i| !mask[i]).collect()
}

fn changed_pixels(prev: &[u8], cur: &[u8], pixels: &[usize]) -> usize {
    pixels
        .iter()
        .filter(|&&i| prev[i].abs_diff(cur[i]) > DIFF)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::pos2;

    /// A 320×180 source.
    fn area(zones: Vec<Rect>, crop: Option<Rect>) -> Area {
        Area {
            aspect: 320.0 / 180.0,
            size: [320, 180],
            crop,
            zones,
        }
    }

    /// Needs `ffmpeg` on PATH. A 4×4 dot blinking in a 1280×720 frame is below the noise floor
    /// of a whole-frame scan, but a crop around it is sampled at full detail and catches it.
    #[test]
    fn crop_scans_at_its_own_resolution() {
        let dir = std::env::temp_dir().join(format!("glint-crop-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("dot.mp4");
        let ok = std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=1280x720:r=10:d=4",
                "-f",
                "lavfi",
                "-i",
                "color=c=white:s=4x4:r=10:d=4",
                "-filter_complex",
                "[0:v][1:v]overlay=1001:601:enable='lt(mod(t,2),1)'[v]",
                "-map",
                "[v]",
                "-c:v",
                "libx264",
                "-crf",
                "10",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&src)
            .status()
            .unwrap();
        assert!(ok.success());
        let at = |crop| Area {
            aspect: 16.0 / 9.0,
            size: [1280, 720],
            crop,
            zones: vec![],
        };
        let whole = wait(scan(src.clone(), 4.0, at(None), Context::default()));
        assert!(whole.active.is_empty(), "{:?}", whole.active);
        let corner = Rect::from_min_max(pos2(0.75, 0.75), pos2(1.0, 1.0));
        let cropped = wait(scan(src, 4.0, at(Some(corner)), Context::default()));
        assert!(!cropped.active.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The final scan, after checking that the live `Partial` updates rebuild a prefix of it.
    fn wait(run: Running) -> Scan {
        let mut partial: Vec<(f64, f64)> = Vec::new();
        let scan = loop {
            match run
                .rx
                .recv_timeout(std::time::Duration::from_secs(120))
                .expect("scan timed out")
            {
                Msg::Done(r) => break r.unwrap(),
                Msg::Progress(_) => {}
                Msg::Partial {
                    from, intervals, ..
                } => {
                    partial.truncate(from);
                    partial.extend(intervals);
                }
            }
        };
        // Every interval but the last sent one is final; the last may have grown since.
        let settled = partial.len().saturating_sub(1);
        assert_eq!(
            partial[..settled],
            scan.active[..settled.min(scan.active.len())]
        );
        scan
    }

    #[test]
    fn pad_merges_and_clamps() {
        let active = [(1.0, 2.0), (4.0, 5.0)];
        assert_eq!(keep(&active, 0.0, 0.0, 10.0), [(1.0, 2.0), (4.0, 5.0)]);
        assert_eq!(keep(&active, 0.5, 0.0, 10.0), [(0.5, 2.5), (3.5, 5.5)]);
        // Two paddings meet: the gap disappears.
        assert_eq!(keep(&active, 1.0, 0.0, 10.0), [(0.0, 6.0)]);
        assert_eq!(keep(&active, 9.0, 0.0, 10.0), [(0.0, 10.0)]);
        assert_eq!(keep(&[(1.0, 2.0)], 0.0, 1.5, 2.5), [(1.5, 2.0)]);
        assert_eq!(keep(&[(1.0, 2.0)], -1.0, 0.0, 10.0), [(1.0, 2.0)]);
        assert!(keep(&[], 1.0, 0.0, 10.0).is_empty());
        assert!(keep(&[(1.0, 1.0)], 0.0, 0.0, 10.0).is_empty());
        // Adjacent intervals can appear at sample boundaries.
        assert_eq!(
            keep(&[(1.0, 2.0), (2.0, 3.0)], 0.0, 0.0, 10.0),
            [(1.0, 3.0)]
        );
    }

    #[test]
    fn sample_sizes_keep_aspect() {
        assert_eq!(sample_size(1920.0 / 1080.0, [1920, 1080]), (320, 180));
        assert_eq!(sample_size(1080.0 / 1920.0, [1080, 1920]), (180, 320));
        assert_eq!(sample_size(4.0 / 3.0, [0, 0]), (276, 208));
        assert_eq!(sample_size(0.0, [0, 0]), (320, 180));
        assert_eq!(sample_size(f32::NAN, [0, 0]), (320, 180));
        // A thin 982×86 crop of a 1080p frame keeps nearly all of its pixels…
        assert_eq!(sample_size(982.0 / 86.0, [982, 86]), (810, 70));
        // …and a tiny one is never scaled up.
        assert_eq!(sample_size(1.0, [100, 100]), (100, 100));
    }

    #[test]
    fn ignore_zones_mask_pixels() {
        let (w, h) = (320, 180);
        let mut a = vec![0u8; w * h];
        let mut b = a.clone();
        for y in 10..14 {
            for x in 10..14 {
                b[y * w + x] = 200;
            }
        }
        let all: Vec<usize> = (0..w * h).collect();
        assert_eq!(changed_pixels(&a, &b, &all), 16);

        // A 2-level shift is compression noise, not activity.
        a.fill(5);
        b.fill(7);
        assert_eq!(changed_pixels(&a, &b, &all), 0);

        let zone = Rect::from_min_max(
            pos2(8.0 / w as f32, 8.0 / h as f32),
            pos2(20.0 / w as f32, 20.0 / h as f32),
        );
        let pixels = compared_pixels(w, h, &[zone]);
        assert_eq!(changed_pixels(&a, &b, &pixels), 0);
        assert_eq!(compared_pixels(w, h, &[crate::crop::full()]).len(), 0);
    }

    /// Needs `ffmpeg` on PATH.
    #[test]
    fn scan_finds_motion_only() {
        let dir = std::env::temp_dir().join(format!("glint-strip-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("motion.mp4");
        let ok = std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=320x180:r=25:d=2",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=s=320x180:r=25:d=1",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=320x180:r=25:d=3",
                "-filter_complex",
                "[0:v][1:v][2:v]concat=n=3:v=1:a=0[v]",
                "-map",
                "[v]",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&src)
            .status()
            .unwrap();
        assert!(ok.success());

        let scan = wait(scan(src, 6.0, area(vec![], None), Context::default()));
        assert!(!scan.active.is_empty());
        // testsrc2 runs 2 s..3 s; sampling adds at most one 100 ms frame at each edge.
        for &(a, b) in &scan.active {
            assert!((1.8..3.3).contains(&a), "active starts at {a}");
            assert!((1.8..3.3).contains(&b), "active ends at {b}");
        }
        let total: f64 = scan.active.iter().map(|(a, b)| b - a).sum();
        assert!((1.0..2.2).contains(&total), "active duration {total}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Needs `ffmpeg` on PATH.
    #[test]
    fn ignore_zone_hides_a_clock() {
        let dir = std::env::temp_dir().join(format!("glint-zone-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("clock.mp4");
        let ok = std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=320x180:r=10:d=4",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=s=40x40:r=10:d=4",
                "-filter_complex",
                "[0:v][1:v]overlay=10:10[v]",
                "-map",
                "[v]",
                "-c:v",
                "libx264",
                "-g",
                "10",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&src)
            .status()
            .unwrap();
        assert!(ok.success());

        // The only moving thing is the overlay in the top-left corner.
        let all = wait(scan(
            src.clone(),
            4.0,
            area(vec![], None),
            Context::default(),
        ));
        assert!(!all.active.is_empty());

        let clock = Rect::from_min_max(pos2(0.0, 0.0), pos2(0.2, 0.3));
        let masked = wait(scan(src, 4.0, area(vec![clock], None), Context::default()));
        assert!(masked.active.is_empty(), "{:?}", masked.active);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
