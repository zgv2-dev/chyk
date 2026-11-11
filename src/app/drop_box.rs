use super::*;

/// The drop target: a glowing card with a dashed edge, the icon tile and its
/// text. `k` eases the glow in and out as the target lights up.
/// `refused`: a file that isn't a video is held over it, so it all goes gray.
pub(super) fn drop_box(
    ui: &mut Ui,
    p: &egui::Painter,
    r: Rect,
    hover: (bool, bool),
    loading: bool,
    k: f32,
) {
    let (hot, refused) = hover;
    let mix = |a: f32, b: f32| a + (b - a) * k;
    // Gray stands in for green everywhere while a non-video is over the box.
    let gray = Color32::from_gray(96);
    let accent = if refused { gray } else { GREEN };
    if !refused {
        p.add(glow(GREEN.gamma_multiply(mix(0.06, 0.14)), 80).as_shape(r, 14));
    }
    p.rect_filled(r, 14.0, Color32::from_rgba_premultiplied(3, 9, 6, 235));
    let edge = if hot {
        GREEN
    } else if refused {
        gray.gamma_multiply(0.7)
    } else {
        Color32::from_rgb(29, 74, 49)
    };
    // One path round the card's edge, its rounded corners included, so the dashes don't stop
    // short of them. Clockwise from the top of the top-right corner.
    let c = r.shrink(14.0);
    let mut edge_path = Vec::new();
    for (corner, from) in [
        (c.right_top(), -90.0_f32),
        (c.right_bottom(), 0.0),
        (c.left_bottom(), 90.0),
        (c.left_top(), 180.0),
    ] {
        for i in 0..=8 {
            let a = (from + i as f32 * 90.0 / 8.0).to_radians();
            edge_path.push(corner + 14.0 * vec2(a.cos(), a.sin()));
        }
    }
    edge_path.push(edge_path[0]);
    p.extend(egui::Shape::dashed_line(
        &edge_path,
        Stroke::new(1.0, edge),
        4.0,
        4.0,
    ));
    let tile = Rect::from_center_size(pos2(r.center().x, stack_top(r) + 36.0), vec2(72.0, 72.0));
    if !refused {
        let tile_glow = glow(GREEN.gamma_multiply(mix(0.15, 0.35)), mix(30.0, 40.0) as u8);
        p.add(tile_glow.as_shape(tile, 18));
    }
    p.rect(
        tile,
        18.0,
        CARD,
        Stroke::new(1.0, LINE),
        egui::StrokeKind::Inside,
    );
    if loading {
        ui.put(
            tile.shrink(22.0),
            egui::Spinner::new().size(28.0).color(GREEN),
        );
    } else {
        let glyph = match (hot, refused) {
            (true, _) => icon::DOWNLOAD_SIMPLE,
            (_, true) => icon::PROHIBIT,
            _ => icon::FILM_SLATE,
        };
        p.text(
            tile.center(),
            Align2::CENTER_CENTER,
            glyph,
            mono(34.0),
            accent,
        );
    }
    let title = match (loading, hot, refused) {
        (true, _, _) => t!("open.opening"),
        (_, true, _) => t!("open.release"),
        (_, _, true) => t!("open.not_video"),
        _ => t!("open.drop"),
    };
    let ty = tile.max.y + 26.0;
    p.text(
        pos2(r.center().x, ty),
        Align2::CENTER_CENTER,
        title,
        bold(17.0),
        match (hot, refused) {
            (true, _) => GREEN,
            (_, true) => gray,
            _ => Color32::WHITE,
        },
    );
    if !loading && !hot && !refused {
        p.text(
            pos2(r.center().x, ty + 22.0),
            Align2::CENTER_CENTER,
            t!("open.browse"),
            mono(12.0),
            MUTED,
        );
    }
}

/// The icon, title, hint and button stack (72 + 16 + 20 + 4 + 14 + 14 + 28 pt) centered in the box.
fn stack_top(r: Rect) -> f32 {
    r.center().y - 84.0
}

/// Top of the Open button inside the box.
pub(super) fn button_top(r: Rect) -> f32 {
    stack_top(r) + 140.0
}
