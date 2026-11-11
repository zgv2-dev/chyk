//! Asking GitHub, through the system `curl`, for the newest release and this system's file in it.

use super::{Kind, State, paths};
use std::process::{Command, Output, Stdio};

pub const REPO: &str = "https://github.com/zgv2-dev/chyk";

/// The newest release, and the size of this system's file in it.
#[derive(Debug, PartialEq)]
pub struct Found {
    pub tag: String,
    /// `None` when the release has no file for this system, or it isn't downloadable here.
    pub size: Option<u64>,
}

/// `curl` with the window hidden on Windows (Chyk has no console there).
pub fn curl() -> Command {
    let mut cmd = Command::new("curl");
    cmd.stdin(Stdio::null());
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000); // CREATE_NO_WINDOW
    cmd
}

/// Response headers of `url` (every hop when `follow`), or `None` if curl is missing or fails.
fn head(url: &str, follow: bool) -> Option<String> {
    let mut cmd = curl();
    // No connect timeout: looking up github.com took over 10 s on a Windows machine, which made
    // the check give up and never offer the update. It runs in the background, so waiting is free.
    cmd.args(["-sSI", "--max-time", "120"]);
    if follow {
        cmd.arg("-L");
    }
    let Output { status, stdout, .. } = cmd.arg(url).output().ok()?;
    status
        .success()
        .then(|| String::from_utf8_lossy(&stdout).into_owned())
}

/// The latest release tag and the size of `asset` in it. `None` offline or on any error.
pub fn latest(asset: Option<&str>) -> Option<Found> {
    let tag = tag_from_location(&head(&format!("{REPO}/releases/latest"), false)?)?;
    let size = asset.and_then(|a| {
        let headers = head(&format!("{REPO}/releases/download/{tag}/{a}"), true)?;
        let (status, len) = last_response(&headers);
        (status == Some(200)).then_some(len).flatten()
    });
    Some(Found { tag, size })
}

/// The tag `releases/latest` redirects to: `location: …/releases/tag/v1.2.1` → `v1.2.1`.
pub fn tag_from_location(headers: &str) -> Option<String> {
    let line = headers
        .lines()
        .find(|l| l.to_ascii_lowercase().starts_with("location:"))?;
    let tag = line.trim().rsplit('/').next()?;
    parse_version(tag)?;
    Some(tag.to_string())
}

/// Status and `content-length` of the final response in curl's `-I -L` output.
pub fn last_response(headers: &str) -> (Option<u16>, Option<u64>) {
    let (mut status, mut len) = (None, None);
    for line in headers.lines() {
        let lower = line.trim().to_ascii_lowercase();
        if lower.starts_with("http/") {
            status = lower.split_whitespace().nth(1).and_then(|s| s.parse().ok());
            len = None;
        } else if let Some(v) = lower.strip_prefix("content-length:") {
            len = v.trim().parse().ok();
        }
    }
    (status, len)
}

/// `v1.2.3` or `1.2.3` → `[1, 2, 3]`. Missing parts count as 0; anything else is `None`.
pub fn parse_version(s: &str) -> Option<[u64; 3]> {
    let s = s.strip_prefix('v').unwrap_or(s);
    let mut out = [0; 3];
    let mut parts = s.split('.');
    for slot in &mut out {
        match parts.next() {
            Some(p) => *slot = p.parse().ok()?,
            None => break,
        }
    }
    (parts.next().is_none() && !s.is_empty()).then_some(out)
}

/// Whether `tag` is a newer version than `current`.
pub fn newer(tag: &str, current: &str) -> bool {
    matches!((parse_version(tag), parse_version(current)), (Some(a), Some(b)) if a > b)
}

/// The check's answer: offer a newer release, or a finished download of it from the cache.
/// Runs on the check thread, since it reads and cleans the cache folder.
pub fn decide(kind: &Kind, found: Option<Found>) -> (State, u64) {
    let (asset, dir) = (paths::asset(kind), paths::cache_dir());
    let current = env!("CARGO_PKG_VERSION");
    let cached = asset
        .zip(dir.as_deref())
        .and_then(|(a, d)| paths::newest_cached(d, a));
    let Some(found) = found else {
        // Offline: a download from an earlier run can still be swapped in.
        return match cached.filter(|(t, _)| newer(t, current)) {
            Some((tag, file)) => (State::Ready { tag, file }, 0),
            None => (State::None, 0),
        };
    };
    let newer = newer(&found.tag, current);
    let ready = cached.filter(|(t, _)| newer && *t == found.tag);
    if let Some(d) = &dir {
        paths::clean(d, ready.as_ref().map(|(_, f)| f.as_path()));
    }
    let size = found.size.unwrap_or(0);
    let state = match (ready, kind) {
        _ if !newer => State::None,
        (Some((tag, file)), _) => State::Ready { tag, file },
        (None, Kind::Package) => State::Available {
            tag: found.tag,
            manual: true,
        },
        // No file for this system in the release: nothing to offer.
        (None, _) if found.size.is_none() => State::None,
        (None, _) => State::Available {
            tag: found.tag,
            manual: false,
        },
    };
    (state, size)
}
