//! Writes and removes the Linux menu entries (`linux_files`) in the user's XDG folders.
//! File managers that aren't installed just never read theirs.

use super::exts;
use super::linux_files::{self as files, Entry};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// `$XDG_DATA_HOME` or `~/.local/share`.
fn data_home() -> Option<PathBuf> {
    xdg("XDG_DATA_HOME", ".local/share")
}

/// `$XDG_CONFIG_HOME` or `~/.config`.
fn config_home() -> Option<PathBuf> {
    xdg("XDG_CONFIG_HOME", ".config")
}

fn xdg(var: &str, fallback: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| Some(PathBuf::from(std::env::var_os("HOME")?).join(fallback)))
}

/// Dolphin's service menu: always written, so it also says whether the entry is on.
fn dolphin_path(data: &Path) -> PathBuf {
    data.join("kio/servicemenus/chyk-edit.desktop")
}

pub fn is_on() -> bool {
    data_home().is_some_and(|d| dolphin_path(&d).exists())
}

/// Whether Nautilus can load Python extensions (the nautilus-python package).
fn nautilus_python() -> bool {
    let roots = ["/usr/lib", "/usr/lib64", "/usr/local/lib"];
    let lib = "libnautilus-python.so";
    let has = |dir: &Path| {
        fs::read_dir(dir.join("nautilus"))
            .is_ok_and(|rd| rd.flatten().any(|e| e.path().join(lib).exists()))
    };
    roots
        .iter()
        .map(Path::new)
        .any(|r| has(r) || fs::read_dir(r).is_ok_and(|rd| rd.flatten().any(|e| has(&e.path()))))
}

fn write(path: &Path, text: &[u8], exec: bool) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::create_dir_all(path.parent().unwrap_or(Path::new("/")))?;
    fs::write(path, text)?;
    // Dolphin (KF6) skips service menus in the home folder that aren't executable.
    let mode = if exec { 0o755 } else { 0o644 };
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

fn remove(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// Chyk's Nautilus scripts, found by their marker line (the file name is the translated title).
fn scripts(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = fs::read_dir(dir) else {
        return Vec::new();
    };
    rd.flatten()
        .map(|e| e.path())
        .filter(|p| fs::read_to_string(p).is_ok_and(|s| s.contains(files::SCRIPT_MARK)))
        .collect()
}

/// Thunar's custom actions: the user's file, else the system default (so its actions stay).
fn uca(config: &Path, e: Option<&Entry>) -> io::Result<()> {
    let thunar = config.join("Thunar");
    if !thunar.is_dir() {
        return Ok(()); // Thunar never ran here.
    }
    let path = thunar.join("uca.xml");
    let doc = fs::read_to_string(&path)
        .or_else(|_| fs::read_to_string("/etc/xdg/Thunar/uca.xml"))
        .unwrap_or_default();
    let new = match e {
        Some(e) => super::thunar::uca_insert(&doc, e, exts()),
        None => super::thunar::uca_remove(&doc),
    };
    if new == doc {
        Ok(())
    } else {
        write(&path, new.as_bytes(), false)
    }
}

/// Add the entries for `exe` titled `label`, or remove them.
pub fn set(on: bool, exe: &Path, label: &str) -> Result<(), String> {
    let data = data_home().ok_or("HOME isn't set")?;
    let config = config_home().ok_or("HOME isn't set")?;
    let icon = data.join("chyk/chyk.png");
    let dolphin = dolphin_path(&data);
    let nemo = data.join("nemo/actions/chyk-edit.nemo_action");
    let fm = data.join("file-manager/actions/chyk-edit.desktop");
    let py = data.join("nautilus-python/extensions/chyk_edit.py");
    let script_dir = data.join("nautilus/scripts");
    // Only for the AppImage: the .deb and AUR packages install their own.
    let open_with = data.join("applications/chyk.desktop");
    let appimage = std::env::var_os("APPIMAGE").is_some();
    let e = Entry {
        exe,
        label,
        icon: &icon,
    };
    let r = (|| -> io::Result<()> {
        for s in scripts(&script_dir) {
            remove(&s)?;
        }
        if !on {
            for p in [&dolphin, &nemo, &fm, &py, &icon] {
                remove(p)?;
            }
            if appimage {
                remove(&open_with)?;
            }
            return uca(&config, None);
        }
        write(&icon, include_bytes!("../../assets/chyk.png"), false)?;
        write(&dolphin, files::dolphin(&e).as_bytes(), true)?;
        write(&nemo, files::nemo(&e, exts()).as_bytes(), false)?;
        write(&fm, files::fm_action(&e).as_bytes(), false)?;
        if nautilus_python() {
            write(&py, files::nautilus_py(&e, exts()).as_bytes(), false)?;
        } else {
            remove(&py)?;
            let name = label.replace('/', "-");
            write(
                &script_dir.join(name),
                files::nautilus_script(&e).as_bytes(),
                true,
            )?;
        }
        if appimage {
            write(&open_with, files::open_with(&e).as_bytes(), false)?;
        }
        uca(&config, Some(&e))
    })();
    if appimage {
        // Best effort: Open With also works without the cache, just later.
        let _ = std::process::Command::new("update-desktop-database")
            .arg(data.join("applications"))
            .output();
    }
    r.map_err(|e| e.to_string())
}

/// On launch: rewrite the entries if they start another path (the AppImage moved or updated).
pub fn refresh(exe: &Path, label: &str) {
    let Some(text) = data_home().and_then(|d| fs::read_to_string(dolphin_path(&d)).ok()) else {
        return;
    };
    if !text.contains(&format!("Exec={} %f", files::exec_arg(exe))) {
        let _ = set(true, exe, label);
    }
}
