use super::*;

/// A join clip's place in its output frame: how it fits, and the frame's `[w, h]`.
pub type InFrame = Option<(Fit, [u32; 2])>;

/// ffmpeg arguments for one frame at `t` seconds, fitted into a join's frame (`fit`) and cropped
/// like an export (`[w, h, x, y]`), no other scaling.
pub(super) fn frame_args(
    src: &Path,
    t: f64,
    fit: InFrame,
    crop: Option<[u32; 4]>,
    dst: &Path,
) -> Vec<String> {
    let mut a: Vec<String> = ["-hide_banner", "-nostdin", "-y", "-v", "error", "-ss"]
        .map(String::from)
        .into();
    a.push(format!("{t:.3}"));
    a.extend(["-i".into(), src.to_string_lossy().into_owned()]);
    a.extend(["-frames:v".into(), "1".into()]);
    let mut vf: Vec<String> = fit
        .map(|(f, frame)| normalize(f, frame, None))
        .into_iter()
        .collect();
    vf.extend(crop.map(|[w, h, x, y]| format!("crop={w}:{h}:{x}:{y}")));
    if !vf.is_empty() {
        a.extend(["-vf".into(), vf.join(",")]);
    }
    // One image, named literally: without it image2 reads `%d` in the name as a sequence pattern.
    a.extend([
        "-update".into(),
        "1".into(),
        dst.to_string_lossy().into_owned(),
    ]);
    a
}

/// Blocking: run on a thread. Err is ffmpeg's last lines.
pub fn save_frame(
    src: &Path,
    t: f64,
    fit: InFrame,
    crop: Option<[u32; 4]>,
    dst: &Path,
) -> Result<(), String> {
    let mut cmd = command("ffmpeg");
    cmd.args(frame_args(src, t, fit, crop, dst))
        .stdin(Stdio::null());
    let out = log::output(log::begin(log::Kind::Frame), &mut cmd).map_err(|e| e.to_string())?;
    if out.status.success() && dst.exists() {
        Ok(())
    } else {
        let _ = std::fs::remove_file(dst);
        Err(error_tail(&String::from_utf8_lossy(&out.stderr)))
    }
}
