use super::*;

/// A temp file, deleted on drop.
pub(super) struct TempFile(pub(super) PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Moves a `-filter_complex` graph longer than [`GRAPH_INLINE`] into a temp file. If the file
/// can't be written the graph stays inline.
pub(super) fn spill_graph(a: &mut [String]) -> Option<TempFile> {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let i = a.iter().position(|s| s == "-filter_complex")?;
    if a[i + 1].len() <= GRAPH_INLINE {
        return None;
    }
    let n = N.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("chyk-graph-{}-{n}.txt", std::process::id()));
    std::fs::write(&path, &a[i + 1]).ok()?;
    a[i] = graph_file_option().into();
    a[i + 1] = path.to_string_lossy().into_owned();
    Some(TempFile(path))
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
