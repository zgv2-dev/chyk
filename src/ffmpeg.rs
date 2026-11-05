//! Everything that shells out to the ffmpeg CLI: export and filmstrip thumbnails.

use crate::i18n::t;
use eframe::egui::{ColorImage, Context};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, OnceLock};

/// Encoders to try, best first. Unavailable ones are filtered out; failing ones fall through to the next.
const H264: &[&str] = &[
    "h264_videotoolbox",
    "h264_nvenc",
    "h264_qsv",
    "h264_amf",
    "libx264",
    "h264_mf",
    "libopenh264",
];
const HEVC: &[&str] = &[
    "hevc_videotoolbox",
    "hevc_nvenc",
    "hevc_qsv",
    "hevc_amf",
    "libx265",
    "hevc_mf",
];

/// `ffmpeg` bundled with Glint, else the one on PATH.
pub(crate) fn command(name: &str) -> Command {
    #[cfg(bundled)]
    let dir = Some(crate::bundle::dir());
    #[cfg(not(bundled))]
    let dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    let bundled = dir
        .map(|d| d.join(format!("{name}{}", std::env::consts::EXE_SUFFIX)))
        .filter(|p| p.is_file());
    let mut cmd = Command::new(bundled.as_deref().unwrap_or(Path::new(name)));
    cmd.stdin(Stdio::null());
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000); // CREATE_NO_WINDOW
    cmd
}

#[derive(Clone)]
pub struct Job {
    pub src: PathBuf,
    pub dst: PathBuf,
    pub start: f64,
    pub end: f64,
    /// `[w, h, x, y]` in source pixels.
    pub crop: Option<[u32; 4]>,
    /// Re-encode even without a crop, so the cut lands on the exact frame.
    pub precise: bool,
    /// Source video codec as named by ffmpeg (`h264`, `hevc`, ...).
    pub codec: String,
    /// Index into [`QUALITIES`]. Anything below the best re-encodes; [`COMPRESS`] also re-encodes audio.
    pub quality: usize,
    /// Drop all audio tracks.
    pub mute: bool,
    /// Retiming factor for the output. `1.0` leaves the source speed unchanged.
    pub speed: f64,
    /// Keep only these absolute source-time intervals, concatenated. Forces a re-encode.
    pub keep: Option<Vec<(f64, f64)>>,
}

impl Job {
    /// Source duration: the trim length, or the sum of the kept intervals.
    pub fn kept_duration(&self) -> f64 {
        self.keep.as_ref().map_or(self.end - self.start, |keep| {
            keep.iter().map(|&(a, b)| (b - a).max(0.0)).sum()
        })
    }

    pub fn retime(&self) -> bool {
        (self.speed - 1.0).abs() > 1e-9
    }

    /// Duration after retiming. Used for progress and estimates.
    pub fn output_duration(&self) -> f64 {
        self.kept_duration() / self.speed.max(0.001)
    }
}

/// Export containers, by file extension.
pub const FORMATS: &[&str] = &["mp4", "mov", "mkv"];
/// Quality levels, best first. Only the best one may stream-copy.
/// UI text keys, see [`crate::i18n`].
pub const QUALITIES: &[&str] = &[
    "quality.high",
    "quality.medium",
    "quality.low",
    "quality.compress",
];
/// Longest filter graph passed on the command line; a longer one goes in a file. Windows caps the
/// whole command line at 32 KiB, and each kept interval costs ~90 characters of expressions.
const GRAPH_INLINE: usize = 24_000;
/// Index into [`QUALITIES`] for the size-first preset: re-encodes video and audio for the smallest file.
pub const COMPRESS: usize = 3;

pub enum Msg {
    Progress(f32),
    /// Bytes written, after a successful export.
    Done(Result<u64, String>),
}

pub struct Running {
    pub rx: Receiver<Msg>,
    cancel: Arc<AtomicBool>,
    /// Whether this export used [`COMPRESS`].
    pub compress: bool,
}

impl Running {
    /// Stops ffmpeg and deletes the partial output.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
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
fn encoder_order(codec: &str, available: &str, compress: bool) -> Vec<&'static str> {
    let mut encs = candidates(codec, available);
    if compress {
        encs.sort_by_key(|e| !matches!(*e, "libx264" | "libx265"));
    }
    encs
}

/// Encoder settings for `level`, an index into [`QUALITIES`], never above `cap` kb/s (the
/// source's video bitrate, so a re-encode can't outgrow it). `strict` trades the quality mode for
/// a plain bitrate at `cap`, for encoders whose constant-quality modes ignore `-maxrate`.
fn quality(enc: &str, level: usize, cap: Option<u32>, strict: bool) -> Vec<String> {
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
    if let Some(k) = cap {
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
fn parse_source(listing: &str) -> Source {
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

fn probe_source(src: &Path) -> Source {
    command("ffmpeg")
        .args(["-hide_banner", "-i"])
        .arg(src)
        .output()
        .map(|o| parse_source(&String::from_utf8_lossy(&o.stderr)))
        .unwrap_or_default()
}

/// AAC bitrate for re-encoded audio: 96k when compressing, else 128k, but never above the
/// source's audio (a re-encode can't add detail), and at least 32k.
fn audio_rate(job: &Job, src: &Source) -> String {
    let want = if job.quality == COMPRESS { 96 } else { 128 };
    format!("{}k", src.audio_kbps.map_or(want, |k| want.min(k.max(32))))
}

/// `keep` intervals relative to the input seek point `start`, rounded to the millisecond the
/// filter expressions are written with, so select and setpts agree exactly.
fn relative(keep: &[(f64, f64)], start: f64) -> Vec<(f64, f64)> {
    let ms = |t: f64| (t * 1000.0).round() / 1000.0;
    keep.iter()
        .map(|&(a, b)| {
            let a = ms((a - start).max(0.0));
            (a, ms((b - start).max(a)))
        })
        .collect()
}

/// `select`/`aselect` expression for relative `keep` intervals.
fn select_expr(keep: &[(f64, f64)]) -> String {
    sum(keep
        .iter()
        .map(|&(a, b)| format!("between(t,{a:.3},{b:.3})"))
        .collect())
}

/// `terms` added as a balanced tree, `(a+b)+(c+d)`. ffmpeg's expression parser rejects a flat
/// chain of more than 100 terms ("Cannot allocate memory"); nesting keeps every level short.
fn sum(mut terms: Vec<String>) -> String {
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
fn close_gaps(keep: &[(f64, f64)]) -> String {
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
fn atempo(speed: f64) -> String {
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
fn head() -> Vec<String> {
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

/// Kept intervals closer than this share one input: decoding through the gap is cheaper than a
/// seek, which restarts decoding at the keyframe before the next interval.
const MERGE_GAP: f64 = 10.0;

/// Most inputs (separate seeks) one strip export opens, for a source path of `path_len`
/// characters. Each input repeats the path on the command line, which on Windows is 32 KiB.
fn max_inputs(path_len: usize) -> usize {
    (4000 / (path_len + 60)).clamp(1, 32)
}

/// Sorted `keep` intervals grouped into at most `max` runs (index ranges): gaps under
/// [`MERGE_GAP`] are always joined, then the smallest gaps until few enough remain.
fn spans(keep: &[(f64, f64)], max: usize) -> Vec<(usize, usize)> {
    let mut g: Vec<(usize, usize)> = (0..keep.len()).map(|i| (i, i + 1)).collect();
    let gap = |g: &[(usize, usize)], k: usize| keep[g[k + 1].0].0 - keep[g[k].1 - 1].1;
    loop {
        let k = (0..g.len().saturating_sub(1)).min_by(|&x, &y| gap(&g, x).total_cmp(&gap(&g, y)));
        match k {
            Some(k) if g.len() > max.max(1) || gap(&g, k) < MERGE_GAP => {
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
fn strip_args(job: &Job, keep: &[(f64, f64)], enc: &str, src: &Source, max: usize) -> Vec<String> {
    let ms = |t: f64| (t * 1000.0).round() / 1000.0;
    let audio = src.audio && !job.mute;
    let mut a = head();
    let mut graph = String::new();
    let mut pads = String::new();
    let runs = spans(keep, max);
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
        graph.push_str(&format!(
            "[{i}:v:0]{crop}select='{sel}',setpts='{}'[v{i}];",
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

fn run(job: &Job, tx: &Sender<Msg>, stop: &AtomicBool, ctx: &Context) -> Result<(), String> {
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

/// A temp file, deleted on drop.
struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Moves a `-filter_complex` graph longer than [`GRAPH_INLINE`] into a temp file. If the file
/// can't be written the graph stays inline.
fn spill_graph(a: &mut [String]) -> Option<TempFile> {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let i = a.iter().position(|s| s == "-filter_complex")?;
    if a[i + 1].len() <= GRAPH_INLINE {
        return None;
    }
    let n = N.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("glint-graph-{}-{n}.txt", std::process::id()));
    std::fs::write(&path, &a[i + 1]).ok()?;
    a[i] = graph_file_option().into();
    a[i + 1] = path.to_string_lossy().into_owned();
    Some(TempFile(path))
}

/// `-/filter_complex` reads a file in ffmpeg 7+; 8 dropped the older `-filter_complex_script`,
/// which distro ffmpeg 6 still needs.
fn graph_file_option() -> &'static str {
    static OPT: OnceLock<&str> = OnceLock::new();
    OPT.get_or_init(|| {
        let out = command("ffmpeg").arg("-version").output();
        let v = out
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default();
        if major_version(&v).is_some_and(|m| m < 7) {
            "-filter_complex_script"
        } else {
            "-/filter_complex"
        }
    })
}

/// Major version from `ffmpeg -version`; `None` for git builds like `N-118000-g…`.
fn major_version(v: &str) -> Option<u32> {
    let v = v.strip_prefix("ffmpeg version ")?;
    let v = v.strip_prefix('n').unwrap_or(v);
    v.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
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

/// Grab `n` evenly spaced `w`×`h` frames from `start..end`, sent as `(index, image)` in any order.
/// Each tile is its own input seek, so long files don't get decoded end to end. Tiles more than
/// [`KEYFRAME_SPAN`] apart take the nearest keyframe (no decoding past it); closer ones seek
/// exactly. With `crop` (normalized), tiles show only that part of the frame. Stops early once
/// `tx`'s receiver is dropped.
pub fn thumbnails(
    src: PathBuf,
    (start, end): (f64, f64),
    n: usize,
    [w, h]: [usize; 2],
    crop: Option<eframe::egui::Rect>,
    tx: Sender<(usize, ColorImage)>,
    ctx: Context,
) {
    let step = (end - start).max(0.001) / n.max(1) as f64;
    let coarse = step > KEYFRAME_SPAN;
    let next = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    for _ in 0..THUMB_JOBS.min(n) {
        let (src, tx, ctx, next) = (src.clone(), tx.clone(), ctx.clone(), next.clone());
        std::thread::spawn(move || {
            loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= n {
                    break;
                }
                let t = start + (i as f64 + 0.5) * step;
                let Some(img) = frame(&src, t, coarse, [w, h], crop) else {
                    continue;
                };
                if tx.send((i, img)).is_err() {
                    break;
                }
                ctx.request_repaint();
            }
        });
    }
}

/// Tiles further apart than this (seconds) use the nearest keyframe instead of an exact seek.
const KEYFRAME_SPAN: f64 = 2.0;
/// ffmpeg processes decoding filmstrip tiles at once.
const THUMB_JOBS: usize = 4;

/// One `w`×`h` RGBA frame at `t`, center-cropped to fill.
fn frame(
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
    let out = cmd
        .args(["-ss", &format!("{t:.3}"), "-i"])
        .arg(src)
        .args([
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
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    (out.stdout.len() >= w * h * 4)
        .then(|| ColorImage::from_rgba_unmultiplied([w, h], &out.stdout[..w * h * 4]))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIST: &str = " V....D h264_videotoolbox    VideoToolbox H.264 Encoder (codec h264)\n V....D libx264              libx264 H.264\n V....D hevc_videotoolbox    VideoToolbox H.265\n";

    /// A source with audio and no known bitrates: no caps.
    const ANY: Source = Source {
        audio: true,
        video_kbps: None,
        audio_kbps: None,
        strict: false,
    };

    #[test]
    fn source_from_ffmpeg_listing() {
        let s = parse_source(
            "Input #0, mov,mp4,m4a,3gp,3g2,mj2, from 'a.mp4':\n  Duration: 00:12:02.27, start: 0.000000, bitrate: 776 kb/s\n  Stream #0:0[0x1](und): Video: hevc (Main) (hvc1 / 0x31637668), yuv420p(tv, bt709, progressive), 1920x1080 [SAR 1:1 DAR 16:9], 770 kb/s, 60 fps, 60 tbr, 15360 tbn (default)\n  Stream #0:1[0x2](und): Audio: aac (LC) (mp4a / 0x6134706D), 48000 Hz, stereo, fltp, 64 kb/s (default)\n",
        );
        assert!(s.audio);
        assert_eq!((s.video_kbps, s.audio_kbps), (Some(770), Some(64)));
        // MKV streams often carry no bitrate: fall back to the file's.
        let s = parse_source(
            "  Duration: 00:00:10.00, start: 0.000000, bitrate: 2500 kb/s\n  Stream #0:0: Video: h264 (High), yuv420p, 1280x720, 30 fps\n",
        );
        assert!(!s.audio);
        assert_eq!((s.video_kbps, s.audio_kbps), (Some(2500), None));
    }

    #[test]
    fn reencodes_are_capped_at_the_source_bitrate() {
        let mut j = job(Some([640, 360, 2, 4]), "out.mp4");
        let src = Source {
            audio: true,
            video_kbps: Some(770),
            audio_kbps: Some(64),
            strict: false,
        };
        let a = args(&j, Some("hevc_videotoolbox"), &src).join(" ");
        assert!(a.contains("-q:v 65 -maxrate 770k -bufsize 1540k"), "{a}");
        j.keep = Some(vec![(1.5, 2.0)]);
        let a = args(&j, Some("libx264"), &src).join(" ");
        assert!(
            a.contains("-crf 20 -preset medium -maxrate 770k -bufsize 1540k"),
            "{a}"
        );
        // Re-encoded audio never above the source's.
        assert!(a.contains("-c:a aac -b:a 64k"), "{a}");
        let strict = Source {
            strict: true,
            ..src
        };
        let a = args(&j, Some("h264_amf"), &strict).join(" ");
        assert!(
            a.contains("-rc vbr_peak -b:v 770k -maxrate 770k -bufsize 1540k"),
            "{a}"
        );
    }

    fn job(crop: Option<[u32; 4]>, dst: &str) -> Job {
        Job {
            src: "in.mkv".into(),
            dst: dst.into(),
            start: 1.5,
            end: 4.0,
            crop,
            precise: false,
            codec: "hevc".into(),
            quality: 0,
            mute: false,
            speed: 1.0,
            keep: None,
        }
    }

    #[test]
    fn candidates_prefer_same_codec_then_h264() {
        assert_eq!(
            candidates("hevc", LIST),
            ["hevc_videotoolbox", "h264_videotoolbox", "libx264"]
        );
        assert_eq!(candidates("vp9", LIST), ["h264_videotoolbox", "libx264"]);
        assert!(candidates("h264", "").is_empty());
    }

    #[test]
    fn copy_args() {
        let a = args(&job(None, "out.mkv"), None, &ANY).join(" ");
        assert!(
            a.contains("-ss 1.500 -to 4.000 -i in.mkv -map 0 -dn -c copy"),
            "{a}"
        );
        assert!(a.ends_with(" out.mkv"));
        let mut j = job(None, "out.mp4");
        j.mute = true;
        let a = args(&j, None, &ANY).join(" ");
        assert!(
            a.ends_with("-an -map -0:t -c:s mov_text -tag:v hvc1 out.mp4"),
            "{a}"
        );
    }

    #[test]
    fn crop_args() {
        let a = args(
            &job(Some([640, 360, 2, 4]), "out.mp4"),
            Some("hevc_videotoolbox"),
            &ANY,
        )
        .join(" ");
        assert!(
            a.contains("-c:v hevc_videotoolbox -q:v 65 -vf crop=640:360:2:4 -map -0:t"),
            "{a}"
        );
        assert!(a.ends_with("-tag:v hvc1 out.mp4"), "{a}");
        let mut j = job(None, "out.mkv");
        j.quality = 2;
        let a = args(&j, Some("libx264"), &ANY).join(" ");
        assert!(
            a.contains("-c:v libx264 -crf 28 -preset medium out.mkv"),
            "{a}"
        );
        let a = args(
            &job(Some([640, 360, 2, 4]), "out.mkv"),
            Some("hevc_videotoolbox"),
            &ANY,
        )
        .join(" ");
        assert!(!a.contains("hvc1"));
    }

    #[test]
    fn compress_prefers_software_encoders() {
        assert_eq!(
            encoder_order("h264", LIST, false),
            ["h264_videotoolbox", "libx264"]
        );
        assert_eq!(
            encoder_order("h264", LIST, true),
            ["libx264", "h264_videotoolbox"]
        );
        assert_eq!(
            encoder_order("hevc", LIST, true),
            ["libx264", "hevc_videotoolbox", "h264_videotoolbox"]
        );
    }

    #[test]
    fn compress_args() {
        let mut j = job(None, "out.mp4");
        j.quality = COMPRESS;
        let a = args(&j, Some("libx264"), &ANY).join(" ");
        assert!(
            a.contains("-c:v libx264 -crf 32 -preset slow -c:a aac -b:a 96k"),
            "{a}"
        );
        let a = args(&j, Some("libx265"), &ANY).join(" ");
        assert!(a.contains("-c:v libx265 -crf 34 -preset slow"), "{a}");
        let a = args(&j, Some("h264_videotoolbox"), &ANY).join(" ");
        assert!(a.contains("-q:v 28"), "{a}");

        j.mute = true;
        let a = args(&j, Some("libx264"), &ANY).join(" ");
        assert!(!a.contains("-c:a aac"), "{a}");
        assert!(a.ends_with("-an -map -0:t -c:s mov_text out.mp4"), "{a}");
    }

    #[test]
    fn kept_duration_sums_intervals() {
        let mut j = job(None, "out.mp4");
        assert_eq!(j.kept_duration(), 2.5);
        j.keep = Some(vec![(1.5, 2.0), (3.0, 4.0)]);
        assert_eq!(j.kept_duration(), 1.5);
    }

    #[test]
    fn strip_args() {
        let mut j = job(None, "out.mp4");
        j.keep = Some(vec![(1.5, 2.0), (3.0, 4.0)]);
        let a = args(&j, Some("libx264"), &ANY).join(" ");
        // Close intervals share one input, seeked to the first one.
        let sel = "between(t,0.000,0.500)+between(t,1.500,2.500)";
        assert!(
            a.contains("-ss 1.500 -to 4.000 -i in.mkv -filter_complex"),
            "{a}"
        );
        assert!(
            a.contains(&format!(
                "[0:v:0]select='{sel}',setpts='(T-0.000-gte(T,1.500)*1.000)/TB'[v0];"
            )),
            "{a}"
        );
        assert!(
            a.contains(&format!("[0:a:0]aselect='{sel}',asetpts=N/SR/TB[a0];")),
            "{a}"
        );
        assert!(
            a.contains("[v0][a0]concat=n=1:v=1:a=1[v][a] -map [v] -map [a]"),
            "{a}"
        );
        assert!(a.contains("-c:a aac -b:a 128k"), "{a}");
        assert!(!a.contains("-map 0:s?"), "{a}");
        assert!(a.contains("-map_chapters -1"), "{a}");

        j.quality = COMPRESS;
        let a = args(&j, Some("libx264"), &ANY).join(" ");
        assert!(a.contains("-c:a aac -b:a 96k"), "{a}");

        j.mute = true;
        let a = args(&j, Some("libx264"), &ANY).join(" ");
        assert!(!a.contains("aselect"), "{a}");
        assert!(a.ends_with("-an out.mp4"), "{a}");

        // No audio stream: video only, nothing that would need one.
        j.mute = false;
        let a = args(&j, Some("libx264"), &Source::default()).join(" ");
        assert!(a.contains("concat=n=1:v=1:a=0[v] -map [v] -c:v"), "{a}");
        assert!(!a.contains("aac"), "{a}");
    }

    #[test]
    fn far_apart_intervals_get_their_own_seek() {
        let mut j = job(Some([640, 360, 2, 4]), "out.mp4");
        j.start = 0.0;
        j.end = 100.0;
        j.keep = Some(vec![(1.0, 2.0), (5.0, 6.0), (50.0, 51.0), (90.0, 91.5)]);
        let a = args(&j, Some("libx264"), &ANY).join(" ");
        assert!(a.contains("-ss 1.000 -to 6.000 -i in.mkv"), "{a}");
        assert!(a.contains("-ss 50.000 -to 51.000 -i in.mkv"), "{a}");
        assert!(a.contains("-ss 90.000 -to 91.500 -i in.mkv"), "{a}");
        assert!(
            a.contains("[2:v:0]crop=640:360:2:4,select='between(t,0.000,1.500)'"),
            "{a}"
        );
        assert!(
            a.contains("[v0][a0][v1][a1][v2][a2]concat=n=3:v=1:a=1[v][a]"),
            "{a}"
        );
    }

    #[test]
    fn spans_merge_close_gaps_then_the_smallest() {
        let keep = [
            (0.0, 1.0),
            (3.0, 4.0),
            (30.0, 31.0),
            (60.0, 61.0),
            (75.0, 76.0),
        ];
        assert_eq!(spans(&keep, 32), [(0, 2), (2, 3), (3, 4), (4, 5)]);
        // Two inputs at most: join the 14 s gap, then the 26 s one.
        assert_eq!(spans(&keep, 2), [(0, 3), (3, 5)]);
        assert_eq!(spans(&keep, 1), [(0, 5)]);
        assert_eq!(max_inputs(260), 12);
        assert_eq!(max_inputs(20), 32);
    }

    #[test]
    fn long_sums_nest() {
        let t = |n: usize| (0..n).map(|i| format!("x{i}")).collect::<Vec<_>>();
        assert_eq!(sum(t(1)), "x0");
        assert_eq!(sum(t(2)), "x0+x1");
        assert_eq!(sum(t(5)), "((x0+x1)+(x2+x3))+x4");
        assert_eq!(
            close_gaps(&[(1.0, 2.0), (3.0, 4.0), (5.0, 6.0)]),
            "(T-1.000-(gte(T,3.000)*1.000+gte(T,5.000)*1.000))/TB"
        );
    }

    /// Needs `ffmpeg` on PATH. Thousands of intervals must still parse: ffmpeg rejected the flat
    /// chains past 100 terms.
    #[test]
    fn ffmpeg_parses_long_expressions() {
        let keep: Vec<(f64, f64)> = (0..3000)
            .map(|i| (i as f64 * 0.003, i as f64 * 0.003 + 0.001))
            .collect();
        let mut a: Vec<String> = ["-v", "error", "-f", "lavfi", "-i", "nullsrc=s=16x16:d=1"]
            .map(String::from)
            .into();
        a.push("-filter_complex".into());
        a.push(format!(
            "[0:v]select='{}',setpts='{}'[v]",
            select_expr(&keep),
            close_gaps(&keep)
        ));
        a.extend(["-map", "[v]", "-f", "null", "-"].map(String::from));
        let file = spill_graph(&mut a).expect("graph long enough to spill");
        assert!(file.0.exists() && a.contains(&graph_file_option().to_string()));
        let out = Command::new("ffmpeg").args(&a).output().unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[test]
    fn major_version_parses_release_and_git_builds() {
        assert_eq!(
            major_version("ffmpeg version 6.1.1-3ubuntu5 Copyright"),
            Some(6)
        );
        assert_eq!(major_version("ffmpeg version n7.1 Copyright"), Some(7));
        assert_eq!(major_version("ffmpeg version 9.0.2 Copyright"), Some(9));
        assert_eq!(
            major_version("ffmpeg version N-118000-gabc Copyright"),
            None
        );
        assert_eq!(major_version(""), None);
    }

    #[test]
    fn error_tail_cuts_long_lines() {
        let long = format!(
            "[select] Error while parsing expression '{}'",
            "x+".repeat(500)
        );
        let e = error_tail(&format!("{long}\nError initializing filters\n\n"));
        assert!(e.lines().all(|l| l.chars().count() <= 161), "{e}");
        assert!(e.ends_with("Error initializing filters"), "{e}");
    }

    #[test]
    fn many_segments_fit_windows_command_line() {
        let mut j = job(None, "out.mp4");
        j.src = "s".repeat(260).into();
        j.dst = "x".repeat(260).into();
        j.start = 0.0;
        j.end = 99_999.0;
        // Long timestamps and short gaps: the widest expressions a strip export can produce.
        j.keep = Some(
            (0..3000)
                .map(|i| {
                    let a = 10_000.0 + i as f64 * 3.123;
                    (a, a + 1.567)
                })
                .collect(),
        );
        j.speed = 2.0;
        let mut a = args(&j, Some("h264_videotoolbox"), &ANY);
        let _graph = spill_graph(&mut a);
        let len: usize = a.iter().map(|a| a.len() + 3).sum();
        assert!(len < 32_000, "command line is {len} characters");
    }

    #[test]
    fn atempo_chain_stays_in_range() {
        assert_eq!(atempo(1.5), "atempo=1.500000");
        assert_eq!(atempo(2.0), "atempo=2.000000");
        assert_eq!(atempo(4.0), "atempo=2.000000,atempo=2.000000");
        assert_eq!(atempo(0.25), "atempo=0.500000,atempo=0.500000");
    }

    #[test]
    fn speed_args_retime_video_and_audio() {
        let mut j = job(None, "out.mp4");
        j.speed = 2.0;
        let a = args(&j, Some("libx264"), &ANY).join(" ");
        assert!(a.contains("-vf setpts=PTS/2.000000"), "{a}");
        assert!(a.contains("-af atempo=2.000000"), "{a}");
        assert!(a.contains("-c:a aac -b:a 128k"), "{a}");
        assert!(!a.contains("-map 0:s?"), "{a}");
        assert!(a.contains("-map_chapters -1"), "{a}");

        j.speed = 0.25;
        let a = args(&j, Some("libx264"), &ANY).join(" ");
        assert!(a.contains("-af atempo=0.500000,atempo=0.500000"), "{a}");

        j.speed = 2.0;
        j.mute = true;
        let a = args(&j, Some("libx264"), &ANY).join(" ");
        assert!(!a.contains("-af"), "{a}");
        assert!(a.ends_with("-an out.mp4"), "{a}");
    }

    #[test]
    fn speed_combines_with_strip() {
        let mut j = job(None, "out.mp4");
        j.keep = Some(vec![(1.5, 2.0), (3.0, 4.0)]);
        j.speed = 2.0;
        let a = args(&j, Some("libx264"), &ANY).join(" ");
        assert!(
            a.contains(
                "concat=n=1:v=1:a=1[v][a];[v]setpts=PTS/2.000000[vs];[a]atempo=2.000000[as]"
            ),
            "{a}"
        );
        assert!(a.contains("-map [vs] -map [as]"), "{a}");
    }

    #[test]
    fn speed_changes_output_duration() {
        let mut j = job(None, "out.mp4");
        assert_eq!(j.output_duration(), 2.5);
        j.speed = 2.0;
        assert_eq!(j.output_duration(), 1.25);
        j.speed = 0.5;
        assert_eq!(j.output_duration(), 5.0);
    }

    /// Needs `ffmpeg` and `ffprobe` on PATH.
    #[test]
    fn export_round_trip() {
        let dir = std::env::temp_dir().join(format!("glint-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("src.mkv");
        let ok = Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=640x360:rate=25:duration=4",
            ])
            .args([
                "-f",
                "lavfi",
                "-i",
                "sine=duration=4",
                "-c:v",
                "libx264",
                "-g",
                "25",
                "-c:a",
                "aac",
                "-shortest",
            ])
            .arg(&src)
            .status()
            .unwrap();
        assert!(ok.success());

        // The filmstrip refreshes one batch for the visible range.
        let (tx, rx) = channel();
        thumbnails(
            src.clone(),
            (1.0, 2.0),
            4,
            [96, 72],
            None,
            tx,
            Context::default(),
        );
        let mut got = std::collections::BTreeSet::new();
        while got.len() < 4 {
            match rx.recv_timeout(std::time::Duration::from_secs(30)) {
                Ok((i, _)) => assert!(got.insert(i), "tile {i} sent twice"),
                Err(e) => panic!("thumbnails: {e}"),
            }
        }
        assert_eq!(got.into_iter().collect::<Vec<_>>(), [0, 1, 2, 3]);
        // Far apart tiles take the keyframe path.
        let (tx, rx) = channel();
        thumbnails(
            src.clone(),
            (0.0, 4.0),
            1,
            [96, 72],
            None,
            tx,
            Context::default(),
        );
        let (_, whole) = rx
            .recv_timeout(std::time::Duration::from_secs(30))
            .expect("keyframe thumbnail");
        // A crop shows only its part of the frame.
        let (tx, rx) = channel();
        let corner = eframe::egui::Rect::from_min_max(
            eframe::egui::pos2(0.0, 0.0),
            eframe::egui::pos2(0.3, 0.3),
        );
        thumbnails(
            src.clone(),
            (0.0, 4.0),
            1,
            [96, 72],
            Some(corner),
            tx,
            Context::default(),
        );
        let (_, cropped) = rx
            .recv_timeout(std::time::Duration::from_secs(30))
            .expect("cropped thumbnail");
        assert_eq!(cropped.size, [96, 72]);
        assert_ne!(cropped.pixels, whole.pixels);

        let wait = |run: Running| -> Result<u64, String> {
            loop {
                match run
                    .rx
                    .recv_timeout(std::time::Duration::from_secs(120))
                    .expect("export timed out")
                {
                    Msg::Done(r) => break r,
                    Msg::Progress(_) => {}
                }
            }
        };
        let probe = |path: &std::path::Path, entries: &str| -> String {
            let out = Command::new("ffprobe")
                .args([
                    "-v",
                    "error",
                    "-select_streams",
                    "v:0",
                    "-show_entries",
                    entries,
                    "-of",
                    "csv=p=0",
                ])
                .arg(path)
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        for (crop, name, size, want) in [
            // Stream copy starts at a keyframe before IN, so allow up to one GOP (1 s) extra.
            (None, "copy.mkv", "640,360", 1.9..3.4),
            (Some([320, 180, 10, 20]), "crop.mp4", "320,180", 1.85..2.15),
        ] {
            let dst = dir.join(name);
            let job = Job {
                src: src.clone(),
                dst: dst.clone(),
                start: 1.0,
                end: 3.0,
                crop,
                precise: false,
                codec: "h264".into(),
                quality: 0,
                mute: false,
                speed: 1.0,
                keep: None,
            };
            wait(export(job, Context::default()))
                .map(|_| ())
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(probe(&dst, "stream=width,height"), size, "{name}");
            let dur: f64 = probe(&dst, "format=duration").parse().unwrap();
            assert!(want.contains(&dur), "{name}: duration {dur}");
        }

        // Compress: keep the whole clip, so the output is comparable to the source.
        let src_len = std::fs::metadata(&src).unwrap().len();
        let dst = dir.join("compress.mp4");
        let job = Job {
            src: src.clone(),
            dst: dst.clone(),
            start: 0.0,
            end: 4.0,
            crop: None,
            precise: false,
            codec: "h264".into(),
            quality: COMPRESS,
            mute: false,
            speed: 1.0,
            keep: None,
        };
        let out_len = wait(export(job, Context::default())).unwrap();
        assert!(out_len < src_len, "compress: {out_len} >= {src_len}");
        assert_eq!(out_len, std::fs::metadata(&dst).unwrap().len());

        // Strip within a trim that starts mid-GOP (1.5 s), keeping 1.5..2 s and 2.5..3 s. The
        // filter times are relative to the `-ss` point, so this checks that they line up.
        let dst = dir.join("strip.mp4");
        let job = Job {
            src: src.clone(),
            dst: dst.clone(),
            start: 1.5,
            end: 3.0,
            crop: None,
            precise: false,
            codec: "h264".into(),
            quality: 0,
            mute: false,
            speed: 1.0,
            keep: Some(vec![(1.5, 2.0), (2.5, 3.0)]),
        };
        wait(export(job, Context::default())).unwrap();
        let dur: f64 = probe(&dst, "format=duration").parse().unwrap();
        assert!((0.9..=1.1).contains(&dur), "strip duration {dur}");
        let audio = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-select_streams",
                "a",
                "-show_entries",
                "stream=codec_name",
                "-of",
                "csv=p=0",
            ])
            .arg(&dst)
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&audio.stdout).trim(), "aac");
        let adur = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-select_streams",
                "a:0",
                "-show_entries",
                "stream=duration",
                "-of",
                "csv=p=0",
            ])
            .arg(&dst)
            .output()
            .unwrap();
        let adur: f64 = String::from_utf8_lossy(&adur.stdout)
            .trim()
            .parse()
            .unwrap();
        assert!((0.9..=1.1).contains(&adur), "strip audio duration {adur}");

        // Variable frame rate, like a screen recording: no frames at all between 1.5 s and 2.5 s.
        // Keeping 1..3 s must keep that hold as time, or the video ends long before the audio.
        let vfr = dir.join("vfr.mp4");
        let ok = Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=320x180:rate=25:duration=4",
                "-f",
                "lavfi",
                "-i",
                "sine=duration=4",
                "-vf",
                "select='not(between(t,1.5,2.5))'",
                "-fps_mode",
                "passthrough",
                "-c:v",
                "libx264",
                "-c:a",
                "aac",
                "-shortest",
            ])
            .arg(&vfr)
            .status()
            .unwrap();
        assert!(ok.success());
        let dst = dir.join("strip-vfr.mp4");
        let job = Job {
            src: vfr,
            dst: dst.clone(),
            start: 0.0,
            end: 4.0,
            crop: None,
            precise: false,
            codec: "h264".into(),
            quality: 0,
            mute: false,
            speed: 1.0,
            keep: Some(vec![(0.5, 1.0), (1.2, 3.0)]),
        };
        wait(export(job, Context::default())).unwrap();
        let vdur: f64 = probe(&dst, "stream=duration").parse().unwrap();
        assert!(
            (2.2..=2.4).contains(&vdur),
            "vfr strip video duration {vdur}"
        );

        // Retiming: 4 s at 2× should land near 2 s with audio still present.
        let dst = dir.join("fast.mp4");
        let job = Job {
            src: src.clone(),
            dst: dst.clone(),
            start: 0.0,
            end: 4.0,
            crop: None,
            precise: false,
            codec: "h264".into(),
            quality: 0,
            mute: false,
            speed: 2.0,
            keep: None,
        };
        wait(export(job, Context::default())).unwrap();
        let dur: f64 = probe(&dst, "format=duration").parse().unwrap();
        assert!((1.8..=2.2).contains(&dur), "speed duration {dur}");
        let audio = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-select_streams",
                "a",
                "-show_entries",
                "stream=codec_name",
                "-of",
                "csv=p=0",
            ])
            .arg(&dst)
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&audio.stdout).trim(), "aac");

        // Slow-down: 4 s at 0.5× should land near 8 s.
        let dst = dir.join("slow.mp4");
        let job = Job {
            src: src.clone(),
            dst: dst.clone(),
            start: 0.0,
            end: 4.0,
            crop: None,
            precise: false,
            codec: "h264".into(),
            quality: 0,
            mute: false,
            speed: 0.5,
            keep: None,
        };
        wait(export(job, Context::default())).unwrap();
        let dur: f64 = probe(&dst, "format=duration").parse().unwrap();
        assert!((7.8..=8.2).contains(&dur), "slow duration {dur}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Real-file check, off by default:
    /// `GLINT_REAL=video.mp4 [GLINT_REAL_CROP=x0,y0,x1,y1] [GLINT_REAL_PAD=0.5] [GLINT_REAL_OUT=dir]
    /// cargo test real_file -- --ignored --nocapture`
    /// Scans (with the crop, normalized), then exports the strip both with one seek (the old way:
    /// everything between the first and last kept frame is decoded) and with a seek per run, and
    /// checks both outputs hold the same frames. Needs `ffmpeg` and `ffprobe` on PATH.
    #[test]
    #[ignore]
    fn real_file() {
        use std::time::Instant;
        let Some(src) = std::env::var_os("GLINT_REAL").map(PathBuf::from) else {
            return;
        };
        let out = std::env::var_os("GLINT_REAL_OUT").map_or_else(std::env::temp_dir, PathBuf::from);
        let crop = std::env::var("GLINT_REAL_CROP").ok().map(|c| {
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
        };
        let job = crate::strip::scan(src.clone(), dur, area, Context::default());
        let scan = loop {
            if let crate::strip::Msg::Done(r) = job.rx.recv().unwrap() {
                break r.unwrap();
            }
        };
        let scan_s = t.elapsed().as_secs_f64();
        let pad = std::env::var("GLINT_REAL_PAD").map_or(0.5, |p| p.parse().unwrap());
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
            let dst = out.join(format!("glint-real-{name}.mp4"));
            let job = Job {
                dst: dst.clone(),
                ..job.clone()
            };
            let t = Instant::now();
            let mut a = super::strip_args(&job, &keep, enc, &source, max);
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
}
