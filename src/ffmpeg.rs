//! Everything that shells out to the ffmpeg CLI: export and filmstrip thumbnails.

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

/// `ffmpeg` bundled with QPE, else the one on PATH.
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
    /// Keep only these absolute source-time intervals, concatenated. Forces a re-encode.
    pub keep: Option<Vec<(f64, f64)>>,
}

impl Job {
    /// Output duration: the trim length, or the sum of the kept intervals.
    pub fn kept_duration(&self) -> f64 {
        self.keep.as_ref().map_or(self.end - self.start, |keep| {
            keep.iter().map(|&(a, b)| (b - a).max(0.0)).sum()
        })
    }
}

/// Export containers, by file extension.
pub const FORMATS: &[&str] = &["mp4", "mov", "mkv"];
/// Quality levels, best first. Only the best one may stream-copy.
pub const QUALITIES: &[&str] = &["High", "Medium", "Low", "Compress"];
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

/// `select`/`aselect` expression for `keep` intervals, relative to the input seek point `start`.
fn select_expr(keep: &[(f64, f64)], start: f64) -> String {
    keep.iter()
        .map(|&(a, b)| {
            let a = (a - start).max(0.0);
            let b = (b - start).max(a);
            format!("between(t,{a:.3},{b:.3})")
        })
        .collect::<Vec<_>>()
        .join("+")
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
    // Selected intervals are relative to the seek point; `-ss` resets input timestamps to zero.
    let strip = job.keep.as_ref().map(|keep| select_expr(keep, job.start));
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
            if strip.is_none() {
                a.extend(s(&["-map", "0:s?"]));
            }
            a.extend(s(&["-c", "copy", "-c:v", enc]));
            a.extend(quality(enc, job.quality));
            let mut filters = Vec::new();
            if let Some([w, h, x, y]) = job.crop {
                filters.push(format!("crop={w}:{h}:{x}:{y}"));
            }
            if let Some(sel) = &strip {
                filters.push(format!("select='{sel}'"));
                filters.push("setpts=N/FRAME_RATE/TB".into());
            }
            if !filters.is_empty() {
                a.extend(["-vf".into(), filters.join(",")]);
            }
            if let Some(sel) = &strip
                && !job.mute
            {
                // Filtering audio means re-encoding it. 96k when compressing, otherwise a safe 128k.
                a.extend(["-af".into(), format!("aselect='{sel}',asetpts=N/SR/TB")]);
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
            if strip.is_some() {
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
    if strip.is_none() {
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
    if let Some(keep) = &job.keep {
        if keep.is_empty() || job.kept_duration() <= 0.0 {
            return Err("Nothing left after stripping inactive frames.".into());
        }
        // The select expression shares Windows' 32 KiB command line with the rest of the args.
        if keep.len() > 500 {
            return Err(
                "Too many separate active segments for one export; increase the padding.".into(),
            );
        }
    }
    let copy = job.crop.is_none() && !job.precise && job.quality == 0 && job.keep.is_none();
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
        return Err("ffmpeg has no H.264/HEVC encoder (or ffmpeg was not found).".into());
    }
    let dur = job.kept_duration().max(0.001);
    let mut err = String::new();
    for enc in encs {
        let mut child = command("ffmpeg")
            .args(args(job, enc))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Can't run ffmpeg: {e}"))?;
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
    Err(tail.into_iter().rev().collect::<Vec<_>>().join("\n"))
}

/// Grab `n` evenly spaced `w`×`h` frames in the background. Stops early once `tx`'s receiver is dropped.
pub fn thumbnails(
    src: PathBuf,
    dur: f64,
    n: usize,
    [w, h]: [usize; 2],
    tx: Sender<(usize, ColorImage)>,
    ctx: Context,
) {
    std::thread::spawn(move || {
        for i in 0..n {
            let t = dur * (i as f64 + 0.5) / n as f64;
            let Ok(out) = command("ffmpeg")
                .args(["-v", "error", "-ss", &format!("{t:.3}"), "-i"])
                .arg(&src)
                .args([
                    "-frames:v",
                    "1",
                    "-vf",
                    &format!("scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h}"),
                    "-f",
                    "rawvideo",
                    "-pix_fmt",
                    "rgba",
                    "-",
                ])
                .output()
            else {
                return;
            };
            if out.stdout.len() != w * h * 4 {
                continue;
            }
            if tx
                .send((i, ColorImage::from_rgba_unmultiplied([w, h], &out.stdout)))
                .is_err()
            {
                return;
            }
            ctx.request_repaint();
        }
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
            a.contains(&format!("-vf select='{sel}',setpts=N/FRAME_RATE/TB")),
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

    /// Needs `ffmpeg` and `ffprobe` on PATH.
    #[test]
    fn export_round_trip() {
        let dir = std::env::temp_dir().join(format!("qpe-test-{}", std::process::id()));
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
        let _ = std::fs::remove_dir_all(&dir);
    }
}
