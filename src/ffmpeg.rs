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
/// Most kept intervals one strip export takes. Each costs ~90 characters of filter expressions
/// (select, aselect, setpts), which share Windows' 32 KiB command line with the rest of the args.
pub const MAX_SEGMENTS: usize = 300;
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

/// Encoder settings for `level`, an index into [`QUALITIES`].
fn quality(enc: &str, level: usize) -> Vec<String> {
    let level = level.min(COMPRESS);
    let q = |v: [u32; 4]| v[level];
    // Compression is worth the slower preset: it spends encoding time to save bytes.
    let preset = if level == COMPRESS { "slow" } else { "medium" };
    match enc.rsplit('_').next().unwrap_or(enc) {
        "videotoolbox" => format!("-q:v {}", q([65, 50, 35, 28])),
        "nvenc" => format!("-rc vbr -cq {} -b:v 0", q([23, 28, 33, 38])),
        "qsv" => format!("-global_quality {}", q([23, 28, 33, 38])),
        "amf" => format!("-rc cqp -qp_i {0} -qp_p {0}", q([22, 28, 34, 38])),
        "libx264" => format!("-crf {} -preset {preset}", q([20, 24, 28, 32])),
        "libx265" => format!("-crf {} -preset {preset}", q([22, 26, 30, 34])),
        // shortcut: fixed bitrate for encoders without a quality mode, revisit if output looks soft.
        _ => format!("-b:v {}M", q([12, 6, 3, 1])),
    }
    .split(' ')
    .map(String::from)
    .collect()
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
    keep.iter()
        .map(|&(a, b)| format!("between(t,{a:.3},{b:.3})"))
        .collect::<Vec<_>>()
        .join("+")
}

/// `setpts` expression that closes the cut gaps by shifting each kept interval back by the time
/// removed before it. Renumbering frames (`N/FRAME_RATE`) would squeeze variable-frame-rate
/// video, such as screen recordings, and drift from the audio.
fn close_gaps(keep: &[(f64, f64)]) -> String {
    let mut e = format!("T-{:.3}", keep.first().map_or(0.0, |k| k.0));
    for w in keep.windows(2) {
        e.push_str(&format!("-gte(T,{:.3})*{:.3}", w[1].0, w[1].0 - w[0].1));
    }
    format!("({e})/TB")
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

/// ffmpeg arguments for `job`. `enc = None` means lossless stream copy.
pub fn args(job: &Job, enc: Option<&str>) -> Vec<String> {
    let mut a: Vec<String> = [
        "-hide_banner",
        "-nostdin",
        "-y",
        "-v",
        "error",
        "-progress",
        "pipe:1",
        "-nostats",
    ]
    .map(String::from)
    .into();
    a.extend([
        "-ss".into(),
        format!("{:.3}", job.start),
        "-to".into(),
        format!("{:.3}", job.end),
    ]);
    a.extend(["-i".into(), job.src.to_string_lossy().into_owned()]);
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    let retime = job.retime();
    // Selected intervals are relative to the seek point; `-ss` resets input timestamps to zero.
    let rel = job.keep.as_ref().map(|keep| relative(keep, job.start));
    let strip = rel.as_deref().map(select_expr);
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
            if strip.is_none() && !retime {
                a.extend(s(&["-map", "0:s?"]));
            }
            a.extend(s(&["-c", "copy", "-c:v", enc]));
            a.extend(quality(enc, job.quality));
            let mut filters = Vec::new();
            if let Some([w, h, x, y]) = job.crop {
                filters.push(format!("crop={w}:{h}:{x}:{y}"));
            }
            if let (Some(sel), Some(rel)) = (&strip, &rel) {
                filters.push(format!("select='{sel}'"));
                filters.push(format!("setpts='{}'", close_gaps(rel)));
            }
            if retime {
                filters.push(format!("setpts=PTS/{:.6}", job.speed));
            }
            if !filters.is_empty() {
                a.extend(["-vf".into(), filters.join(",")]);
            }
            let mut audio_filters = Vec::new();
            if let Some(sel) = &strip
                && !job.mute
            {
                audio_filters.push(format!("aselect='{sel}'"));
                audio_filters.push("asetpts=N/SR/TB".into());
            }
            if retime && !job.mute {
                audio_filters.push(atempo(job.speed));
            }
            if !audio_filters.is_empty() {
                // Filtering audio means re-encoding it. 96k when compressing, otherwise a safe 128k.
                a.extend(["-af".into(), audio_filters.join(",")]);
                let bitrate = if job.quality == COMPRESS {
                    "96k"
                } else {
                    "128k"
                };
                a.extend(s(&["-c:a", "aac", "-b:a", bitrate]));
            } else if job.quality == COMPRESS && !job.mute {
                // Copying audio would leave its share of the file untouched, so compress it too.
                a.extend(s(&["-c:a", "aac", "-b:a", "96k"]));
            }
            if strip.is_some() || retime {
                a.extend(s(&["-map_chapters", "-1"]));
            }
        }
    }
    if job.mute {
        a.push("-an".into());
    }
    let mkv = |p: &Path| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("mkv"));
    // Subtitles and attachments don't carry across containers: MP4/MOV only take mov_text, MKV can't.
    // A stripped file drops them instead: their timestamps would no longer line up.
    if strip.is_none() && !retime {
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
    if let Some(keep) = &job.keep {
        if keep.is_empty() || job.kept_duration() <= 0.0 {
            return Err(t!("status.nothing_left").into());
        }
        if keep.len() > MAX_SEGMENTS {
            return Err(t!("status.too_many").into());
        }
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
    let mut err = String::new();
    for enc in encs {
        let mut child = command("ffmpeg")
            .args(args(job, enc))
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
            return Ok(());
        }
        err = errs.join().unwrap_or_default();
    }
    let tail: Vec<&str> = err.lines().rev().take(3).collect();
    let msg = tail.into_iter().rev().collect::<Vec<_>>().join("\n");
    Err(if msg.is_empty() {
        t!("error.ffmpeg").into()
    } else {
        msg
    })
}

/// Grab `n` evenly spaced `w`×`h` frames from `start..end` in one ffmpeg run.
/// Stops early once `tx`'s receiver is dropped.
pub fn thumbnails(
    src: PathBuf,
    start: f64,
    end: f64,
    n: usize,
    [w, h]: [usize; 2],
    tx: Sender<(usize, ColorImage)>,
    ctx: Context,
) {
    let span = (end - start).max(0.001);
    let fps = n as f64 / span;
    std::thread::spawn(move || {
        let Ok(mut child) = command("ffmpeg")
            .args([
                "-v",
                "error",
                "-ss",
                &format!("{start:.3}"),
                "-to",
                &format!("{end:.3}"),
                "-i",
            ])
            .arg(&src)
            .args([
                "-frames:v",
                &n.to_string(),
                "-vf",
                &format!(
                    "fps={fps:.6},scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h}"
                ),
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgba",
                "-",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        else {
            return;
        };
        let Some(stdout) = child.stdout.take() else {
            return;
        };
        let mut reader = BufReader::new(stdout);
        let mut buf = vec![0u8; w * h * 4];
        for i in 0..n {
            if reader.read_exact(&mut buf).is_err() {
                break;
            }
            if tx
                .send((i, ColorImage::from_rgba_unmultiplied([w, h], &buf)))
                .is_err()
            {
                let _ = child.kill();
                break;
            }
            ctx.request_repaint();
        }
        let _ = child.wait();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIST: &str = " V....D h264_videotoolbox    VideoToolbox H.264 Encoder (codec h264)\n V....D libx264              libx264 H.264\n V....D hevc_videotoolbox    VideoToolbox H.265\n";

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
        let a = args(&job(None, "out.mkv"), None).join(" ");
        assert!(
            a.contains("-ss 1.500 -to 4.000 -i in.mkv -map 0 -dn -c copy"),
            "{a}"
        );
        assert!(a.ends_with(" out.mkv"));
        let mut j = job(None, "out.mp4");
        j.mute = true;
        let a = args(&j, None).join(" ");
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
        )
        .join(" ");
        assert!(
            a.contains("-c:v hevc_videotoolbox -q:v 65 -vf crop=640:360:2:4 -map -0:t"),
            "{a}"
        );
        assert!(a.ends_with("-tag:v hvc1 out.mp4"), "{a}");
        let mut j = job(None, "out.mkv");
        j.quality = 2;
        let a = args(&j, Some("libx264")).join(" ");
        assert!(
            a.contains("-c:v libx264 -crf 28 -preset medium out.mkv"),
            "{a}"
        );
        let a = args(
            &job(Some([640, 360, 2, 4]), "out.mkv"),
            Some("hevc_videotoolbox"),
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
        let a = args(&j, Some("libx264")).join(" ");
        assert!(
            a.contains("-c:v libx264 -crf 32 -preset slow -c:a aac -b:a 96k"),
            "{a}"
        );
        let a = args(&j, Some("libx265")).join(" ");
        assert!(a.contains("-c:v libx265 -crf 34 -preset slow"), "{a}");
        let a = args(&j, Some("h264_videotoolbox")).join(" ");
        assert!(a.contains("-q:v 28"), "{a}");

        j.mute = true;
        let a = args(&j, Some("libx264")).join(" ");
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
        let a = args(&j, Some("libx264")).join(" ");
        let sel = "between(t,0.000,0.500)+between(t,1.500,2.500)";
        assert!(
            a.contains(&format!(
                "-vf select='{sel}',setpts='(T-0.000-gte(T,1.500)*1.000)/TB'"
            )),
            "{a}"
        );
        assert!(
            a.contains(&format!("-af aselect='{sel}',asetpts=N/SR/TB")),
            "{a}"
        );
        assert!(a.contains("-c:a aac -b:a 128k"), "{a}");
        assert!(!a.contains("-map 0:s?"), "{a}");
        assert!(a.contains("-map_chapters -1"), "{a}");

        j.quality = COMPRESS;
        let a = args(&j, Some("libx264")).join(" ");
        assert!(a.contains("-c:a aac -b:a 96k"), "{a}");

        j.mute = true;
        let a = args(&j, Some("libx264")).join(" ");
        assert!(!a.contains("-af"), "{a}");
        assert!(a.ends_with("-an out.mp4"), "{a}");
    }

    #[test]
    fn max_segments_fit_windows_command_line() {
        let mut j = job(None, "out.mp4");
        j.src = "s".repeat(260).into();
        j.dst = "x".repeat(260).into();
        j.start = 0.0;
        j.end = 99_999.0;
        // Long timestamps and short gaps: the widest expressions a strip export can produce.
        j.keep = Some(
            (0..MAX_SEGMENTS)
                .map(|i| {
                    let a = 10_000.0 + i as f64 * 3.123;
                    (a, a + 1.567)
                })
                .collect(),
        );
        j.speed = 2.0;
        let len: usize = args(&j, Some("h264_videotoolbox"))
            .iter()
            .map(|a| a.len() + 3)
            .sum();
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
        let a = args(&j, Some("libx264")).join(" ");
        assert!(a.contains("-vf setpts=PTS/2.000000"), "{a}");
        assert!(a.contains("-af atempo=2.000000"), "{a}");
        assert!(a.contains("-c:a aac -b:a 128k"), "{a}");
        assert!(!a.contains("-map 0:s?"), "{a}");
        assert!(a.contains("-map_chapters -1"), "{a}");

        j.speed = 0.25;
        let a = args(&j, Some("libx264")).join(" ");
        assert!(a.contains("-af atempo=0.500000,atempo=0.500000"), "{a}");

        j.speed = 2.0;
        j.mute = true;
        let a = args(&j, Some("libx264")).join(" ");
        assert!(!a.contains("-af"), "{a}");
        assert!(a.ends_with("-an out.mp4"), "{a}");
    }

    #[test]
    fn speed_combines_with_strip() {
        let mut j = job(None, "out.mp4");
        j.keep = Some(vec![(1.5, 2.0), (3.0, 4.0)]);
        j.speed = 2.0;
        let a = args(&j, Some("libx264")).join(" ");
        let sel = "between(t,0.000,0.500)+between(t,1.500,2.500)";
        assert!(
            a.contains(&format!(
                "-vf select='{sel}',setpts='(T-0.000-gte(T,1.500)*1.000)/TB',setpts=PTS/2.000000"
            )),
            "{a}"
        );
        assert!(
            a.contains(&format!(
                "-af aselect='{sel}',asetpts=N/SR/TB,atempo=2.000000"
            )),
            "{a}"
        );
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
        thumbnails(src.clone(), 1.0, 2.0, 4, [96, 72], tx, Context::default());
        let mut got = 0;
        while got < 4 {
            match rx.recv_timeout(std::time::Duration::from_secs(30)) {
                Ok((i, _)) => {
                    assert_eq!(i, got);
                    got += 1;
                }
                Err(e) => panic!("thumbnails: {e}"),
            }
        }

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
}
