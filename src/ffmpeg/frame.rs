use super::*;

/// ffmpeg arguments for one frame at `t` seconds, cropped like an export (`[w, h, x, y]`), no scaling.
pub(super) fn frame_args(src: &Path, t: f64, crop: Option<[u32; 4]>, dst: &Path) -> Vec<String> {
    let mut a: Vec<String> = ["-hide_banner", "-nostdin", "-y", "-v", "error", "-ss"]
        .map(String::from)
        .into();
    a.push(format!("{t:.3}"));
    a.extend(["-i".into(), src.to_string_lossy().into_owned()]);
    a.extend(["-frames:v".into(), "1".into()]);
    if let Some([w, h, x, y]) = crop {
        a.extend(["-vf".into(), format!("crop={w}:{h}:{x}:{y}")]);
    }
    a.push(dst.to_string_lossy().into_owned());
    a
}

/// Blocking: run on a thread. Err is ffmpeg's last lines.
pub fn save_frame(src: &Path, t: f64, crop: Option<[u32; 4]>, dst: &Path) -> Result<(), String> {
    let out = command("ffmpeg")
        .args(frame_args(src, t, crop, dst))
        .stdin(Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() && dst.exists() {
        Ok(())
    } else {
        let _ = std::fs::remove_file(dst);
        Err(error_tail(&String::from_utf8_lossy(&out.stderr)))
    }
}
