//! Finding the parts of a video where the picture actually changes, so the static parts can be cut.

use crate::ffmpeg;
use crate::i18n::t;
use eframe::egui::{Context, Rect};
use std::io::{BufReader, ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

/// Frames compared per second. 10 Hz is plenty for screen recordings and keeps hours of video cheap.
pub const FPS: f64 = 10.0;
/// Longest side of the grayscale frames used for comparison.
const SAMPLE: usize = 320;
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
pub fn scan(src: PathBuf, dur: f64, aspect: f32, zones: Vec<Rect>, ctx: Context) -> Running {
    let (tx, rx) = channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let stop = cancel.clone();
    std::thread::spawn(move || {
        let res = run(&src, dur, aspect, &zones, &tx, &stop, &ctx);
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
    aspect: f32,
    zones: &[Rect],
    tx: &Sender<Msg>,
    stop: &AtomicBool,
    ctx: &Context,
) -> Result<Scan, String> {
    let (w, h) = sample_size(aspect);
    let pixels = compared_pixels(w, h, zones);
    if pixels.is_empty() {
        return Err(t!("error.zones_cover").into());
    }

    let filter = format!("fps={FPS},scale={w}:{h}:flags=fast_bilinear,format=gray");
    let mut child = ffmpeg::command("ffmpeg")
        .args(["-hide_banner", "-nostdin", "-v", "error", "-i"])
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
        let tail: Vec<&str> = err.lines().rev().take(3).collect();
        let msg = tail.into_iter().rev().collect::<Vec<_>>().join("\n");
        return Err(if msg.is_empty() {
            t!("error.ffmpeg").into()
        } else {
            msg
        });
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

/// Pixel size used for comparisons: 320 px on the long side, even dimensions.
fn sample_size(aspect: f32) -> (usize, usize) {
    let aspect = if aspect.is_finite() && aspect > 0.0 {
        aspect
    } else {
        16.0 / 9.0
    };
    let (mut sw, mut sh) = if aspect >= 1.0 {
        (SAMPLE, (SAMPLE as f32 / aspect).round() as usize)
    } else {
        ((SAMPLE as f32 * aspect).round() as usize, SAMPLE)
    };
    sw = sw.max(2) & !1;
    sh = sh.max(2) & !1;
    (sw, sh)
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

    fn wait(run: Running) -> Scan {
        loop {
            match run
                .rx
                .recv_timeout(std::time::Duration::from_secs(120))
                .expect("scan timed out")
            {
                Msg::Done(r) => break r.unwrap(),
                Msg::Progress(_) => {}
            }
        }
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
        assert_eq!(sample_size(1920.0 / 1080.0), (320, 180));
        assert_eq!(sample_size(1080.0 / 1920.0), (180, 320));
        assert_eq!(sample_size(4.0 / 3.0), (320, 240));
        assert_eq!(sample_size(0.0), (320, 180));
        assert_eq!(sample_size(f32::NAN), (320, 180));
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

        let scan = wait(scan(src, 6.0, 320.0 / 180.0, vec![], Context::default()));
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
            320.0 / 180.0,
            vec![],
            Context::default(),
        ));
        assert!(!all.active.is_empty());

        let clock = Rect::from_min_max(pos2(0.0, 0.0), pos2(0.2, 0.3));
        let masked = wait(scan(
            src,
            4.0,
            320.0 / 180.0,
            vec![clock],
            Context::default(),
        ));
        assert!(masked.active.is_empty(), "{:?}", masked.active);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
