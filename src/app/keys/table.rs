//! The shortcut table and the command names.

use super::*;

pub(in crate::app) fn bindings() -> Vec<Binding> {
    use Cmd::*;
    use Group as G;
    let (n, sh, cmd) = (Modifiers::NONE, Modifiers::SHIFT, Modifiers::COMMAND);
    let mut v = vec![
        (PlayPause, n, Key::Space, G::Playback),
        (FrameBack, n, Key::ArrowLeft, G::Playback),
        (FrameNext, n, Key::ArrowRight, G::Playback),
        (Back5, sh, Key::ArrowLeft, G::Playback),
        (Forward5, sh, Key::ArrowRight, G::Playback),
        (Slower, n, Key::Comma, G::Playback),
        (Slower, sh, Key::Comma, G::Playback),
        (Faster, n, Key::Period, G::Playback),
        (Faster, sh, Key::Period, G::Playback),
        (SetIn, n, Key::I, G::Trim),
        (SetOut, n, Key::O, G::Trim),
        (GoIn, sh, Key::I, G::Trim),
        (GoOut, sh, Key::O, G::Trim),
        (ClearSelection, n, Key::X, G::Trim),
        (Crop, n, Key::C, G::Crop),
        (RemoveCrop, sh, Key::C, G::Crop),
        (Strip, n, Key::S, G::Strip),
        (AddZone, n, Key::A, G::Strip),
        (EditZones, n, Key::Z, G::Strip),
        (DeleteZone, n, Key::Backspace, G::Strip),
        (DeleteZone, n, Key::Delete, G::Strip),
        (Rescan, n, Key::R, G::Strip),
        (StripOff, sh, Key::S, G::Strip),
        (Undo, cmd, Key::Z, G::Edit),
        (Redo, cmd | sh, Key::Z, G::Edit),
        (ZoomIn, n, Key::Plus, G::View),
        (ZoomIn, n, Key::Equals, G::View),
        (ZoomIn, sh, Key::Plus, G::View),
        (ZoomOut, n, Key::Minus, G::View),
        (ZoomFit, n, Key::Num0, G::View),
        (TimelineOut, n, Key::OpenBracket, G::View),
        (TimelineIn, n, Key::CloseBracket, G::View),
        (TimelineToSelection, n, Key::Backslash, G::View),
        (TimelineWhole, sh, Key::Backslash, G::View),
        (Open, cmd, Key::O, G::File),
        (Export, cmd, Key::S, G::File),
        (Info, cmd, Key::I, G::File),
        (CloseVideo, cmd, Key::W, G::File),
        (SaveFrame, cmd | sh, Key::E, G::File),
        (Shortcuts, n, Key::Questionmark, G::File),
        (RevealExport, cmd | sh, Key::R, G::File),
        (ToggleLog, cmd, Key::L, G::File),
    ];
    if !cfg!(target_os = "macos") {
        v.push((Redo, Modifiers::CTRL, Key::Y, G::Edit));
    }
    let presets = [
        Key::Num1,
        Key::Num2,
        Key::Num3,
        Key::Num4,
        Key::Num5,
        Key::Num6,
    ];
    v.extend(
        presets
            .iter()
            .enumerate()
            .map(|(i, &k)| (CropPreset(i), n, k, G::Crop)),
    );
    v.extend(
        presets[..5]
            .iter()
            .enumerate()
            .map(|(i, &k)| (OpenRecent(i), cmd, k, G::File)),
    );
    v.into_iter()
        .map(|(cmd, mods, key, group)| Binding {
            cmd,
            mods,
            key,
            group,
        })
        .collect()
}

/// i18n key of the command's name.
#[allow(dead_code)] // used by the sheet (Task 5)
pub(in crate::app) fn label(cmd: Cmd) -> &'static str {
    use Cmd::*;
    match cmd {
        PlayPause => "cmd.play_pause",
        FrameBack => "cmd.frame_back",
        FrameNext => "cmd.frame_next",
        Back5 => "cmd.back5",
        Forward5 => "cmd.forward5",
        Slower => "cmd.slower",
        Faster => "cmd.faster",
        SetIn => "cmd.set_in",
        SetOut => "cmd.set_out",
        GoIn => "cmd.go_in",
        GoOut => "cmd.go_out",
        ClearSelection => "cmd.clear_selection",
        Crop => "cmd.crop",
        CropPreset(_) => "cmd.crop_preset",
        RemoveCrop => "cmd.remove_crop",
        Strip => "cmd.strip",
        AddZone => "cmd.add_zone",
        EditZones => "cmd.edit_zones",
        DeleteZone => "cmd.delete_zone",
        Rescan => "cmd.rescan",
        StripOff => "cmd.strip_off",
        Undo => "cmd.undo",
        Redo => "cmd.redo",
        ZoomIn => "cmd.zoom_in",
        ZoomOut => "cmd.zoom_out",
        ZoomFit => "cmd.zoom_fit",
        TimelineIn => "cmd.timeline_in",
        TimelineOut => "cmd.timeline_out",
        TimelineToSelection => "cmd.timeline_to_selection",
        TimelineWhole => "cmd.timeline_whole",
        Open => "cmd.open",
        Export => "cmd.export",
        Info => "cmd.info",
        CloseVideo => "cmd.close_video",
        SaveFrame => "cmd.save_frame",
        Shortcuts => "cmd.shortcuts",
        RevealExport => "cmd.reveal_export",
        OpenRecent(_) => "cmd.open_recent",
        ToggleLog => "cmd.toggle_log",
    }
}
