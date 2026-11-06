use super::*;

pub(super) fn run(
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
pub(super) fn decode_threads() -> usize {
    std::thread::available_parallelism()
        .map_or(8, |n| n.get() * 2)
        .min(16)
}
