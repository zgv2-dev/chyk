//! Install and uninstall, run on a background thread by the setup window.

use super::*;
use crate::i18n::t;
use std::os::windows::process::CommandExt;
use std::process::Command;

/// No console window for the helper processes.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// The registered install folder, if its exe is still there.
pub(super) fn installed() -> Option<PathBuf> {
    let dir = PathBuf::from(win::get(UNINSTALL_KEY, "InstallLocation")?);
    dir.join(EXE).is_file().then_some(dir)
}

pub(super) fn skipped() -> bool {
    win::get(SETUP_KEY, "Portable").is_some()
}

/// "Run without installing": don't offer setup again for this user.
pub fn skip() {
    let _ = win::put(SETUP_KEY, "Portable", &Value::Str("1".into()));
}

fn env_dir(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

fn start_menu_link() -> Option<PathBuf> {
    Some(env_dir("APPDATA")?.join(r"Microsoft\Windows\Start Menu\Programs\Chyk.lnk"))
}

fn desktop_link() -> Option<PathBuf> {
    win::desktop().ok().map(|d| d.join("Chyk.lnk"))
}

fn size_kb(exe: &Path) -> u32 {
    std::fs::metadata(exe).map_or(0, |m| (m.len() / 1024) as u32)
}

/// Copies this exe into `c.dir` and registers it, reporting each step. Ends with [`Msg::Done`].
pub fn install(c: Choices, tx: Sender<Msg>) {
    let total = 3 + usize::from(c.desktop) + usize::from(c.menu) + usize::from(c.assoc);
    let mut r = Report::new(&tx, total);
    let done = run_install(&c, &mut r)
        .map_err(|e| r.line(Kind::Fail, e))
        .ok();
    let _ = tx.send(Msg::Done(done));
}

fn run_install(c: &Choices, r: &mut Report) -> Result<PathBuf, String> {
    let target = c.dir.join(EXE);
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    if !same_file(&me, &target) {
        std::fs::create_dir_all(&c.dir)
            .and_then(|()| std::fs::copy(&me, &target))
            .map_err(|e| t!("setup.log_copy_failed", error = e))?;
    }
    r.line(Kind::Ok, t!("setup.log_copy", path = target.display()));

    let start = start_menu_link().ok_or("APPDATA is not set")?;
    win::shortcut(&start, &target)?;
    r.line(Kind::Ok, t!("setup.log_start"));

    if c.desktop {
        match desktop_link().map(|l| win::shortcut(&l, &target)) {
            Some(Ok(())) => r.line(Kind::Ok, t!("setup.log_desktop")),
            _ => r.line(Kind::Warn, t!("setup.log_desktop_failed")),
        }
    }

    if c.menu {
        // The installed copy registers itself, so the menu points at it, not at this download.
        let code = Command::new(&target)
            .arg("--register")
            .status()
            .ok()
            .and_then(|s| s.code());
        match code {
            Some(0) => r.line(Kind::Ok, t!("setup.log_menu")),
            Some(2) => r.line(Kind::Warn, t!("setup.log_menu_classic")),
            Some(3) => r.line(Kind::Warn, t!("setup.log_menu_win11_failed")),
            _ => r.line(Kind::Warn, t!("setup.log_menu_failed")),
        }
    }

    if c.assoc {
        let res = assoc_values(&target)
            .into_iter()
            .try_for_each(|(k, n, d)| win::put(&k, &n, &Value::Str(d)));
        win::notify();
        match res {
            Ok(()) => {
                // Only the user can make Chyk the default; open its page in Default apps.
                let _ = Command::new(system_exe("explorer.exe"))
                    .arg("ms-settings:defaultapps?registeredAppUser=Chyk")
                    .spawn();
                r.line(Kind::Ok, t!("setup.log_assoc"));
            }
            Err(_) => r.line(Kind::Warn, t!("setup.log_assoc_failed")),
        }
    }

    if !c.updates {
        set_check_updates(false);
    }

    for (name, value) in uninstall_values(&target, size_kb(&target)) {
        win::put(UNINSTALL_KEY, name, &value)?;
    }
    let _ = win::delete_tree(SETUP_KEY);
    r.line(Kind::Ok, t!("setup.log_uninstall"));
    Ok(target)
}

/// Chyk's settings file (`%APPDATA%\Chyk\settings.txt`, `key = value` lines), where the update
/// check switch lives.
fn set_check_updates(on: bool) {
    let Some(path) = env_dir("APPDATA").map(|d| d.join(r"Chyk\settings.txt")) else {
        return;
    };
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&path, with_flag(&old, "check_updates", on));
}

/// Removes everything [`install`] added: from the registered install, even when another copy
/// runs this. If a registration can't be removed, the Installed apps entry and the files stay so
/// uninstall can run again. The exe and the folders Chyk uses stay locked until it exits, so a
/// hidden `cmd` deletes them once it has. Ends with [`Msg::Done`].
pub fn uninstall(keep_settings: bool, tx: Sender<Msg>) {
    let mut r = Report::new(&tx, 4);
    let me = std::env::current_exe().unwrap_or_default();
    let exe = installed().map_or_else(|| me.clone(), |dir| dir.join(EXE));
    let mut errors = Vec::new();

    // PowerShell for the Windows 11 package: the slow part.
    match crate::shell_menu::set(false) {
        Ok(crate::shell_menu::Win11::Failed(e)) | Err(e) => errors.push(e),
        Ok(_) => {}
    }
    r.line(Kind::Ok, t!("setup.rm_menu"));

    for (key, name, _) in assoc_values(&exe) {
        errors.extend(win::delete_value(&key, &name).err());
    }
    for key in OWNED {
        errors.extend(win::delete_tree(key).err());
    }
    win::notify();
    r.line(Kind::Ok, t!("setup.rm_assoc"));

    if let Some(e) = errors.into_iter().next() {
        r.line(Kind::Fail, t!("setup.rm_failed", error = e));
        let _ = tx.send(Msg::Done(None));
        return;
    }
    for link in [start_menu_link(), desktop_link()].into_iter().flatten() {
        let _ = std::fs::remove_file(link);
    }
    let _ = win::delete_tree(UNINSTALL_KEY);
    let _ = win::delete_tree(r"Software\Chyk");
    r.line(Kind::Ok, t!("setup.rm_app"));

    let mut files = vec![exe.clone(), exe.with_extension("old.exe")];
    if !same_file(&me, &exe) {
        files.push(me);
    }
    let dir: Vec<PathBuf> = exe.parent().map(Path::to_path_buf).into_iter().collect();
    let mut whole: Vec<PathBuf> = env_dir("LOCALAPPDATA")
        .map(|d| d.join("Chyk"))
        .into_iter()
        .collect();
    if !keep_settings {
        whole.extend(env_dir("APPDATA").map(|d| d.join("Chyk")));
    }
    run_later(cleanup_script(&files, &dir, &whole));
    r.line(Kind::Ok, t!("setup.rm_cache"));
    let _ = tx.send(Msg::Done(Some(exe)));
}

/// Deletes `file` (the downloaded setup copy) after Chyk exits.
pub fn delete_later(file: &Path) {
    run_later(cleanup_script(&[file.to_path_buf()], &[], &[]));
}

fn run_later((script, env): (String, Vec<(String, std::ffi::OsString)>)) {
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

/// On launch of the installed copy: after an update, show the new version in Installed apps.
pub fn refresh() {
    if cfg!(debug_assertions) {
        return;
    }
    let (Some(dir), Ok(exe)) = (installed(), std::env::current_exe()) else {
        return;
    };
    if !same_file(&dir.join(EXE), &exe)
        || win::get(UNINSTALL_KEY, "DisplayVersion").is_some_and(|v| v == VERSION)
    {
        return;
    }
    for (name, value) in uninstall_values(&exe, size_kb(&exe)) {
        if matches!(name, "DisplayVersion" | "EstimatedSize") {
            let _ = win::put(UNINSTALL_KEY, name, &value);
        }
    }
}
