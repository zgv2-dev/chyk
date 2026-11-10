use super::*;

/// Kept intervals closer than this share one input: decoding through the gap is cheaper than a
/// seek, which restarts decoding at the keyframe before the next interval.
pub(super) const MERGE_GAP: f64 = 10.0;

/// Most inputs (separate seeks) one strip export opens, for a source path of `path_len`
/// characters. Each input repeats the path on the command line, which on Windows is 32 KiB.
pub(super) fn max_inputs(path_len: usize) -> usize {
    (4000 / (path_len + 60)).clamp(1, 32)
}

/// Sorted `keep` intervals grouped into at most `max` runs (index ranges): gaps under
/// `join` seconds (exports use [`MERGE_GAP`]) are always joined, then the smallest gaps until
/// few enough remain.
pub(super) fn spans(keep: &[(f64, f64)], max: usize, join: f64) -> Vec<(usize, usize)> {
    let mut g: Vec<(usize, usize)> = (0..keep.len()).map(|i| (i, i + 1)).collect();
    let gap = |g: &[(usize, usize)], k: usize| keep[g[k + 1].0].0 - keep[g[k].1 - 1].1;
    loop {
        let k = (0..g.len().saturating_sub(1)).min_by(|&x, &y| gap(&g, x).total_cmp(&gap(&g, y)));
        match k {
            Some(k) if g.len() > max.max(1) || gap(&g, k) < join => {
                g[k].1 = g[k + 1].1;
                g.remove(k + 1);
            }
            _ => break,
        }
    }
    g
}

/// Strip export: one input per run of kept intervals, each seeked to its first interval, so the
/// cut stretches between runs are never decoded; `concat` joins the runs.
pub(super) fn strip_args(
    job: &Job,
    keep: &[(f64, f64)],
    enc: &str,
    src: &Source,
    (max, join): (usize, f64),
) -> Vec<String> {
    let ms = |t: f64| (t * 1000.0).round() / 1000.0;
    let audio = src.audio && !job.mute;
    let mut a = head();
    let mut graph = String::new();
    let mut pads = String::new();
    let runs = spans(keep, max, join);
    for (i, &(lo, hi)) in runs.iter().enumerate() {
        let part = &keep[lo..hi];
        let (start, end) = (ms(part[0].0), ms(part[part.len() - 1].1));
        a.extend([
            "-ss".into(),
            format!("{start:.3}"),
            "-to".into(),
            format!("{end:.3}"),
            "-i".into(),
            job.src.to_string_lossy().into_owned(),
        ]);
        // Interval times are relative to the seek point: `-ss` resets input timestamps to zero.
        let rel = relative(part, start);
        let sel = select_expr(&rel);
        let crop = job
            .crop
            .map_or_else(String::new, |[w, h, x, y]| format!("crop={w}:{h}:{x}:{y},"));
        let scale = job
            .scale
            .map_or_else(String::new, |[w, h]| format!("scale={w}:{h},"));
        graph.push_str(&format!(
            "[{i}:v:0]{crop}{scale}select='{sel}',setpts='{}'[v{i}];",
            close_gaps(&rel)
        ));
        pads.push_str(&format!("[v{i}]"));
        if audio {
            graph.push_str(&format!("[{i}:a:0]aselect='{sel}',asetpts=N/SR/TB[a{i}];"));
            pads.push_str(&format!("[a{i}]"));
        }
    }
    let n = runs.len();
    graph.push_str(&format!("{pads}concat=n={n}:v=1:a={}[v]", u8::from(audio)));
    if audio {
        graph.push_str("[a]");
    }
    if job.retime() {
        graph.push_str(&format!(";[v]setpts=PTS/{:.6}[vs]", job.speed));
        if audio {
            graph.push_str(&format!(";[a]{}[as]", atempo(job.speed)));
        }
    }
    let (v, aud) = if job.retime() {
        ("[vs]", "[as]")
    } else {
        ("[v]", "[a]")
    };
    a.extend(["-filter_complex".into(), graph, "-map".into(), v.into()]);
    if audio {
        a.extend(["-map".into(), aud.into()]);
    }
    a.extend(["-c:v".into(), enc.into()]);
    a.extend(quality(enc, job.quality, src.video_kbps, src.strict));
    if audio {
        // Filtering audio means re-encoding it.
        a.extend([
            "-c:a".into(),
            "aac".into(),
            "-b:a".into(),
            audio_rate(job, src),
        ]);
    }
    a.extend(["-map_chapters".into(), "-1".into()]);
    if job.mute {
        a.push("-an".into());
    }
    let mkv = job
        .dst
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("mkv"));
    if HEVC.contains(&enc) && !mkv {
        a.extend(["-tag:v".into(), "hvc1".into()]); // so QuickTime/Apple players open HEVC MP4s
    }
    a.push(job.dst.to_string_lossy().into_owned());
    a
}
