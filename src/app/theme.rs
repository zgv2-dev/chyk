use super::*;

/// Text fonts, Phosphor icons, and the `bold` family.
pub(super) fn fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    // Space Mono for English, Fira Code for Ukrainian (Space Mono has no Cyrillic).
    // Fira Code also covers Cyrillic file names in the English UI. Both OFL, see assets/fonts.
    for (name, data) in [
        (
            "SpaceMono",
            &include_bytes!("../../assets/fonts/SpaceMono-Regular.ttf")[..],
        ),
        (
            "SpaceMonoBold",
            &include_bytes!("../../assets/fonts/SpaceMono-Bold.ttf")[..],
        ),
        (
            "FiraCode",
            &include_bytes!("../../assets/fonts/FiraCode-Regular.ttf")[..],
        ),
        (
            "FiraCodeBold",
            &include_bytes!("../../assets/fonts/FiraCode-Bold.ttf")[..],
        ),
    ] {
        let mut font = egui::FontData::from_static(data);
        if name.starts_with("FiraCode") {
            // Fira Code's glyphs sit higher in their line than Space Mono's, which the layout is
            // tuned to: Ukrainian text would ride above the icons and controls beside it.
            font.tweak.y_offset_factor = 0.25;
        }
        fonts.font_data.insert(name.into(), font.into());
    }
    let (regular, heavy) = match i18n::lang() {
        i18n::Lang::Uk => (["FiraCode"], ["FiraCodeBold"]),
        i18n::Lang::En => (["SpaceMono"], ["SpaceMonoBold"]),
    };
    // Text fonts first, then Phosphor icons: egui's built-in emoji font reuses some of
    // Phosphor's code points, and Phosphor itself has a space glyph, so it can't lead.
    let mut order: Vec<&str> = regular.to_vec();
    for name in ["FiraCode", "phosphor"] {
        if !order.contains(&name) {
            order.push(name);
        }
    }
    for family in [egui::FontFamily::Monospace, egui::FontFamily::Proportional] {
        let list = fonts.families.entry(family).or_default();
        list.retain(|n| !order.contains(&n.as_str()));
        for (i, name) in order.iter().enumerate() {
            list.insert(i, (*name).into());
        }
    }
    let mut bold: Vec<String> = Vec::new();
    for name in heavy.iter().copied().chain(["FiraCodeBold", "phosphor"]) {
        if !bold.iter().any(|n| n == name) {
            bold.push(name.into());
        }
    }
    fonts
        .families
        .insert(egui::FontFamily::Name("bold".into()), bold);
    ctx.set_fonts(fonts);
}

/// Always dark. Forcing the theme matters: with the system preference, a light-mode OS gets
/// black text on the black stage.
pub(super) fn theme(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    // Dark native title bar too, so it doesn't glow white above the video.
    ctx.send_viewport_cmd(egui::ViewportCommand::SetTheme(egui::SystemTheme::Dark));
    ctx.style_mut_of(egui::Theme::Dark, |s| {
        let v = &mut s.visuals;
        v.panel_fill = BG;
        v.window_fill = PANEL;
        v.extreme_bg_color = CARD;
        v.faint_bg_color = CARD;
        v.window_stroke = Stroke::new(1.0, LINE);
        v.window_corner_radius = 12.into();
        v.menu_corner_radius = 8.into();
        v.window_shadow = shadow();
        v.popup_shadow = shadow();
        v.selection.bg_fill = GREEN.gamma_multiply(0.18);
        v.selection.stroke = Stroke::new(1.0, GREEN);
        v.hyperlink_color = GREEN;
        v.override_text_color = None;
        v.weak_text_color = Some(MUTED);
        let w = &mut v.widgets;
        for (wv, fill, stroke, text) in [
            (&mut w.noninteractive, Color32::TRANSPARENT, LINE, TEXT),
            (&mut w.inactive, CARD, LINE, TEXT),
            (
                &mut w.hovered,
                CARD_HI,
                GREEN.gamma_multiply(0.5),
                Color32::WHITE,
            ),
            (&mut w.active, GREEN.gamma_multiply(0.18), GREEN, GREEN),
            (&mut w.open, CARD_HI, GREEN.gamma_multiply(0.7), GREEN),
        ] {
            wv.bg_fill = fill;
            wv.weak_bg_fill = fill;
            wv.bg_stroke = Stroke::new(1.0, stroke);
            wv.fg_stroke = Stroke::new(1.0, text);
            wv.corner_radius = 6.into();
            wv.expansion = 0.0;
        }
        let sp = &mut s.spacing;
        sp.item_spacing = vec2(6.0, 6.0);
        sp.button_padding = vec2(10.0, 4.0);
        sp.menu_margin = egui::Margin::same(6);
        sp.interact_size.y = 28.0;
        // Solid bars take their own width; floating ones are drawn over the content's edge.
        sp.scroll = egui::style::ScrollStyle::solid();
        // The handle in the text colors: the dark fill matched its track and vanished.
        sp.scroll.foreground_color = true;
        for (style, size) in [
            (egui::TextStyle::Small, 10.0),
            (egui::TextStyle::Body, 12.0),
            (egui::TextStyle::Button, 12.0),
            (egui::TextStyle::Monospace, 12.0),
            (egui::TextStyle::Heading, 16.0),
        ] {
            s.text_styles.insert(style, FontId::monospace(size));
        }
    });
}

pub(super) fn bold(size: f32) -> FontId {
    FontId::new(size, egui::FontFamily::Name("bold".into()))
}

pub(super) fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

/// Deep drop shadow under popovers.
pub(super) fn shadow() -> egui::Shadow {
    egui::Shadow {
        offset: [0, 24],
        blur: 60,
        spread: 0,
        color: Color32::from_black_alpha(180),
    }
}

/// Soft green light behind an element, like a phosphor glow.
pub(super) fn glow(color: Color32, blur: u8) -> egui::Shadow {
    egui::Shadow {
        offset: [0, 0],
        blur,
        spread: 0,
        color,
    }
}

/// Draw at `size`. egui's own Ctrl +/- zoom is off, so the setting is the one place it's set.
pub(super) fn set_ui_size(ctx: &egui::Context, size: crate::settings::UiSize) {
    let z = size.zoom(ctx.native_pixels_per_point().unwrap_or(1.0));
    ctx.options_mut(|o| o.zoom_with_keyboard = false);
    if (ctx.zoom_factor() - z).abs() > 1e-3 {
        ctx.set_zoom_factor(z);
    }
}

/// Keep this window's layout when the interface size changes: egui rescales the content but
/// not the window, so a bigger size would leave less room in points. Resize it by as much,
/// with its minimum (`min`, in points), staying within 90% of the monitor.
pub(super) fn follow_ui_size(ctx: &egui::Context, min: Vec2) {
    let id = egui::Id::new("chyk-ui-size").with(ctx.viewport_id());
    let z = ctx.zoom_factor();
    let (size, monitor) = ctx.input(|i| {
        let v = i.viewport();
        (v.inner_rect.map(|r| r.size()), v.monitor_size)
    });
    let Some(size) = size else { return };
    let seen = ctx.data(|d| d.get_temp::<(f32, Vec2)>(id));
    if let Some((was, was_size)) = seen
        && (was - z).abs() > 1e-3
    {
        // The window's current monitor, measured in points at the zoom the input was taken
        // with (`was`): in points at the new zoom it's `was / z` of that.
        let room = monitor.map_or(Vec2::INFINITY, |m| m * (0.9 * was / z));
        ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(min.min(room)));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(was_size.min(room)));
        // Measured again once the window has its new size.
        ctx.data_mut(|d| d.remove::<(f32, Vec2)>(id));
        return;
    }
    ctx.data_mut(|d| d.insert_temp(id, (z, size)));
}
