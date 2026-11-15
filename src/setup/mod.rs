#![cfg_attr(not(windows), allow(dead_code))]

//! Installing on Windows, from the same exe. The installer build (the default Windows build;
//! the portable one is `--features portable`) shows its setup window (`app::setup_window`)
//! instead of the editor when it runs from anywhere but an install; this module does the work.
//! - Just for me: `%LOCALAPPDATA%\Programs\Chyk`, everything else under HKCU, no admin.
//! - All users: `C:\Program Files\Chyk`, registered under HKLM, with the all-users Start menu
//!   and public desktop. Setup restarts itself elevated (UAC) for this.
//!
//! ffmpeg and mpv are unpacked into the install folder as an install step. Installed apps runs
//! `Chyk.exe --uninstall`, and the updater runs a newer setup with `--update <folder>`.

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
pub use work::{
    delete_later, elevated, install, open_unelevated, refresh, restart_elevated, run_and_wait,
    uninstall, update_from_args,
};

pub const EXE: &str = "Chyk.exe";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Chyk";
const PROG_ID: &str = "Chyk.Video";
/// Keys only Chyk writes; uninstall deletes them whole.
const OWNED: [&str; 2] = [
    r"Software\Classes\Chyk.Video",
    r"Software\Classes\Applications\Chyk.exe",
];
/// Libraries next to Chyk.exe mark an installed copy (setup unpacks them there).
const MARKER: &str = "libmpv-2.dll";

/// Who an install is for, which decides the folder, the registry hive and the shortcuts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// HKCU, the user's own Start menu and desktop. No admin rights.
    User,
    /// HKLM, the all-users Start menu and public desktop. Needs admin rights.
    Machine,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The setup window from its Welcome page.
    Install,
    /// The elevated copy setup started for an all-users install into this folder: opens on Options.
    InstallAllUsers(PathBuf),
    /// The uninstall window, from Installed apps.
    Uninstall,
    /// The elevated copy the uninstall window started: removes at once.
    UninstallNow { keep_settings: bool },
}

/// This is the installer build running from its install folder: the editor, not setup.
pub fn installed_copy() -> bool {
    !crate::portable::ON && cfg!(windows) && crate::portable::exe_dir().join(MARKER).is_file()
}

/// What this launch shows instead of the editor, if anything. Elsewhere only the flags open
/// the window (to look at it; there is nothing to install).
pub fn mode() -> Option<Mode> {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    // The portable build never installs; debug builds start the editor unless asked.
    let installer =
        cfg!(windows) && !crate::portable::ON && !cfg!(debug_assertions) && !installed_copy();
    decide(&args, installer)
}

/// [`mode`] without the system. `args` come after the program name; `installer`: a plain launch
/// opens setup (the installer build, not running from an install).
pub(crate) fn decide(args: &[OsString], installer: bool) -> Option<Mode> {
    let arg = |i: usize| args.get(i).and_then(|a| a.to_str());
    match arg(0) {
        Some("--uninstall") if arg(1) == Some("--confirm") => Some(Mode::UninstallNow {
            keep_settings: arg(2) == Some("--keep-settings"),
        }),
        Some("--uninstall") => Some(Mode::Uninstall),
        Some("--setup") if arg(1) == Some("--all-users") => match args.get(2) {
            Some(dir) => Some(Mode::InstallAllUsers(PathBuf::from(dir))),
            None => Some(Mode::Install),
        },
        Some("--setup") => Some(Mode::Install),
        // A file to open, or another flag: the editor.
        Some(_) => None,
        None => installer.then_some(Mode::Install),
    }
}

/// What the Welcome and Options pages set.
#[derive(Clone, Debug)]
pub struct Choices {
    pub scope: Scope,
    pub dir: PathBuf,
    /// "Edit with Chyk" in Explorer's menu.
    pub menu: bool,
    pub desktop: bool,
    pub updates: bool,
}

impl Default for Choices {
    fn default() -> Self {
        Self {
            scope: Scope::User,
            dir: default_dir(Scope::User).unwrap_or_default(),
            menu: true,
            desktop: false,
            updates: true,
        }
    }
}

fn env_dir(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// `%LOCALAPPDATA%\Programs\Chyk` for the user, `%ProgramFiles%\Chyk` for all users.
pub fn default_dir(scope: Scope) -> Option<PathBuf> {
    Some(match scope {
        Scope::User => env_dir("LOCALAPPDATA")?.join("Programs").join("Chyk"),
        Scope::Machine => env_dir("ProgramFiles")?.join("Chyk"),
    })
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

/// `args` as one Windows command line, each quoted so it comes back whole (the
/// `CommandLineToArgvW` rules: backslashes are doubled only before a quote).
pub(crate) fn command_line(args: &[OsString]) -> OsString {
    let mut out = String::new();
    for (i, arg) in args.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push('"');
        let mut slashes = 0;
        for c in arg.to_string_lossy().chars() {
            match c {
                '\\' => slashes += 1,
                '"' => {
                    out.extend(std::iter::repeat_n('\\', slashes * 2 + 1));
                    out.push('"');
                    slashes = 0;
                }
                c => {
                    out.extend(std::iter::repeat_n('\\', slashes));
                    out.push(c);
                    slashes = 0;
                }
            }
        }
        out.extend(std::iter::repeat_n('\\', slashes * 2));
        out.push('"');
    }
    out.into()
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

/// Registry values (key under HKCU, name, data) that put Chyk in "Open with" for every video
/// type. Chyk is an editor, not a player, so it never asks to be the default.
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
            dot,
            "".into(),
        ));
    }
    v
}

/// Whether Chyk can install into `dir` without administrator rights (Program Files can't be).
/// A folder that doesn't exist yet is judged the way install makes it: a folder is created in
/// the closest one that does exist, and a file inside it. Only what this call created is
/// removed again.
pub fn writable(dir: &Path) -> bool {
    let Some(existing) = dir.ancestors().find(|d| d.is_dir()) else {
        return false;
    };
    let probe = format!(".chyk-setup-{}", std::process::id());
    if existing != dir {
        let folder = existing.join(probe);
        if std::fs::create_dir(&folder).is_err() {
            return false;
        }
        let ok = probe_file(&folder.join("probe"));
        let _ = std::fs::remove_dir(&folder);
        return ok;
    }
    probe_file(&existing.join(probe))
}

/// Creates `file` (never opening one that is already there), then deletes it.
fn probe_file(file: &Path) -> bool {
    let made = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(file)
        .is_ok();
    if made {
        let _ = std::fs::remove_file(file);
    }
    made
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
/// minute, since a running Chyk keeps its exe locked. Each target is tried until it is gone and
/// then left alone, so a file made later at the same path (a reinstall) is never touched.
///
/// The paths go in as environment variables `CHYK_Pn`, so `cmd` never reads `%` or `^` in them.
/// `"%CHYK_Pn%"` is expanded once, when the line is read; `if defined` is checked on every
/// pass, so clearing the variable marks its target done.
pub(crate) fn cleanup_script(
    files: &[PathBuf],
    empty: &[PathBuf],
    whole: &[PathBuf],
) -> (String, Vec<(String, OsString)>) {
    let targets = files
        .iter()
        .map(|p| (p, "del /f /q"))
        .chain(empty.iter().map(|p| (p, "rmdir")))
        .chain(whole.iter().map(|p| (p, "rmdir /s /q")));
    let mut env = Vec::new();
    let mut steps = vec!["ping -n 2 127.0.0.1 >nul".to_owned()];
    for (path, remove) in targets {
        let name = format!("CHYK_P{}", env.len());
        env.push((name.clone(), path.as_os_str().to_owned()));
        steps.push(format!(
            "if defined {name} ({remove} \"%{name}%\" 2>nul & if not exist \"%{name}%\" set {name}=)"
        ));
    }
    if env.is_empty() {
        return (String::new(), env);
    }
    let all: String = env
        .iter()
        .map(|(name, _)| format!("if not defined {name} "))
        .collect();
    steps.push(format!("{all}exit"));
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

    /// Progress within the step under way: `share` of it (0 to 1) is done.
    pub(crate) fn partial(&self, share: f32) {
        let _ = self.tx.send(Msg::Progress(
            ((self.done + share.clamp(0.0, 1.0)) / self.total).min(1.0),
        ));
    }
}

/// Elsewhere there is nothing to install; the window only shows up with `--setup`.
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
pub fn delete_later(_: &Path) {}

#[cfg(not(windows))]
pub fn refresh() {}

#[cfg(not(windows))]
pub fn update_from_args() -> Option<i32> {
    None
}

#[cfg(not(windows))]
pub fn elevated() -> bool {
    false
}

/// Starts `exe` with `args` (elevated if asked) and waits for its exit code.
#[cfg(not(windows))]
pub fn run_and_wait(exe: &Path, args: &[OsString], _elevate: bool) -> std::io::Result<u32> {
    let status = std::process::Command::new(exe).args(args).status()?;
    Ok(status.code().unwrap_or(1) as u32)
}

#[cfg(not(windows))]
pub fn restart_elevated(_: &[OsString]) -> std::io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}

#[cfg(not(windows))]
pub fn open_unelevated(exe: &Path) -> std::io::Result<()> {
    std::process::Command::new(exe).spawn().map(drop)
}

/// The scope of the registered install, if there is one. Elsewhere: none.
#[cfg(not(windows))]
pub fn installed_scope() -> Option<Scope> {
    None
}

#[cfg(windows)]
pub fn installed_scope() -> Option<Scope> {
    work::installed().map(|(_, scope)| scope)
}
