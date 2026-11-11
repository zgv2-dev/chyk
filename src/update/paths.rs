//! What kind of install this copy is, which release file it updates from, and the cache folder.

use std::path::{Path, PathBuf};

/// How this copy of Chyk is installed, which decides how it updates.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// `Chyk.app` (any name) on macOS: swapped from the release DMG.
    MacApp(PathBuf),
    /// The single-file Windows exe: swapped by renaming the running exe aside.
    WindowsExe(PathBuf),
    /// A Linux AppImage at this path: the file itself is replaced.
    AppImage(PathBuf),
    /// Installed by a Linux package manager (.deb, .rpm, AUR): only the download page is offered.
    Package,
}

/// This copy's install kind. `None` for builds that never update (debug builds, `cargo run`).
pub fn kind() -> Option<Kind> {
    if cfg!(debug_assertions) {
        return None;
    }
    let exe = std::env::current_exe().ok()?;
    let appimage = std::env::var_os("APPIMAGE").map(PathBuf::from);
    kind_from(std::env::consts::OS, &exe, appimage)
}

pub(super) fn kind_from(os: &str, exe: &Path, appimage: Option<PathBuf>) -> Option<Kind> {
    match os {
        "macos" => {
            let macos = exe.parent()?;
            let contents = macos.parent()?;
            let bundle = contents.parent()?;
            let is_bundle = macos.file_name()? == "MacOS"
                && contents.file_name()? == "Contents"
                && bundle.extension()? == "app";
            is_bundle.then(|| Kind::MacApp(bundle.to_path_buf()))
        }
        "windows" => Some(Kind::WindowsExe(exe.to_path_buf())),
        "linux" => match appimage {
            Some(path) => Some(Kind::AppImage(path)),
            None if exe.starts_with("/usr/") => Some(Kind::Package),
            None => None,
        },
        _ => None,
    }
}

/// The release file this install updates from (see SPEC.md, Packaging). `None`: download page only.
pub fn asset(kind: &Kind) -> Option<&'static str> {
    asset_for(kind, std::env::consts::ARCH)
}

pub(super) fn asset_for(kind: &Kind, arch: &str) -> Option<&'static str> {
    match (kind, arch) {
        (Kind::MacApp(_), "aarch64") => Some("Chyk-macos-arm64.dmg"),
        (Kind::MacApp(_), "x86_64") => Some("Chyk-macos-x86_64.dmg"),
        (Kind::WindowsExe(_), "x86_64") => Some("Chyk-windows-x64.exe"),
        (Kind::AppImage(_), "x86_64") => Some("Chyk-linux-x86_64.AppImage"),
        _ => None,
    }
}

/// Chyk's cache folder (downloads, recent-file thumbnails).
pub fn cache_base() -> Option<PathBuf> {
    let env = |k| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    Some(if cfg!(target_os = "macos") {
        env("HOME")?.join("Library/Caches/Chyk")
    } else if cfg!(windows) {
        env("LOCALAPPDATA")?.join("Chyk")
    } else {
        env("XDG_CACHE_HOME")
            .or_else(|| env("HOME").map(|h| h.join(".cache")))?
            .join("chyk")
    })
}

/// Where downloads wait until they're swapped in.
pub fn cache_dir() -> Option<PathBuf> {
    Some(cache_base()?.join("update"))
}

/// A finished download in the cache: `<tag>-<asset>`, e.g. `v1.2.1-Chyk-macos-arm64.dmg`.
pub fn cached_file(dir: &Path, tag: &str, asset: &str) -> PathBuf {
    dir.join(format!("{tag}-{asset}"))
}

/// The newest finished download for `asset` in the cache, as `(tag, path)`.
pub fn newest_cached(dir: &Path, asset: &str) -> Option<(String, PathBuf)> {
    let suffix = format!("-{asset}");
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let tag = name.strip_suffix(&suffix)?.to_string();
            super::check::parse_version(&tag)?;
            Some((tag, e.path()))
        })
        .max_by_key(|(tag, _)| super::check::parse_version(tag))
}

/// Delete everything in the cache except `keep`.
pub fn clean(dir: &Path, keep: Option<&Path>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let path = e.path();
        if Some(path.as_path()) == keep {
            continue;
        }
        let _ = if path.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
    }
}
