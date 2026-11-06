use super::*;

pub fn export(job: Job, ctx: Context) -> Running {
    let compress = job.quality == COMPRESS;
    let (tx, rx) = channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let stop = cancel.clone();
    std::thread::spawn(move || {
        let res = run(&job, &tx, &stop, &ctx);
        if stop.load(Ordering::Relaxed) || res.is_err() {
            let _ = std::fs::remove_file(&job.dst);
        }
        if !stop.load(Ordering::Relaxed) {
            let res = res.map(|()| {
                std::fs::metadata(&job.dst)
                    .map(|m| m.len())
                    .unwrap_or_default()
            });
            let _ = tx.send(Msg::Done(res));
            ctx.request_repaint();
        }
    });
    Running {
        rx,
        cancel,
        compress,
    }
}

pub(super) fn run(
    job: &Job,
    tx: &Sender<Msg>,
    stop: &AtomicBool,
    ctx: &Context,
) -> Result<(), String> {
    static ENCODERS: OnceLock<String> = OnceLock::new();
    if !job.speed.is_finite() || !(0.25..=4.0).contains(&job.speed) {
        return Err(t!("error.speed").into());
    }
    if let Some(keep) = &job.keep
        && (keep.is_empty() || job.kept_duration() <= 0.0)
    {
        return Err(t!("status.nothing_left").into());
    }
    let copy = job.crop.is_none()
        && !job.precise
        && job.quality == 0
        && job.keep.is_none()
        && !job.retime();
    let list = ENCODERS.get_or_init(|| {
        command("ffmpeg")
            .args(["-hide_banner", "-encoders"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default()
    });
    // Copy when allowed. Re-encode if it fails, e.g. a codec the new container can't hold.
    let cands = encoder_order(&job.codec, list, job.quality == COMPRESS);
    let encs: Vec<Option<&str>> = copy
        .then_some(None)
        .into_iter()
        .chain(cands.into_iter().map(Some))
        .collect();
    if encs.is_empty() {
        return Err(t!("error.no_encoder").into());
    }
    let dur = job.output_duration().max(0.001);
    let mut source = probe_source(&job.src);
    let src_len = std::fs::metadata(&job.src).map_or(u64::MAX, |m| m.len());
    let mut err = String::new();
    // Each encoder, and a re-encode that came out bigger than the source once more, strictly capped.
    for (enc, strict) in encs.iter().flat_map(|&e| [(e, false), (e, true)]) {
        if strict && (enc.is_none() || !source.strict) {
            continue;
        }
        let mut a = args(job, enc, &source);
        let _graph = spill_graph(&mut a);
        let mut child = command("ffmpeg")
            .args(a)
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
        for line in BufReader::new(child.stdout.take().unwrap())
            .lines()
            .map_while(Result::ok)
        {
            if stop.load(Ordering::Relaxed) {
                let _ = child.kill();
                break;
            }
            if let Some(us) = line
                .strip_prefix("out_time_us=")
                .and_then(|v| v.parse::<f64>().ok())
            {
                let _ = tx.send(Msg::Progress((us / 1e6 / dur).clamp(0.0, 1.0) as f32));
                ctx.request_repaint();
            }
        }
        let ok = child.wait().is_ok_and(|s| s.success());
        if stop.load(Ordering::Relaxed) {
            return Err("cancelled".into());
        }
        if ok {
            let out_len = std::fs::metadata(&job.dst).map_or(0, |m| m.len());
            // A re-encode must not outgrow its source. Constant-quality modes of some encoders
            // (QSV, AMF) ignore -maxrate: retry this encoder at a plain bitrate that fits.
            if enc.is_some()
                && !strict
                && out_len > src_len
                && let Some(k) = source.video_kbps
            {
                let scale = src_len as f64 / out_len as f64 * 0.9;
                source.video_kbps = Some(((k as f64 * scale) as u32).max(1));
                source.strict = true;
                continue;
            }
            return Ok(());
        }
        err = errs.join().unwrap_or_default();
        source.strict = false;
    }
    Err(error_tail(&err))
}

/// The last lines of ffmpeg's stderr, each cut short: some errors echo a whole filter expression.
pub(crate) fn error_tail(err: &str) -> String {
    let tail: Vec<String> = err
        .lines()
        .filter(|l| !l.trim().is_empty())
        .rev()
        .take(3)
        .map(|l| {
            let l = l.trim();
            if l.chars().count() > 160 {
                format!("{}…", l.chars().take(160).collect::<String>())
            } else {
                l.to_string()
            }
        })
        .collect();
    if tail.is_empty() {
        t!("error.ffmpeg").into()
    } else {
        tail.into_iter().rev().collect::<Vec<_>>().join("\n")
    }
}
