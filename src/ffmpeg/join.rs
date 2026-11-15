//! Join: clips one after another in one output frame. Lossless through the concat demuxer when
//! every clip matches; else one input per clip (or per kept run) normalized, then `concat`.

use super::*;

/// How a clip fills the output frame. `Fit` is the name the UI shows, so it stays.
#[allow(clippy::enum_variant_names)]
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Fit {
    /// The whole picture, black bars where the aspect differs.
    #[default]
    Fit,
    /// Fills the frame; the overhanging edges are cut off.
    Fill,
    /// Fills the frame by distorting the picture.
    Stretch,
    /// Own size, top left at `[x, y]` (may overhang, cutting the clip off); black around it.
    Place([i32; 2]),
}

impl Fit {
    /// Same mode, wherever a placed clip sits.
    pub fn same_mode(self, other: Fit) -> bool {
        std::mem::discriminant(&self) == std::mem::discriminant(&other)
    }

    /// Whether a clip already the frame's size comes out unchanged.
    pub fn keeps_frame(self) -> bool {
        !matches!(self, Fit::Place(o) if o != [0, 0])
    }
}

/// One clip of a join: a source, its trim, and the kept intervals when stripped.
#[derive(Clone, PartialEq, Debug)]
pub struct Part {
    pub src: PathBuf,
    pub start: f64,
    pub end: f64,
    pub fit: Fit,
    /// Absolute source times to keep (Strip). `Some(empty)` drops the clip.
    pub keep: Option<Vec<(f64, f64)>>,
    /// Has an audio stream. The export fills this in from `ffmpeg -i`.
    pub audio: bool,
}

impl Part {
    pub fn kept(&self) -> f64 {
        self.keep.as_ref().map_or(self.end - self.start, |k| {
            k.iter().map(|&(a, b)| (b - a).max(0.0)).sum()
        })
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Join {
    pub parts: Vec<Part>,
    /// Output `[w, h]`, even.
    pub frame: [u32; 2],
    /// Output frame rate; `None` keeps the clips' own (they all match).
    pub fps: Option<f64>,
}

/// Scale (and pad or crop) into `[w, h]`, square pixels, 4:2:0, at `fps`.
pub(super) fn normalize(fit: Fit, [w, h]: [u32; 2], fps: Option<f64>) -> String {
    let s = match fit {
        Fit::Fit => format!(
            "scale={w}:{h}:force_original_aspect_ratio=decrease:force_divisible_by=2,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2"
        ),
        Fit::Fill => format!(
            "scale={w}:{h}:force_original_aspect_ratio=increase:force_divisible_by=2,crop={w}:{h}"
        ),
        Fit::Stretch => format!("scale={w}:{h}"),
        // The part inside the frame, padded out to it.
        Fit::Place([x, y]) => {
            let (cx, cy) = ((-x).max(0), (-y).max(0));
            let (px, py) = (x.clamp(0, w as i32), y.clamp(0, h as i32));
            format!(
                "crop=max(2\\,min(iw-{cx}\\,{w}-{px})):max(2\\,min(ih-{cy}\\,{h}-{py})):{cx}:{cy},pad={w}:{h}:{px}:{py}"
            )
        }
    };
    let rate = fps.map_or_else(String::new, |f| format!(",fps={f:.3}"));
    format!("{s},setsar=1{rate},format=yuv420p")
}

/// One ffmpeg input: `(start, end, kept intervals)`.
type Run = (f64, f64, Option<Vec<(f64, f64)>>);

/// Inputs: one per clip, or one per run of kept intervals.
fn runs(p: &Part, max: usize) -> Vec<Run> {
    let ms = |t: f64| (t * 1000.0).round() / 1000.0;
    match &p.keep {
        None => vec![(ms(p.start), ms(p.end), None)],
        Some(keep) => spans(keep, max, MERGE_GAP)
            .into_iter()
            .map(|(lo, hi)| {
                let k = &keep[lo..hi];
                (ms(k[0].0), ms(k[k.len() - 1].1), Some(k.to_vec()))
            })
            .collect(),
    }
}

/// Re-encoding join of `j` with encoder `enc`. `src` gives the bitrate caps.
pub(super) fn join_args(job: &Job, j: &Join, enc: &str, src: &Source) -> Vec<String> {
    let audio = !job.mute && j.parts.iter().any(|p| p.audio);
    let longest = j.parts.iter().map(|p| p.src.as_os_str().len()).max();
    let per = (max_inputs(longest.unwrap_or(0)) / j.parts.len().max(1)).max(1);
    let (mut a, mut graph, mut pads, mut n) = (head(), String::new(), String::new(), 0);
    for p in &j.parts {
        for (start, end, keep) in runs(p, per) {
            let path = p.src.to_string_lossy().into_owned();
            a.extend([
                "-ss".into(),
                format!("{start:.3}"),
                "-to".into(),
                format!("{end:.3}"),
            ]);
            a.extend(["-i".into(), path]);
            let rel = keep.as_ref().map(|k| relative(k, start));
            let len = rel
                .as_ref()
                .map_or(end - start, |r| r.iter().map(|(a, b)| b - a).sum());
            let (vsel, asel) = rel.as_ref().map_or_else(
                || ("setpts=PTS-STARTPTS".into(), "asetpts=PTS-STARTPTS".into()),
                |r| {
                    let sel = select_expr(r);
                    (
                        format!("select='{sel}',setpts='{}'", close_gaps(r)),
                        format!("aselect='{sel}',asetpts=N/SR/TB"),
                    )
                },
            );
            let fit = normalize(p.fit, j.frame, j.fps);
            graph.push_str(&format!("[{n}:v:0]{vsel},{fit}[v{n}];"));
            pads.push_str(&format!("[v{n}]"));
            if audio {
                if p.audio {
                    graph.push_str(&format!(
                        "[{n}:a:0]{asel},aresample=48000,aformat=channel_layouts=stereo[a{n}];"
                    ));
                } else {
                    graph.push_str(&format!(
                        "anullsrc=r=48000:cl=stereo,atrim=duration={len:.3}[a{n}];"
                    ));
                }
                pads.push_str(&format!("[a{n}]"));
            }
            n += 1;
        }
    }
    graph.push_str(&format!("{pads}concat=n={n}:v=1:a={}[vj]", u8::from(audio)));
    if audio {
        graph.push_str("[aj]");
    }
    // Crop and resolution act on the joined frame, then the retime.
    let mut post = Vec::new();
    if let Some([w, h, x, y]) = job.crop {
        post.push(format!("crop={w}:{h}:{x}:{y}"));
    }
    if let Some([w, h]) = job.scale {
        post.push(format!("scale={w}:{h}"));
    }
    if job.retime() {
        post.push(format!("setpts=PTS/{:.6}", job.speed));
    }
    let v = if post.is_empty() {
        "[vj]"
    } else {
        graph.push_str(&format!(";[vj]{}[v]", post.join(",")));
        "[v]"
    };
    let aud = if job.retime() && audio {
        graph.push_str(&format!(";[aj]{}[a]", atempo(job.speed)));
        "[a]"
    } else {
        "[aj]"
    };
    a.extend(["-filter_complex".into(), graph, "-map".into(), v.into()]);
    if audio {
        a.extend(["-map".into(), aud.into()]);
    }
    a.extend(["-c:v".into(), enc.into()]);
    a.extend(quality(enc, job.quality, src.video_kbps, src.strict));
    if audio {
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
    tag_hevc(&mut a, HEVC.contains(&enc), &job.dst);
    a.push(job.dst.to_string_lossy().into_owned());
    a
}
