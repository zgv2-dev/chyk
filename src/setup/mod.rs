#![cfg_attr(not(windows), allow(dead_code))]

//! Installing on Windows, from the same exe. Started from outside an install (e.g. Downloads),
//! Chyk shows its setup window (`app::setup_window`) instead of the editor; this module does the
//! work. Everything is per user: the exe goes to `%LOCALAPPDATA%\Programs\Chyk`, the rest under
//! HKCU, so no admin. Installed apps runs `Chyk.exe --uninstall` to remove it again.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

#[cfg(test)]
mod tests;
#[cfg(windows)]
mod win;
#[cfg(windows)]
mod work;
#[cfg(windows)]
pub use work::{delete_later, install, refresh, skip, uninstall};

pub const EXE: &str = "Chyk.exe";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Chyk";
const SETUP_KEY: &str = r"Software\Chyk\Setup";
const PROG_ID: &str = "Chyk.Video";
const CAPABILITIES: &str = r"Software\Chyk\Capabilities";
/// Keys only Chyk writes; uninstall deletes them whole.
const OWNED: [&str; 3] = [
    r"Software\Classes\Chyk.Video",
    r"Software\Classes\Applications\Chyk.exe",
    CAPABILITIES,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Install,
    Uninstall,
}

/// What this launch shows instead of the editor, if anything. Elsewhere only the flags open
/// the window (to look at it; there is nothing to install).
pub fn mode() -> Option<Mode> {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    #[cfg(windows)]
    return decide(
        &args,
        work::installed().is_some(),
        work::skipped(),
        cfg!(debug_assertions),
    );
    #[cfg(not(windows))]
    decide(&args, true, true, true)
}

/// [`mode`] without the system. `args` come after the program name; `installed`: an install is
/// registered (this copy may be it); `skipped`: "Run without installing" was chosen before.
/// `--setup` opens the window anyway, so a skipped or debug copy can still install.
pub(crate) fn decide(
    args: &[OsString],
    installed: bool,
    skipped: bool,
    debug: bool,
) -> Option<Mode> {
    match args.first().and_then(|a| a.to_str()) {
        Some("--uninstall") => Some(Mode::Uninstall),
        Some("--setup") => Some(Mode::Install),
        // A file to open, or another flag: the editor.
        Some(_) => None,
        None => (!installed && !skipped && !debug).then_some(Mode::Install),
    }
}

/// What the Options page sets.
#[derive(Clone, Debug)]
pub struct Choices {
    pub dir: PathBuf,
    /// "Edit with Chyk" in Explorer's menu.
    pub menu: bool,
    /// Chyk in "Open with" and Default apps for videos.
    pub assoc: bool,
    pub desktop: bool,
    pub updates: bool,
}

impl Default for Choices {
    fn default() -> Self {
        Self {
            dir: default_dir().unwrap_or_default(),
            menu: true,
            assoc: false,
            desktop: false,
            updates: true,
        }
    }
}

/// `%LOCALAPPDATA%\Programs\Chyk`, where per-user apps go.
pub fn default_dir() -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA").filter(|v| !v.is_empty())?;
    Some(PathBuf::from(local).join("Programs").join("Chyk"))
}

/// The install folder for a folder picked in Change…: Chyk gets its own folder inside it.
pub fn folder_for(picked: &Path) -> PathBuf {
    if picked
        .file_name()
        .is_some_and(|n| n.eq_ignore_ascii_case("Chyk"))
    {
        picked.to_path_buf()
    } else {
        picked.join("Chyk")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Ok,
    Warn,
    Fail,
}

/// What the install and uninstall threads report.
#[derive(Debug)]
pub enum Msg {
    Line(Kind, String),
    /// Share of the work done, 0 to 1.
    Progress(f32),
    /// Finished; the installed exe, or `None` when a step failed.
    Done(Option<PathBuf>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Value {
    Str(OsString),
    Dword(u32),
}

/// `"exe"` followed by `rest`.
fn command(exe: &Path, rest: &str) -> OsString {
    let mut s = OsString::from("\"");
    s.push(exe.as_os_str());
    s.push("\"");
    s.push(rest);
    s
}

/// The Installed apps entry for `exe`, `size_kb` big.
pub(crate) fn uninstall_values(exe: &Path, size_kb: u32) -> Vec<(&'static str, Value)> {
    let dir = exe.parent().unwrap_or(exe).as_os_str().to_owned();
    vec![
        ("DisplayName", Value::Str("Chyk".into())),
        ("DisplayVersion", Value::Str(VERSION.into())),
        ("Publisher", Value::Str("zgv2".into())),
        ("DisplayIcon", Value::Str(command(exe, ",0"))),
        ("InstallLocation", Value::Str(dir)),
        ("UninstallString", Value::Str(command(exe, " --uninstall"))),
        ("URLInfoAbout", Value::Str("https://chyk.zgv2.dev".into())),
        ("EstimatedSize", Value::Dword(size_kb)),
        ("NoModify", Value::Dword(1)),
        ("NoRepair", Value::Dword(1)),
    ]
}

/// Registry values (key under HKCU, name, data) that put Chyk in "Open with" and Default apps
/// for every video type. Windows lets only the user pick the default, so this offers Chyk there.
pub(crate) fn assoc_values(exe: &Path) -> Vec<(String, String, OsString)> {
    let classes = r"Software\Classes";
    let s = String::from;
    let mut v: Vec<(String, String, OsString)> = vec![
        (format!(r"{classes}\{PROG_ID}"), s(""), "Video".into()),
        (
            format!(r"{classes}\{PROG_ID}\DefaultIcon"),
            s(""),
            command(exe, ",0"),
        ),
        (
            format!(r"{classes}\{PROG_ID}\shell\open\command"),
            s(""),
            command(exe, " \"%1\""),
        ),
        (
            format!(r"{classes}\Applications\{EXE}"),
            s("FriendlyAppName"),
            "Chyk".into(),
        ),
        (s(CAPABILITIES), s("ApplicationName"), "Chyk".into()),
        (
            s(CAPABILITIES),
            s("ApplicationDescription"),
            "Video trimmer".into(),
        ),
        (
            s(r"Software\RegisteredApplications"),
            s("Chyk"),
            CAPABILITIES.into(),
        ),
    ];
    for ext in crate::app::videos::VIDEO_EXTS {
        let dot = format!(".{ext}");
        v.push((
            format!(r"{classes}\{dot}\OpenWithProgids"),
            s(PROG_ID),
            "".into(),
        ));
        v.push((
            format!(r"{classes}\Applications\{EXE}\SupportedTypes"),
            dot.clone(),
            "".into(),
        ));
        v.push((
            format!(r"{CAPABILITIES}\FileAssociations"),
            dot,
            PROG_ID.into(),
        ));
    }
    v
}

/// `text` (a settings file) with `key = 0/1`, replacing an earlier line for the same key.
pub(crate) fn with_flag(text: &str, key: &str, on: bool) -> String {
    let mut out: Vec<String> = text
        .lines()
        .filter(|l| l.split_once('=').is_none_or(|(k, _)| k.trim() != key))
        .map(str::to_owned)
        .collect();
    out.push(format!("{key} = {}", u8::from(on)));
    out.join("\n") + "\n"
}

/// The `cmd /c` line that deletes `files` and the folders (`empty` only if nothing else is in
/// them, `whole` with everything inside) once Chyk has exited. It retries every second for a
/// minute, since an open Chyk window keeps its exe locked, and stops once the first file is gone.
/// The paths go in as environment variables, so `cmd` never reads `%` or `^` in them.
pub(crate) fn cleanup_script(
    files: &[PathBuf],
    empty: &[PathBuf],
    whole: &[PathBuf],
) -> (String, Vec<(String, OsString)>) {
    let mut env = Vec::new();
    let mut var = |p: &PathBuf| {
        let name = format!("CHYK_P{}", env.len());
        env.push((name.clone(), p.as_os_str().to_owned()));
        format!("\"%{name}%\"")
    };
    let mut steps = vec!["ping -n 2 127.0.0.1 >nul".to_owned()];
    let first = files.first().map(&mut var);
    if let Some(first) = &first {
        let rest: Vec<String> = files[1..].iter().map(&mut var).collect();
        steps.push(format!("del /f /q {first} {} 2>nul", rest.join(" ")).replace("  ", " "));
    }
    steps.extend(empty.iter().map(|d| format!("rmdir {} 2>nul", var(d))));
    steps.extend(
        whole
            .iter()
            .map(|d| format!("rmdir /s /q {} 2>nul", var(d))),
    );
    if let Some(first) = &first {
        steps.push(format!("if not exist {first} exit"));
    }
    (
        format!("for /l %i in (1,1,60) do @({})", steps.join(" & ")),
        env,
    )
}

/// Whether `a` and `b` are the same file, through whatever path spelling (junctions, subst
/// drives). Paths that can't be resolved count as different.
pub fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a.as_os_str().eq_ignore_ascii_case(b.as_os_str()),
        _ => false,
    }
}

/// Reports to the window: a line, then the progress so far.
pub(crate) struct Report<'a> {
    tx: &'a Sender<Msg>,
    done: f32,
    total: f32,
}

impl<'a> Report<'a> {
    pub(crate) fn new(tx: &'a Sender<Msg>, total: usize) -> Self {
        Self {
            tx,
            done: 0.0,
            total: total.max(1) as f32,
        }
    }

    pub(crate) fn line(&mut self, kind: Kind, text: impl Into<String>) {
        self.done += 1.0;
        let _ = self.tx.send(Msg::Line(kind, text.into()));
        let _ = self
            .tx
            .send(Msg::Progress((self.done / self.total).min(1.0)));
    }
}

/// Elsewhere there is nothing to install; the window only shows up with `--setup` on Windows.
#[cfg(not(windows))]
pub fn install(_: Choices, tx: Sender<Msg>) {
    let _ = tx.send(Msg::Line(Kind::Fail, "Setup is for Windows.".into()));
    let _ = tx.send(Msg::Done(None));
}

#[cfg(not(windows))]
pub fn uninstall(_: bool, tx: Sender<Msg>) {
    let _ = tx.send(Msg::Done(None));
}

#[cfg(not(windows))]
pub fn skip() {}

#[cfg(not(windows))]
pub fn delete_later(_: &Path) {}

#[cfg(not(windows))]
pub fn refresh() {}
