use super::*;

pub fn export(mut job: Job, ctx: Context) -> Running {
    let compress = job.quality == COMPRESS;
    let (tx, rx) = channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let stop = cancel.clone();
    std::thread::spawn(move || {
        // ffmpeg writes beside the destination, which is replaced only by a finished export:
        // a failed or cancelled one leaves an existing file as it was.
        let dst = std::mem::replace(&mut job.dst, PathBuf::new());
        job.dst = part::path(&dst);
        // Only a part file this export created is removed (by its guard), never one it found.
        let res = part::claim(&job.dst, &dst).and_then(|()| {
            let _part = TempFile(job.dst.clone(), None);
            run(&job, &tx, &stop, &ctx)?;
            if stop.load(Ordering::Relaxed) {
                return Err("cancelled".into());
            }
            std::fs::rename(&job.dst, &dst).map_err(|e| t!("error.save", error = e))
        });
        if !stop.load(Ordering::Relaxed) {
            let res = res.map(|()| std::fs::metadata(&dst).map(|m| m.len()).unwrap_or_default());
            let _ = tx.send(match res {
                Err(e) if e == COPY_FAILED => Msg::CopyFailed,
                res => Msg::Done(res),
            });
            crate::wake(&ctx);
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
    if !job.speed.is_finite() || !(0.25..=4.0).contains(&job.speed) {
        return Err(t!("error.speed").into());
    }
    if let Some(keep) = &job.keep
        && (keep.is_empty() || job.kept_duration() <= 0.0)
    {
        return Err(t!("status.nothing_left").into());
    }
    let joined = job.join.as_ref().map(|j| prepare(job, j, stop));
    // A cancelled prepare leaves its probe half done: stop before using it.
    if stop.load(Ordering::Relaxed) {
        return Err("cancelled".into());
    }
    let job = joined.as_ref().map_or(job, |p| &p.job);
    let copy = joined.as_ref().is_none_or(|p| p.same)
        && job.crop.is_none()
        && job.scale.is_none()
        && !job.precise
        && job.quality == 0
        && job.keep.is_none()
        && !job.retime();
    // Copy when allowed. Re-encode if it fails, e.g. a codec the new container can't hold. A
    // join that fails to copy stops instead: the app asks before re-encoding it.
    let join_copy = copy && job.join.is_some();
    let cands = match join_copy {
        true => Vec::new(),
        false => encoder_order(&job.codec, encoders(), job.quality == COMPRESS),
    };
    let encs: Vec<Option<&str>> = copy
        .then_some(None)
        .into_iter()
        .chain(cands.into_iter().map(Some))
        .collect();
    if encs.is_empty() {
        return Err(t!("error.no_encoder").into());
    }
    let dur = job.output_duration().max(0.001);
    let mut source = joined
        .as_ref()
        .map_or_else(|| probe_source(&job.src), |p| p.source);
    let src_len = joined.as_ref().map_or_else(
        || std::fs::metadata(&job.src).map_or(u64::MAX, |m| m.len()),
        |p| p.bytes,
    );
    let id = log::begin(log::Kind::Export);
    if join_copy && !join_check::check_seams(job, id, stop) {
        return Err(COPY_FAILED.into());
    }
    let mut err = String::new();
    // Each encoder, and a re-encode that came out bigger than the source once more, strictly capped.
    for (enc, strict) in encs.iter().flat_map(|&e| [(e, false), (e, true)]) {
        if strict && (enc.is_none() || !source.strict) {
            continue;
        }
        if stop.load(Ordering::Relaxed) {
            return Err("cancelled".into());
        }
        let mut a = args(job, enc, &source);
        // A lossless join reads its clips from a list file.
        let _list = match (enc, &job.join) {
            (None, Some(j)) => Some(
                write_list(&mut a, &job.src, j).map_err(|e| t!("error.run_ffmpeg", error = e))?,
            ),
            _ => None,
        };
        let _graph = spill_graph(&mut a, &job.src);
        let at = Instant::now();
        let mut cmd = command("ffmpeg");
        cmd.args(a).stdout(Stdio::piped()).stderr(Stdio::piped());
        let (mut child, run) =
            log::spawn(id, &mut cmd).map_err(|e| t!("error.run_ffmpeg", error = e))?;
        let errs = log::stderr(id, run, &mut child);
        // The log shows the latest `-progress` block, which ends with `progress=`.
        let mut block = Vec::new();
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
                crate::wake(ctx);
            }
            let end = line.starts_with("progress=");
            block.push(line);
            if end {
                log::progress(id, &block.join(" "));
                block.clear();
            }
        }
        let status = child.wait().ok();
        log::exited(id, run, status, at.elapsed());
        let ok = status.is_some_and(|s| s.success());
        if stop.load(Ordering::Relaxed) {
            return Err("cancelled".into());
        }
        if ok {
            let out_len = std::fs::metadata(&job.dst).map_or(0, |m| m.len());
            // A re-encode must not outgrow its source. Constant-quality modes of some encoders
            // (QSV, AMF) ignore -maxrate: retry this encoder at a plain bitrate that fits. Not a
            // join: clips scaled up to its frame rightly outgrow their files.
            if enc.is_some()
                && !strict
                && job.join.is_none()
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
    match join_copy {
        true => Err(COPY_FAILED.into()),
        false => Err(error_tail(&err)),
    }
}

/// The error of a join whose lossless copy failed; [`export`] sends it as
/// [`Msg::CopyFailed`].
const COPY_FAILED: &str = "\u{1}copy-failed\u{1}";
