//! The recently opened files: a small text file in the config folder, and a thumbnail cache.

use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;

const KEEP: usize = 10;

#[derive(Clone, Debug, PartialEq)]
pub struct Recent {
    pub path: PathBuf,
    /// Unix seconds.
    pub opened: u64,
    pub codec: String,
    pub size: [u32; 2],
    pub dur: f64,
}

/// One file per line, `opened\tdur\tWxH\tcodec\tpath`. Broken lines are skipped.
pub fn parse(text: &str) -> Vec<Recent> {
    text.lines().filter_map(parse_line).collect()
}

fn parse_line(line: &str) -> Option<Recent> {
    let mut f = line.splitn(5, '\t');
    let opened = f.next()?.parse().ok()?;
    let dur: f64 = f.next()?.parse().ok()?;
    let (w, h) = f.next()?.split_once('x')?;
    let codec = f.next()?.to_string();
    let path = f.next().filter(|p| !p.is_empty())?;
    Some(Recent {
        path: path.into(),
        opened,
        codec,
        size: [w.parse().ok()?, h.parse().ok()?],
        dur,
    })
}

pub fn format(list: &[Recent]) -> String {
    list.iter()
        .map(|r| {
            let [w, h] = r.size;
            let codec = r.codec.replace(['\t', '\n'], " ");
            let path = r.path.to_string_lossy();
            format!("{}\t{}\t{w}x{h}\t{codec}\t{path}\n", r.opened, r.dur)
        })
        .collect()
}

/// Newest first, one entry per path, at most ten.
pub fn push(list: &mut Vec<Recent>, item: Recent) {
    list.retain(|r| r.path != item.path);
    list.insert(0, item);
    list.truncate(KEEP);
}

pub fn config_dir() -> Option<PathBuf> {
    let env = |k| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    Some(if cfg!(target_os = "macos") {
        env("HOME")?.join("Library/Application Support/Chyk")
    } else if cfg!(windows) {
        env("APPDATA")?.join("Chyk")
    } else {
        env("XDG_CONFIG_HOME")
            .or_else(|| env("HOME").map(|h| h.join(".config")))?
            .join("chyk")
    })
}

fn file() -> Option<PathBuf> {
    Some(config_dir()?.join("recent"))
}

pub fn load() -> Vec<Recent> {
    file()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .map(|t| parse(&t))
        .unwrap_or_default()
}

pub fn save(list: &[Recent]) -> std::io::Result<()> {
    let file = file().ok_or_else(|| std::io::Error::other("no config folder"))?;
    save_to(&file, list)
}

fn save_to(file: &Path, list: &[Recent]) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(file, format(list))
}

/// Where the thumbnail of `path` is cached.
pub fn thumb_path(path: &Path) -> Option<PathBuf> {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut h = DefaultHasher::new();
    path.hash(&mut h);
    Some(
        crate::update::cache_base()?
            .join("recent")
            .join(format!("{:x}.rgba", h.finish())),
    )
}

/// 8-byte header (width, height as u32 LE), then RGBA.
fn encode_thumb(size: [u32; 2], rgba: &[u8]) -> Vec<u8> {
    let mut out = [size[0].to_le_bytes(), size[1].to_le_bytes()].concat();
    out.extend_from_slice(rgba);
    out
}

fn decode_thumb(bytes: &[u8]) -> Option<([u32; 2], Vec<u8>)> {
    let (w, h) = (
        u32::from_le_bytes(bytes.get(..4)?.try_into().ok()?),
        u32::from_le_bytes(bytes.get(4..8)?.try_into().ok()?),
    );
    let rgba = bytes.get(8..)?;
    (rgba.len() == w as usize * h as usize * 4).then(|| ([w, h], rgba.to_vec()))
}

/// Cache a thumbnail; errors are ignored.
pub fn save_thumb(path: &Path, size: [u32; 2], rgba: &[u8]) {
    let Some(t) = thumb_path(path) else { return };
    if let Some(dir) = t.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(t, encode_thumb(size, rgba));
}

pub fn load_thumb(path: &Path) -> Option<([u32; 2], Vec<u8>)> {
    decode_thumb(&std::fs::read(thumb_path(path)?).ok()?)
}
