use super::*;

/// `keep` intervals relative to the input seek point `start`, rounded to the millisecond the
/// filter expressions are written with, so select and setpts agree exactly.
pub(super) fn relative(keep: &[(f64, f64)], start: f64) -> Vec<(f64, f64)> {
    let ms = |t: f64| (t * 1000.0).round() / 1000.0;
    keep.iter()
        .map(|&(a, b)| {
            let a = ms((a - start).max(0.0));
            (a, ms((b - start).max(a)))
        })
        .collect()
}

/// `select`/`aselect` expression for relative `keep` intervals.
pub(super) fn select_expr(keep: &[(f64, f64)]) -> String {
    sum(keep
        .iter()
        .map(|&(a, b)| format!("between(t,{a:.3},{b:.3})"))
        .collect())
}

/// `terms` added as a balanced tree, `(a+b)+(c+d)`. ffmpeg's expression parser rejects a flat
/// chain of more than 100 terms ("Cannot allocate memory"); nesting keeps every level short.
pub(super) fn sum(mut terms: Vec<String>) -> String {
    while terms.len() > 2 {
        terms = terms
            .chunks(2)
            .map(|c| match c {
                [a, b] => format!("({a}+{b})"),
                [a] => a.clone(),
                _ => unreachable!(),
            })
            .collect();
    }
    terms.join("+")
}

/// `setpts` expression that closes the cut gaps by shifting each kept interval back by the time
/// removed before it. Renumbering frames (`N/FRAME_RATE`) would squeeze variable-frame-rate
/// video, such as screen recordings, and drift from the audio.
pub(super) fn close_gaps(keep: &[(f64, f64)]) -> String {
    let start = format!("T-{:.3}", keep.first().map_or(0.0, |k| k.0));
    let shifts: Vec<String> = keep
        .windows(2)
        .map(|w| format!("gte(T,{:.3})*{:.3}", w[1].0, w[1].0 - w[0].1))
        .collect();
    match shifts.len() {
        0 => format!("({start})/TB"),
        1 => format!("({start}-{})/TB", shifts[0]),
        _ => format!("({start}-({}))/TB", sum(shifts)),
    }
}

/// Pitch-preserving audio time-stretch chain. `atempo` only accepts 0.5..2.0 per instance.
pub(super) fn atempo(speed: f64) -> String {
    let mut s = speed.max(0.001);
    let mut filters = Vec::new();
    while s > 2.0 {
        filters.push("atempo=2.000000".to_string());
        s /= 2.0;
    }
    while s < 0.5 {
        filters.push("atempo=0.500000".to_string());
        s *= 2.0;
    }
    filters.push(format!("atempo={s:.6}"));
    filters.join(",")
}

/// Arguments every export starts with.
pub(super) fn head() -> Vec<String> {
    [
        "-hide_banner",
        "-nostdin",
        "-y",
        "-v",
        "error",
        "-progress",
        "pipe:1",
        "-nostats",
        // Progress every 100 ms (default 500), so the export bar has something to follow.
        "-stats_period",
        "0.1",
    ]
    .map(String::from)
    .into()
}

/// ffmpeg arguments for `job` from `src`. `enc = None` means lossless stream copy.
pub fn args(job: &Job, enc: Option<&str>, src: &Source) -> Vec<String> {
    if let (Some(keep), Some(enc)) = (&job.keep, enc) {
        return strip_args(job, keep, enc, src, max_inputs(job.src.as_os_str().len()));
    }
    let mut a = head();
    a.extend([
        "-ss".into(),
        format!("{:.3}", job.start),
        "-to".into(),
        format!("{:.3}", job.end),
    ]);
    a.extend(["-i".into(), job.src.to_string_lossy().into_owned()]);
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    let retime = job.retime();
    match enc {
        None => a.extend(s(&[
            "-map",
            "0",
            "-dn",
            "-c",
            "copy",
            "-avoid_negative_ts",
            "make_zero",
        ])),
        Some(enc) => {
            a.extend(s(&["-map", "0:v:0", "-map", "0:a?"]));
            if !retime {
                a.extend(s(&["-map", "0:s?"]));
            }
            a.extend(s(&["-c", "copy", "-c:v", enc]));
            a.extend(quality(enc, job.quality, src.video_kbps, src.strict));
            let mut filters = Vec::new();
            if let Some([w, h, x, y]) = job.crop {
                filters.push(format!("crop={w}:{h}:{x}:{y}"));
            }
            if let Some([w, h]) = job.scale {
                filters.push(format!("scale={w}:{h}"));
            }
            if retime {
                filters.push(format!("setpts=PTS/{:.6}", job.speed));
            }
            if !filters.is_empty() {
                a.extend(["-vf".into(), filters.join(",")]);
            }
            let mut audio_filters = Vec::new();
            if retime && !job.mute {
                audio_filters.push(atempo(job.speed));
            }
            if !audio_filters.is_empty() {
                // Filtering audio means re-encoding it.
                a.extend(["-af".into(), audio_filters.join(",")]);
                a.extend(s(&["-c:a", "aac", "-b:a", &audio_rate(job, src)]));
            } else if job.quality == COMPRESS && !job.mute {
                // Copying audio would leave its share of the file untouched, so compress it too.
                a.extend(s(&["-c:a", "aac", "-b:a", &audio_rate(job, src)]));
            }
            if retime {
                a.extend(s(&["-map_chapters", "-1"]));
            }
        }
    }
    if job.mute {
        a.push("-an".into());
    }
    let mkv = |p: &Path| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("mkv"));
    // Subtitles and attachments don't carry across containers: MP4/MOV only take mov_text, MKV can't.
    // A retimed file drops them instead: their timestamps would no longer line up.
    if !retime {
        match (mkv(&job.src), mkv(&job.dst)) {
            (true, false) => a.extend(s(&["-map", "-0:t", "-c:s", "mov_text"])),
            (false, true) => a.extend(s(&["-c:s", "srt"])),
            _ => {}
        }
    }
    if enc.map_or(job.codec == "hevc", |e| HEVC.contains(&e)) && !mkv(&job.dst) {
        a.extend(s(&["-tag:v", "hvc1"])); // so QuickTime/Apple players open HEVC MP4s
    }
    a.push(job.dst.to_string_lossy().into_owned());
    a
}
