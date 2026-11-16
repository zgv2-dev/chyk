use super::*;

/// A temp file, deleted on drop, with its `.chyk-tmp` folder once that is empty. On Windows the
/// second field holds the folder open so it, and every folder above it, can't be renamed.
pub(super) struct TempFile(pub(super) PathBuf, pub(super) Option<std::fs::File>);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        drop(self.1.take());
        if let Some(dir) = self.0.parent().filter(|d| d.ends_with(scratch::DIR)) {
            // Fails while another job still has a file there.
            let _ = std::fs::remove_dir(dir);
        }
    }
}

/// Moves a `-filter_complex` graph longer than [`GRAPH_INLINE`] into a temp file for a job on
/// `src`. If the file can't be written the graph stays inline.
pub(super) fn spill_graph(a: &mut [String], src: &Path) -> Option<TempFile> {
    let i = a.iter().position(|s| s == "-filter_complex")?;
    if a[i + 1].len() <= GRAPH_INLINE {
        return None;
    }
    let (file, mut f) = scratch::create(src, "graph", "txt").ok()?;
    std::io::Write::write_all(&mut f, a[i + 1].as_bytes()).ok()?;
    let path = file.0.clone();
    a[i] = graph_file_option().into();
    a[i + 1] = path.to_string_lossy().into_owned();
    Some(file)
}

/// `-/filter_complex` reads a file in ffmpeg 7+; 8 dropped the older `-filter_complex_script`,
/// which distro ffmpeg 6 still needs.
pub(super) fn graph_file_option() -> &'static str {
    static OPT: OnceLock<&str> = OnceLock::new();
    OPT.get_or_init(|| {
        let mut cmd = command("ffmpeg");
        cmd.arg("-version");
        let out = log::output(log::begin(log::Kind::Probe), &mut cmd);
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
pub(super) fn major_version(v: &str) -> Option<u32> {
    let v = v.strip_prefix("ffmpeg version ")?;
    let v = v.strip_prefix('n').unwrap_or(v);
    v.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
}
