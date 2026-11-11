//! Every keyboard shortcut in one table, and the key caps that show them.

use super::*;

mod table;
#[cfg(test)]
mod tests;

#[allow(unused_imports)] // unused until the sheet lands
pub(super) use table::{bindings, label};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Cmd {
    PlayPause,
    FrameBack,
    FrameNext,
    Back5,
    Forward5,
    Slower,
    Faster,
    SetIn,
    SetOut,
    GoIn,
    GoOut,
    ClearSelection,
    Crop,
    CropPreset(usize),
    RemoveCrop,
    Strip,
    AddZone,
    EditZones,
    DeleteZone,
    Rescan,
    StripOff,
    Undo,
    Redo,
    ZoomIn,
    ZoomOut,
    ZoomFit,
    TimelineIn,
    TimelineOut,
    TimelineToSelection,
    TimelineWhole,
    Open,
    Export,
    Info,
    CloseVideo,
    SaveFrame,
    Shortcuts,
    RevealExport,
    OpenRecent(usize),
    ToggleLog,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Group {
    Playback,
    Trim,
    Crop,
    Strip,
    Edit,
    View,
    File,
}

#[derive(Clone, Copy)]
pub(super) struct Binding {
    pub(super) cmd: Cmd,
    pub(super) mods: Modifiers,
    pub(super) key: Key,
    pub(super) group: Group,
}

/// The caps of one chord: ⌃⌥⇧⌘ then the key on macOS, Ctrl, Alt, Shift, key elsewhere.
pub(super) fn caps(mods: Modifiers, key: Key, mac: bool) -> Vec<String> {
    let mut v = Vec::new();
    let mut push = |on: bool, s: &str| {
        if on {
            v.push(s.to_string());
        }
    };
    if mac {
        push(mods.ctrl, "⌃");
        push(mods.alt, "⌥");
        push(mods.shift, "⇧");
        push(mods.command || mods.mac_cmd, "⌘");
    } else {
        push(mods.ctrl || mods.command, "Ctrl");
        push(mods.alt, "Alt");
        push(mods.shift, "Shift");
    }
    v.push(
        match key {
            Key::ArrowLeft => "←",
            Key::ArrowRight => "→",
            Key::ArrowUp => "↑",
            Key::ArrowDown => "↓",
            Key::Backspace => "⌫",
            Key::Enter => "⏎",
            Key::Escape => "Esc",
            k => k.symbol_or_name(),
        }
        .to_string(),
    );
    v
}

pub(super) fn first_binding(cmd: Cmd) -> Option<Binding> {
    bindings().into_iter().find(|b| b.cmd == cmd)
}

/// Caps side by side, each drawn by `controls::keycap`.
pub(super) fn keycaps(ui: &mut Ui, caps: &[String]) -> Response {
    let caps: Vec<String> = caps
        .iter()
        .map(|c| {
            if c == "Space" {
                t!("key.space").to_string()
            } else {
                c.clone()
            }
        })
        .collect();
    let gap = 4.0;
    let widths: Vec<f32> = caps
        .iter()
        .map(|c| {
            let w = ui
                .painter()
                .layout_no_wrap(c.clone(), mono(11.0), TEXT)
                .size()
                .x;
            (w + 12.0).max(22.0)
        })
        .collect();
    let total = widths.iter().sum::<f32>() + gap * caps.len().saturating_sub(1) as f32;
    let (resp, p) = ui.allocate_painter(vec2(total, 22.0), Sense::hover());
    let mut x = resp.rect.min.x;
    for c in &caps {
        x += keycap(&p, pos2(x, resp.rect.min.y), c) + gap;
    }
    resp
}
