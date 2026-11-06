use super::*;

/// Real-file check, off by default:
/// `CHYK_REAL=video.mp4 [CHYK_REAL_CROP=x0,y0,x1,y1] [CHYK_REAL_PAD=0.5] [CHYK_REAL_OUT=dir]
/// cargo test real_file -- --ignored --nocapture`
/// Scans (with the crop, normalized), then exports the strip both with one seek (the old way:
/// everything between the first and last kept frame is decoded) and with a seek per run, and
/// checks both outputs hold the same frames. Needs `ffmpeg` and `ffprobe` on PATH.
#[test]
#[ignore]
fn real_file() {
    use std::time::Instant;
    let Some(src) = std::env::var_os("CHYK_REAL").map(PathBuf::from) else {
        return;
    };
    let out = std::env::var_os("CHYK_REAL_OUT").map_or_else(std::env::temp_dir, PathBuf::from);
    let crop = std::env::var("CHYK_REAL_CROP").ok().map(|c| {
        let v: Vec<f32> = c.split(',').map(|x| x.trim().parse().unwrap()).collect();
        eframe::egui::Rect::from_min_max(
            eframe::egui::pos2(v[0], v[1]),
            eframe::egui::pos2(v[2], v[3]),
        )
    });
    let probe = |p: &Path, what: &str| {
        let o = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-count_packets",
                "-show_entries",
                what,
            ])
            .args(["-of", "csv=p=0:s=x"])
            .arg(p)
            .output()
            .unwrap();
        String::from_utf8_lossy(&o.stdout).trim().to_string()
    };
    let size: Vec<u32> = probe(&src, "stream=width,height")
        .split('x')
        .map(|v| v.parse().unwrap())
        .collect();
    let dur: f64 = probe(&src, "format=duration").parse().unwrap();

    let t = Instant::now();
    let area = crate::strip::Area {
        aspect: size[0] as f32 / size[1] as f32,
        size: [size[0], size[1]],
        crop,
        zones: vec![],
        range: None,
    };
    let job = crate::strip::scan(src.clone(), dur, area, Context::default());
    let scan = loop {
        if let crate::strip::Msg::Done(r) = job.rx.recv().unwrap() {
            break r.unwrap();
        }
    };
    let scan_s = t.elapsed().as_secs_f64();
    let pad = std::env::var("CHYK_REAL_PAD").map_or(0.5, |p| p.parse().unwrap());
    let keep = crate::strip::keep(&scan.active, pad, 0.0, dur);
    let kept: f64 = keep.iter().map(|(a, b)| b - a).sum();
    println!(
        "scan: {scan_s:.1}s for {dur:.0}s of video ({:.0}x realtime), {} runs kept, {kept:.1}s",
        dur / scan_s,
        keep.len()
    );

    let px = crop.map(|c| crate::crop::to_pixels(c, size[0], size[1]));
    let job = Job {
        src: src.clone(),
        dst: PathBuf::new(),
        start: 0.0,
        end: dur,
        crop: px,
        precise: false,
        codec: "hevc".into(),
        quality: 0,
        mute: false,
        speed: 1.0,
        keep: Some(keep.clone()),
    };
    let source = probe_source(&src);
    let src_len = std::fs::metadata(&src).unwrap().len();
    let list = String::from_utf8_lossy(
        &command("ffmpeg")
            .args(["-hide_banner", "-encoders"])
            .output()
            .unwrap()
            .stdout,
    )
    .into_owned();
    let enc = candidates("hevc", &list)[0];
    let mut frames = Vec::new();
    for (name, max) in [
        ("one-seek", 1),
        ("per-run", max_inputs(src.as_os_str().len())),
    ] {
        let dst = out.join(format!("chyk-real-{name}.mp4"));
        let job = Job {
            dst: dst.clone(),
            ..job.clone()
        };
        let t = Instant::now();
        let mut a = strip_args(&job, &keep, enc, &source, max);
        let graph = spill_graph(&mut a);
        println!("{name}: graph in a file: {}", graph.is_some());
        let ok = command("ffmpeg").args(&a).status().unwrap();
        assert!(ok.success(), "{name} export failed");
        let secs = t.elapsed().as_secs_f64();
        let n: u64 = probe(&dst, "stream=nb_read_packets").parse().unwrap();
        let d: f64 = probe(&dst, "format=duration").parse().unwrap();
        println!(
            "export {name}: {secs:.1}s, {} inputs, {n} frames, {d:.3}s ({enc})",
            spans(&keep, max).len()
        );
        assert!(
            (d - kept).abs() < 0.2,
            "{name}: duration {d} vs kept {kept}"
        );
        let len = std::fs::metadata(&dst).unwrap().len();
        println!("  {len} bytes (source {src_len})");
        assert!(len <= src_len, "{name}: {len} bytes, source {src_len}");
        frames.push(n);
    }
    assert!(
        frames[0].abs_diff(frames[1]) <= 2,
        "frame counts differ: {frames:?}"
    );
}
