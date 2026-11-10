use super::*;

/// `ffmpeg -encoders` output, read once.
pub(super) fn encoders() -> &'static str {
    static LIST: OnceLock<String> = OnceLock::new();
    LIST.get_or_init(|| {
        command("ffmpeg")
            .args(["-hide_banner", "-encoders"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default()
    })
}

/// Encoders worth trying for a source codec, given the output of `ffmpeg -encoders`.
pub fn candidates(codec: &str, available: &str) -> Vec<&'static str> {
    let has = |e: &&str| {
        available
            .lines()
            .any(|l| l.split_whitespace().nth(1) == Some(*e))
    };
    let hevc = if codec == "hevc" { HEVC } else { &[] };
    hevc.iter()
        .chain(H264)
        .copied()
        .filter(|e| has(e))
        .collect()
}

/// Encoders to try, in order. Compression prefers software encoders: they squeeze out smaller files.
pub(super) fn encoder_order(codec: &str, available: &str, compress: bool) -> Vec<&'static str> {
    let mut encs = candidates(codec, available);
    if compress {
        encs.sort_by_key(|e| !matches!(*e, "libx264" | "libx265"));
    }
    encs
}

/// Encoder settings for `level`, an index into [`QUALITIES`], never above `cap` kb/s (the
/// source's video bitrate, so a re-encode can't outgrow it). `strict` trades the quality mode for
/// a plain bitrate at `cap`, for encoders whose constant-quality modes ignore `-maxrate`.
pub(super) fn quality(enc: &str, level: usize, cap: Option<u32>, strict: bool) -> Vec<String> {
    let level = level.min(COMPRESS);
    let q = |v: [u32; 4]| v[level];
    let family = enc.rsplit('_').next().unwrap_or(enc);
    if let (Some(k), true) = (cap, strict) {
        let rc = if family == "amf" { "-rc vbr_peak " } else { "" };
        return format!("{rc}-b:v {k}k -maxrate {k}k -bufsize {}k", 2 * k)
            .split(' ')
            .map(String::from)
            .collect();
    }
    // Compression is worth the slower preset: it spends encoding time to save bytes.
    let preset = if level == COMPRESS { "slow" } else { "medium" };
    let mut a = match family {
        "videotoolbox" => format!("-q:v {}", q([65, 50, 35, 28])),
        "nvenc" => format!("-rc vbr -cq {} -b:v 0", q([23, 28, 33, 38])),
        "qsv" => format!("-global_quality {}", q([23, 28, 33, 38])),
        "amf" => format!("-rc cqp -qp_i {0} -qp_p {0}", q([22, 28, 34, 38])),
        "libx264" => format!("-crf {} -preset {preset}", q([20, 24, 28, 32])),
        "libx265" => format!("-crf {} -preset {preset}", q([22, 26, 30, 34])),
        // shortcut: fixed bitrate for encoders without a quality mode, revisit if output looks soft.
        _ => format!(
            "-b:v {}k",
            cap.map_or(1000, |k| k.max(1)).min(q([12, 6, 3, 1]) * 1000)
        ),
    };
    // VideoToolbox ignores -q:v under a rate limit: every quality came out the same size. It runs
    // uncapped; `run` redoes an encode that outgrew its source at a plain bitrate.
    if let Some(k) = cap
        && family != "videotoolbox"
    {
        a.push_str(&format!(" -maxrate {k}k -bufsize {}k", 2 * k));
    }
    a.split(' ').map(String::from).collect()
}

/// What an export needs to know about its source, read from `ffmpeg -i` (ffprobe isn't always
/// bundled).
#[derive(Clone, Copy, Default)]
pub struct Source {
    pub audio: bool,
    /// The first video stream's bitrate in kb/s, else the whole file's: a re-encode's ceiling.
    pub video_kbps: Option<u32>,
    /// The first audio stream's bitrate in kb/s.
    pub audio_kbps: Option<u32>,
    /// Re-encode at a plain bitrate (see [`quality`]); set when a capped encode still came out
    /// bigger than the source.
    pub strict: bool,
}

/// `Source` from `ffmpeg -i` output.
pub(super) fn parse_source(listing: &str) -> Source {
    let kbps = |l: &str| {
        l.split(", ").find_map(|part| {
            let (n, _) = part.split_once(" kb/s")?;
            n.trim().rsplit(' ').next()?.parse::<u32>().ok()
        })
    };
    let mut src = Source::default();
    let (mut overall, mut video, mut seen_video) = (None, None, false);
    for l in listing.lines().map(str::trim_start) {
        if l.starts_with("Duration:") {
            overall = l.split_once("bitrate: ").and_then(|(_, r)| kbps(r));
        } else if l.starts_with("Stream #") && l.contains(": Video:") && !seen_video {
            seen_video = true;
            video = kbps(l);
        } else if l.starts_with("Stream #") && l.contains(": Audio:") && !src.audio {
            src.audio = true;
            src.audio_kbps = kbps(l);
        }
    }
    src.video_kbps = video.or(overall);
    src
}

pub(super) fn probe_source(src: &Path) -> Source {
    command("ffmpeg")
        .args(["-hide_banner", "-i"])
        .arg(src)
        .output()
        .map(|o| parse_source(&String::from_utf8_lossy(&o.stderr)))
        .unwrap_or_default()
}

/// AAC bitrate for re-encoded audio: 96k when compressing, else 128k, but never above the
/// source's audio (a re-encode can't add detail), and at least 32k.
pub(super) fn audio_rate(job: &Job, src: &Source) -> String {
    let want = if job.quality == COMPRESS { 96 } else { 128 };
    format!("{}k", src.audio_kbps.map_or(want, |k| want.min(k.max(32))))
}
