//! Temp files go in a hidden folder next to the source video, private to the user.

use super::*;
use crate::ffmpeg::scratch::{DIR, create};

/// An empty folder for one test, removed on drop.
struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Self {
        let d = temp().join(format!("chyk-scratch-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        Self(d)
    }
}

/// The system temp folder as temp paths show it: links resolved on Unix (`/var` is
/// `/private/var` on macOS, long names on Windows).
fn temp() -> PathBuf {
    real(&std::env::temp_dir())
}

/// `p` with links resolved, as temp paths show it (no `\\?\` on Windows drive paths).
fn real(p: &Path) -> PathBuf {
    let p = std::fs::canonicalize(p).unwrap();
    #[cfg(windows)]
    let p = p
        .to_str()
        .and_then(|s| s.strip_prefix(r"\\?\"))
        .map_or(p.clone(), PathBuf::from);
    p
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn temp_files_sit_hidden_next_to_the_source_and_go_with_the_last_one() {
    let d = Dir::new("near");
    let src = d.0.join("clip.mp4");
    let (a, _) = create(&src, "seams", "mp4").unwrap();
    let (b, _) = create(&src, "seams", "txt").unwrap();
    let tmp = d.0.join(DIR);
    assert_eq!(a.0.parent(), Some(tmp.as_path()));
    assert_ne!(a.0, b.0);
    assert!(a.0.extension().is_some_and(|e| e == "mp4"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&tmp), 0o700);
        assert_eq!(mode(&a.0), 0o600);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        assert_ne!(std::fs::metadata(&tmp).unwrap().file_attributes() & 0x2, 0);
    }
    // The folder stays while another file is in it.
    drop(a);
    assert!(tmp.is_dir());
    drop(b);
    assert!(!tmp.exists());
}

#[cfg(unix)]
#[test]
fn a_folder_others_can_open_is_not_used() {
    use std::os::unix::fs::PermissionsExt;
    let d = Dir::new("open");
    let tmp = d.0.join(DIR);
    std::fs::create_dir(&tmp).unwrap();
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755)).unwrap();
    let (f, _) = create(&d.0.join("clip.mp4"), "sample", "mkv").unwrap();
    assert!(!f.0.starts_with(&tmp), "{}", f.0.display());
    assert!(f.0.starts_with(temp()));
    let dir = f.0.parent().unwrap();
    assert_eq!(
        std::fs::metadata(dir).unwrap().permissions().mode() & 0o777,
        0o700
    );
}

#[cfg(unix)]
#[test]
fn a_folder_others_can_change_keeps_temp_files_out() {
    use std::os::unix::fs::PermissionsExt;
    let d = Dir::new("shared");
    let shared = d.0.join("shared");
    std::fs::create_dir(&shared).unwrap();
    // Others could rename `.chyk-tmp` here and put their own folder in its place.
    std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o777)).unwrap();
    let (f, _) = create(&shared.join("clip.mp4"), "join", "txt").unwrap();
    assert!(!f.0.starts_with(&shared), "{}", f.0.display());
    assert!(!shared.join(DIR).exists());
    // A folder below it is no safer: the shared one can still be swapped.
    let below = shared.join("below");
    std::fs::create_dir(&below).unwrap();
    let (g, _) = create(&below.join("clip.mp4"), "join", "txt").unwrap();
    assert!(!g.0.starts_with(&shared), "{}", g.0.display());
}

#[test]
fn a_file_in_the_way_falls_back_to_the_private_folder() {
    let d = Dir::new("blocked");
    std::fs::write(d.0.join(DIR), "").unwrap();
    let (f, _) = create(&d.0.join("clip.mp4"), "graph", "txt").unwrap();
    assert!(f.0.starts_with(temp()) && !f.0.starts_with(&d.0));
}

#[test]
fn what_a_crashed_run_left_is_cleared() {
    let d = Dir::new("stale");
    let tmp = d.0.join(DIR);
    std::fs::create_dir(&tmp).unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(&tmp, std::os::unix::fs::PermissionsExt::from_mode(0o700)).unwrap();
    let (old, new) = (tmp.join("seams-1-0.mp4"), tmp.join("seams-1-1.mp4"));
    let day = std::time::Duration::from_secs(25 * 60 * 60);
    let f = std::fs::File::create(&old).unwrap();
    f.set_modified(std::time::SystemTime::now() - day).unwrap();
    std::fs::write(&new, "").unwrap();
    let _t = create(&d.0.join("clip.mp4"), "seams", "mp4").unwrap();
    assert!(!old.exists() && new.exists());
}

/// No one can swap the temp folder, or a folder above it, while a temp file is in it.
#[cfg(windows)]
#[test]
fn the_temp_folder_and_its_path_cannot_be_moved_while_in_use() {
    let d = Dir::new("pinned");
    let inner = d.0.join("videos");
    std::fs::create_dir(&inner).unwrap();
    let (a, _) = create(&inner.join("clip.mp4"), "seams", "mp4").unwrap();
    let tmp = inner.join(DIR);
    assert_eq!(a.0.parent(), Some(tmp.as_path()));
    assert!(std::fs::rename(&tmp, d.0.join("moved")).is_err());
    assert!(std::fs::rename(&inner, d.0.join("moved")).is_err());
    drop(a);
    assert!(!tmp.exists());
    std::fs::rename(&inner, d.0.join("moved")).unwrap();
}

/// A junction on the way is resolved, so swapping it later can't redirect the temp files.
#[cfg(windows)]
#[test]
fn a_junction_on_the_way_is_followed_to_the_real_folder() {
    let d = Dir::new("junction");
    let target = d.0.join("real");
    std::fs::create_dir(&target).unwrap();
    let link = d.0.join("link");
    let ok = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(&target)
        .status()
        .unwrap()
        .success();
    assert!(ok);
    let (a, _) = create(&link.join("clip.mp4"), "seams", "mp4").unwrap();
    assert_eq!(a.0.parent(), Some(target.join(DIR).as_path()));
    drop(a);
    assert!(!target.join(DIR).exists());
}
