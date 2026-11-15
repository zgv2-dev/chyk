//! The portable Windows build (`--features portable`) keeps everything in a `Chyk` folder next to
//! its exe: the unpacked libraries, settings, the session and update downloads. Nothing goes to
//! the user's profile or the registry, so the folder can move to another PC as it is.

use std::path::PathBuf;

/// Whether this is the portable build.
pub const ON: bool = cfg!(all(windows, feature = "portable"));

/// The folder next to the exe that holds this copy's data. `None` for every other build, which
/// use the system's folders.
pub fn data_dir() -> Option<PathBuf> {
    ON.then(|| exe_dir().join("Chyk"))
}

/// The folder Chyk.exe runs from.
pub fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}
