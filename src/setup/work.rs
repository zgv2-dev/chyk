//! Install, update and uninstall, run on a background thread by the setup window (update runs
//! headless, from the updater).

use super::*;
use crate::i18n::t;
use std::os::windows::process::CommandExt;
use std::process::Command;

/// No console window for the helper processes.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub use win::elevated;

/// The registered install folder and its scope, if its exe is still there. With both a per-user
/// and an all-users install, the one this exe runs from wins, else all users.
pub(super) fn installed() -> Option<(PathBuf, Scope)> {
    let found: Vec<_> = [Scope::Machine, Scope::User]
        .into_iter()
        .filter_map(|scope| {
            let dir = PathBuf::from(win::get(scope, UNINSTALL_KEY, "InstallLocation")?);
            dir.join(EXE).is_file().then_some((dir, scope))
        })
        .collect();
    let me = std::env::current_exe().ok();
    let mine = found.iter().position(|(dir, _)| {
        me.as_deref()
            .is_some_and(|me| same_file(&dir.join(EXE), me))
    });
    found.into_iter().nth(mine.unwrap_or(0))
}

/// The registered install and what it has now: menu entry, desktop shortcut, update check.
pub fn existing() -> Option<Existing> {
    let (dir, scope) = installed()?;
    Some(Existing {
        version: win::get(scope, UNINSTALL_KEY, "DisplayVersion")
            .map(|v| v.to_string_lossy().into_owned())
            .unwrap_or_default(),
        choices: Choices {
            scope,
            dir,
            menu: crate::shell_menu::is_on(),
            desktop: desktop_link(scope).is_some_and(|l| l.exists()),
            updates: crate::settings::Settings::load().check_updates,
            fresh: false,
        },
    })
}

/// Reinstall, before it starts: this account's cache and session, and its settings unless kept.
/// Runs in the window the user started, never the elevated copy, which may be another account.
/// A file Chyk still has open stays.
pub fn clear_user_data(keep_settings: bool) {
    for dir in user_dirs(keep_settings) {
        let _ = std::fs::remove_dir_all(dir);
    }
    if !keep_settings {
        let _ = win::delete_tree(Scope::User, r"Software\Chyk");
    }
}

fn start_menu_link(scope: Scope) -> Option<PathBuf> {
    win::programs(scope).ok().map(|d| d.join("Chyk.lnk"))
}

fn desktop_link(scope: Scope) -> Option<PathBuf> {
    win::desktop(scope).ok().map(|d| d.join("Chyk.lnk"))
}

fn size_kb(dir: &Path) -> u32 {
    let files = std::iter::once(EXE).chain(libraries());
    let bytes: u64 = files
        .filter_map(|n| std::fs::metadata(dir.join(n)).ok())
        .map(|m| m.len())
        .sum();
    (bytes / 1024) as u32
}

/// The files setup adds next to Chyk.exe.
fn libraries() -> Box<dyn Iterator<Item = &'static str>> {
    #[cfg(bundled)]
    return Box::new(crate::bundle::names());
    #[cfg(not(bundled))]
    Box::new(std::iter::empty())
}

/// Puts ffmpeg and mpv into `dir`: downloaded and checked against their pinned hashes, or kept
/// when a matching copy is already there. A dev build pins none and has nothing to add.
fn unpack(dir: &Path, progress: &mut dyn FnMut(f32)) -> Result<bool, String> {
    #[cfg(bundled)]
    return crate::bundle::unpack(dir, progress)
        .map(|()| true)
        .map_err(|e| e.to_string());
    #[cfg(not(bundled))]
    {
        let _ = (dir, progress);
        Ok(false)
    }
}

/// Copies this exe to `target`. A copy already there (running, during an update) is renamed to
/// `Chyk.old.exe` first, as the updater does; it's deleted on a later launch.
fn copy_exe(target: &Path) -> std::io::Result<()> {
    let me = std::env::current_exe()?;
    if same_file(&me, target) {
        return Ok(());
    }
    if let Some(dir) = target.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let old = target.with_extension("old.exe");
    let moved = target.exists() && {
        let _ = std::fs::remove_file(&old);
        std::fs::rename(target, &old)?;
        true
    };
    std::fs::copy(&me, target).map(drop).inspect_err(|_| {
        if moved {
            let _ = std::fs::rename(&old, target);
        }
    })
}

/// Copies this exe into `c.dir`, unpacks its libraries and registers it, reporting each step.
/// Ends with [`Msg::Done`].
pub fn install(c: Choices, tx: Sender<Msg>) {
    let total = 5 + usize::from(c.desktop) + usize::from(c.menu);
    let mut r = Report::new(&tx, total);
    let done = run_install(&c, &mut r)
        .map_err(|e| r.line(Kind::Fail, e))
        .ok();
    let _ = tx.send(Msg::Done(done));
}

fn run_install(c: &Choices, r: &mut Report) -> Result<PathBuf, String> {
    let scope = c.scope;
    let target = c.dir.join(EXE);
    if c.fresh {
        // A copy still in use stays, and is checked against its hash like any other.
        for name in libraries() {
            let _ = std::fs::remove_file(c.dir.join(name));
        }
        if !c.desktop
            && let Some(link) = desktop_link(scope)
        {
            let _ = std::fs::remove_file(link);
        }
        if !c.menu {
            let _ = match scope {
                Scope::Machine => crate::shell_menu::remove_all_users(),
                Scope::User => crate::shell_menu::set(false).map(drop),
            };
        }
    }
    copy_exe(&target).map_err(|e| t!("setup.log_copy_failed", error = e))?;
    r.line(Kind::Ok, t!("setup.log_copy", path = target.display()));

    match unpack(&c.dir, &mut |share| r.partial(share)) {
        Ok(true) => r.line(Kind::Ok, t!("setup.log_unpack")),
        Ok(false) => r.line(Kind::Warn, t!("setup.log_unpack_none")),
        Err(e) => return Err(t!("setup.log_unpack_failed", error = e)),
    }
    // The Windows 11 menu's files, so each user can register it later without writing here.
    let _ = crate::shell_menu::write_package_files(&c.dir);

    let start = start_menu_link(scope).ok_or("No Start menu folder")?;
    win::shortcut(&start, &target)?;
    r.line(Kind::Ok, t!("setup.log_start"));

    if c.desktop {
        match desktop_link(scope).map(|l| win::shortcut(&l, &target)) {
            Some(Ok(())) => r.line(Kind::Ok, t!("setup.log_desktop")),
            _ => r.line(Kind::Warn, t!("setup.log_desktop_failed")),
        }
    }

    if c.menu {
        // The installed copy registers itself, so the menu points at it, not at this download.
        let mut cmd = Command::new(&target);
        cmd.arg("--register");
        if scope == Scope::Machine {
            cmd.arg("--all-users");
        }
        match cmd.status().ok().and_then(|s| s.code()) {
            Some(0) => r.line(Kind::Ok, t!("setup.log_menu")),
            Some(2) => r.line(Kind::Warn, t!("setup.log_menu_classic")),
            Some(3) => r.line(Kind::Warn, t!("setup.log_menu_win11_failed")),
            _ => r.line(Kind::Warn, t!("setup.log_menu_failed")),
        }
    }

    let res = assoc_values(&target)
        .into_iter()
        .try_for_each(|(k, n, d)| win::put(scope, &k, &n, &Value::Str(d)));
    win::notify();
    match res {
        Ok(()) => r.line(Kind::Ok, t!("setup.log_assoc")),
        Err(_) => r.line(Kind::Warn, t!("setup.log_assoc_failed")),
    }

    // Repair can turn it back on, so write it either way.
    set_check_updates(c.updates);

    for (name, value) in uninstall_values(&target, size_kb(&c.dir)) {
        win::put(scope, UNINSTALL_KEY, name, &value)?;
    }
    // A setup from before scopes remembered "Run without installing" here.
    let _ = win::delete_tree(Scope::User, r"Software\Chyk\Setup");
    r.line(Kind::Ok, t!("setup.log_uninstall"));
    Ok(target)
}

/// `Chyk-setup.exe --update <folder>`, run by the updater: puts this exe and its libraries into
/// the install at `<folder>` and updates its Installed apps entry, with no window. Returns the
/// exit code (0 on success), or `None` without the flag.
pub fn update_from_args() -> Option<i32> {
    let mut args = std::env::args_os().skip(1);
    if args.next()? != "--update" {
        return None;
    }
    let dir = PathBuf::from(args.next()?);
    Some(match update(&dir) {
        Ok(()) => 0,
        Err(_) => 1,
    })
}

fn update(dir: &Path) -> Result<(), String> {
    let target = dir.join(EXE);
    copy_exe(&target).map_err(|e| e.to_string())?;
    unpack(dir, &mut |_| {})?;
    let _ = crate::shell_menu::write_package_files(dir);
    let scope = [Scope::Machine, Scope::User].into_iter().find(|&s| {
        win::get(s, UNINSTALL_KEY, "InstallLocation").is_some_and(|d| same_file(Path::new(&d), dir))
    });
    if let Some(scope) = scope {
        for (name, value) in uninstall_values(&target, size_kb(dir)) {
            if matches!(name, "DisplayVersion" | "EstimatedSize") {
                let _ = win::put(scope, UNINSTALL_KEY, name, &value);
            }
        }
    }
    Ok(())
}

/// Sets the update check switch in Chyk's settings file: `%APPDATA%\Chyk\settings.toml` (flat
/// TOML). A settings.txt from before the switch to TOML is edited instead while there is no
/// settings.toml yet, since Chyk reads it only then and moves it to TOML on its next save.
fn set_check_updates(on: bool) {
    let Some(dir) = env_dir("APPDATA").map(|d| d.join("Chyk")) else {
        return;
    };
    let (toml, txt) = (dir.join("settings.toml"), dir.join("settings.txt"));
    let (path, value) = if !toml.exists() && txt.exists() {
        (txt, if on { "1" } else { "0" })
    } else {
        (toml, if on { "true" } else { "false" })
    };
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(&path, with_flag(&old, "check_updates", value));
}

/// Removes everything [`install`] added: from the registered install, even when another copy
/// runs this. An all-users install needs this process elevated (the window restarts it so).
/// If a registration can't be removed, the Installed apps entry and the files stay so uninstall
/// can run again. The exe and the libraries stay locked until Chyk exits, so a hidden `cmd`
/// deletes them once it has. `user_data`: also this account's settings, cache and registry key
/// (see [`clean_user`]). Ends with [`Msg::Done`].
pub fn uninstall(keep_settings: bool, user_data: bool, tx: Sender<Msg>) {
    let mut r = Report::new(&tx, 4);
    let me = std::env::current_exe().unwrap_or_default();
    let (exe, scope) = installed().map_or_else(
        || (me.clone(), Scope::User),
        |(dir, scope)| (dir.join(EXE), scope),
    );
    let mut errors = Vec::new();

    // PowerShell for the Windows 11 package: the slow part. Without `user_data` this is an
    // elevated worker, maybe another account: only the all-users entry, `clean_user` does the rest.
    let menu = if user_data {
        match crate::shell_menu::set_for(false, scope == Scope::Machine) {
            Ok(crate::shell_menu::Win11::Failed(e)) | Err(e) => Err(e),
            Ok(_) => Ok(()),
        }
    } else {
        crate::shell_menu::remove_all_users()
    };
    errors.extend(menu.err());
    r.line(Kind::Ok, t!("setup.rm_menu"));

    for (key, name, _) in assoc_values(&exe) {
        errors.extend(win::delete_value(scope, &key, &name).err());
    }
    for key in OWNED {
        errors.extend(win::delete_tree(scope, key).err());
    }
    win::notify();
    r.line(Kind::Ok, t!("setup.rm_assoc"));

    // The Installed apps entry goes last, so a failure anywhere leaves a way to run this again.
    let removed = errors.into_iter().next().map_or_else(
        || {
            if user_data {
                win::delete_tree(Scope::User, r"Software\Chyk")
            } else {
                Ok(())
            }
            .and_then(|()| win::delete_tree(scope, UNINSTALL_KEY))
        },
        Err,
    );
    if let Err(e) = removed {
        r.line(Kind::Fail, t!("setup.rm_failed", error = e));
        let _ = tx.send(Msg::Done(None));
        return;
    }
    for link in [start_menu_link(scope), desktop_link(scope)]
        .into_iter()
        .flatten()
    {
        let _ = std::fs::remove_file(link);
    }
    r.line(Kind::Ok, t!("setup.rm_app"));

    let dir = exe.parent().map(Path::to_path_buf).unwrap_or_default();
    let mut files = vec![exe.clone(), exe.with_extension("old.exe")];
    for name in libraries().chain(["AppxManifest.xml", "chyk.png"]) {
        let path = dir.join(name);
        files.push(path.clone());
        let mut old = path.into_os_string();
        old.push(".old");
        files.push(old.into());
    }
    // A setup started from elsewhere (Downloads) stays: it isn't part of the install.
    files.retain(|f| f.exists());
    let whole = if user_data {
        user_dirs(keep_settings)
    } else {
        Vec::new()
    };
    run_later(cleanup_script(&files, &[dir], &whole));
    if user_data {
        r.line(Kind::Ok, t!("setup.rm_cache"));
    }
    let _ = tx.send(Msg::Done(Some(exe)));
}

/// This account's Chyk data: the cache and session, and the settings unless kept.
fn user_dirs(keep_settings: bool) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = env_dir("LOCALAPPDATA")
        .map(|d| d.join("Chyk"))
        .into_iter()
        .collect();
    if !keep_settings {
        dirs.extend(env_dir("APPDATA").map(|d| d.join("Chyk")));
    }
    dirs
}

/// The per-user part of an uninstall, for the window that started an elevated one: UAC may run
/// that copy as another account, whose data isn't the one to remove. Includes this account's menu
/// entries: its HKCU verbs and its Windows 11 package, which is per user.
pub fn clean_user(keep_settings: bool) -> Result<(), String> {
    let menu = match crate::shell_menu::set_for(false, false) {
        Ok(crate::shell_menu::Win11::Failed(e)) | Err(e) => Err(e),
        Ok(_) => Ok(()),
    };
    win::delete_tree(Scope::User, r"Software\Chyk")?;
    run_later(cleanup_script(&[], &[], &user_dirs(keep_settings)));
    menu
}

/// Deletes `file` (the downloaded setup copy) after Chyk exits.
pub fn delete_later(file: &Path) {
    run_later(cleanup_script(&[file.to_path_buf()], &[], &[]));
}

fn run_later((script, env): (String, Vec<(String, std::ffi::OsString)>)) {
    if script.is_empty() {
        return;
    }
    let _ = Command::new(system_exe(r"System32\cmd.exe"))
        .raw_arg(format!("/c {script}"))
        .envs(env)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}

/// A program in the Windows folder by full path: a bare name would find a file of that name
/// beside Chyk.exe first.
fn system_exe(rel: &str) -> PathBuf {
    env_dir("SystemRoot")
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
        .join(rel)
}

/// Starts `exe` with `args`, through the UAC prompt if `elevate`, and waits for its exit code.
pub fn run_and_wait(exe: &Path, args: &[OsString], elevate: bool) -> std::io::Result<u32> {
    win::shell_execute(exe, &command_line(args), elevate, true).map(|c| c.unwrap_or(1))
}

/// Starts this exe elevated (the UAC prompt) with `args`, without waiting. Fails if the prompt is
/// declined.
pub fn restart_elevated(args: &[OsString]) -> std::io::Result<()> {
    let me = std::env::current_exe()?;
    win::shell_execute(&me, &command_line(args), true, false).map(drop)
}

/// Starts `exe` as the signed-in user even from an elevated setup, through Explorer, so Chyk
/// doesn't run as administrator (drag and drop from Explorer fails into an elevated window).
pub fn open_unelevated(exe: &Path) -> std::io::Result<()> {
    if !elevated() {
        return Command::new(exe).spawn().map(drop);
    }
    Command::new(system_exe("explorer.exe"))
        .arg(exe)
        .spawn()
        .map(drop)
}

/// On launch of the installed copy: delete what an update left aside, and after an update, show
/// the new version in Installed apps (an all-users entry only when this process may write it).
pub fn refresh() {
    if cfg!(debug_assertions) {
        return;
    }
    let (Some((dir, scope)), Ok(exe)) = (installed(), std::env::current_exe()) else {
        return;
    };
    if !same_file(&dir.join(EXE), &exe) {
        return;
    }
    #[cfg(bundled)]
    crate::bundle::clean_aside();
    if win::get(scope, UNINSTALL_KEY, "DisplayVersion").is_some_and(|v| v == VERSION) {
        return;
    }
    for (name, value) in uninstall_values(&exe, size_kb(&dir)) {
        if matches!(name, "DisplayVersion" | "EstimatedSize") {
            let _ = win::put(scope, UNINSTALL_KEY, name, &value);
        }
    }
}
