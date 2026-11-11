//! Every keyboard shortcut in one table, and the key caps that show them.

use super::*;

mod table;
#[cfg(test)]
mod tests;

#[allow(unused_imports)] // `label` waits for the sheet
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
    #[allow(dead_code)] // read by the sheet (Task 5)
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

/// The caps of a command's first binding, for this platform.
pub(super) fn cmd_caps(cmd: Cmd) -> Vec<String> {
    first_binding(cmd).map_or_else(Vec::new, |b| caps(b.mods, b.key, cfg!(target_os = "macos")))
}

const CAP_GAP: f32 = 4.0;

fn localized(caps: &[String]) -> Vec<String> {
    let space = |c: &String| {
        if c == "Space" {
            t!("key.space").to_string()
        } else {
            c.clone()
        }
    };
    caps.iter().map(space).collect()
}

/// Width of `caps` side by side.
pub(super) fn caps_width(p: &egui::Painter, caps: &[String]) -> f32 {
    let caps = localized(caps);
    caps.iter().map(|c| keycap_width(p, c)).sum::<f32>()
        + CAP_GAP * caps.len().saturating_sub(1) as f32
}

/// Draw `caps` from `at`; returns the width used.
pub(super) fn paint_caps(p: &egui::Painter, at: Pos2, caps: &[String]) -> f32 {
    let mut x = at.x;
    for c in &localized(caps) {
        x += keycap(p, pos2(x, at.y), c) + CAP_GAP;
    }
    caps_width(p, caps)
}

/// Caps side by side, each drawn by `controls::keycap`.
pub(super) fn keycaps(ui: &mut Ui, caps: &[String]) -> Response {
    let (resp, p) = ui.allocate_painter(vec2(caps_width(ui.painter(), caps), 22.0), Sense::hover());
    paint_caps(&p, resp.rect.min, caps);
    resp
}

/// Hover tooltip: the label, then the caps of each command's first binding.
pub(super) fn tip_cmds(resp: Response, label: &str, cmds: &[Cmd]) -> Response {
    resp.on_hover_ui(|ui| {
        ui.horizontal(|ui| {
            ui.label(label);
            for &c in cmds {
                keycaps(ui, &cmd_caps(c));
            }
        });
    })
}

pub(super) fn tip_keys(resp: Response, label: &str, cmd: Cmd) -> Response {
    tip_cmds(resp, label, &[cmd])
}
