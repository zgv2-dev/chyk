use super::*;

impl App {
    /// Start screen: a clickable drop target over faint digital rain, and the shortcuts below it.
    pub(super) fn empty(&mut self, ui: &mut Ui) {
        let full = ui.max_rect();
        let p = ui.painter_at(full);
        p.rect_filled(full, 0.0, BG);

        let hot = ui.input(|i| !i.raw.hovered_files.is_empty());
        // A file the system names (not on Wayland) that isn't a video: grayed out, it won't open.
        let refused = ui.input(|i| {
            i.raw
                .hovered_files
                .first()
                .and_then(|f| f.path.as_deref())
                .is_some_and(|p| !videos::is_video(p))
        });
        let hot = hot && !refused;
        let loading = self.loading.is_some();
        // 16:9 when there is room; on short windows the box gives up height so the legend fits.
        let w = (full.width() - 120.0).clamp(320.0, 560.0);
        let h = (w * 9.0 / 16.0)
            .min(full.height() - 40.0 - 26.0 - LEGEND_H)
            .max(170.0);
        let block = h + 26.0 + LEGEND_H;
        let r = Rect::from_min_size(
            pos2(full.center().x - w / 2.0, full.center().y - block / 2.0),
            vec2(w, h),
        );
        // The rain stays out of the drop box and the shortcut legend under it.
        let clear =
            Rect::from_min_max(r.min, pos2(r.max.x, r.max.y + 26.0 + LEGEND_H)).expand(18.0);
        let time = ui.input(|i| i.time);
        rain(&p, full, clear, time);
        // ~30 fps is plenty for falling glyphs, and only while the start screen shows.
        ui.ctx().request_repaint_after(Duration::from_millis(33));
        // A file dragged over it, or the pointer: light the target up, easing in and out.
        let lit = hot || (!loading && ui.rect_contains_pointer(r));
        let k = ui
            .ctx()
            .animate_bool_with_time(ui.id().with("drop_glow"), lit, 0.2);
        drop_box(ui, &p, r, (hot, refused), loading, k);
        let click = ui.interact(r, ui.id().with("drop"), Sense::click());
        if click.clicked() && !loading {
            self.open_dialog();
        }
        if click.hovered() && !loading {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        click.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, t!("open.drop"))
        });
        legend(ui, &p, r);
    }
}

/// The drop target: a glowing card with a dashed edge, viewfinder corners, the icon tile and its
/// text. `k` eases the glow in and out as the target lights up.
/// `refused`: a file that isn't a video is held over it, so it all goes gray.
fn drop_box(ui: &mut Ui, p: &egui::Painter, r: Rect, hover: (bool, bool), loading: bool, k: f32) {
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
    let inset = r.shrink(14.0);
    for seg in [
        [pos2(inset.min.x, r.min.y), pos2(inset.max.x, r.min.y)],
        [pos2(inset.min.x, r.max.y), pos2(inset.max.x, r.max.y)],
        [pos2(r.min.x, inset.min.y), pos2(r.min.x, inset.max.y)],
        [pos2(r.max.x, inset.min.y), pos2(r.max.x, inset.max.y)],
    ] {
        p.extend(egui::Shape::dashed_line(
            &seg,
            Stroke::new(1.0, edge),
            4.0,
            4.0,
        ));
    }
    // Viewfinder corners with the same 14 pt rounding as the card.
    for (hx, hy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0f32)] {
        let corner = pos2(
            if hx < 0.0 { r.min.x } else { r.max.x },
            if hy < 0.0 { r.min.y } else { r.max.y },
        );
        let center = corner - vec2(hx, hy) * 14.0;
        let mut pts = vec![corner - vec2(0.0, hy * 26.0)];
        for i in 0..=8 {
            let a = i as f32 / 8.0 * std::f32::consts::FRAC_PI_2;
            pts.push(center + vec2(hx * a.cos(), hy * a.sin()) * 14.0);
        }
        pts.push(corner - vec2(hx * 26.0, 0.0));
        p.line(pts, Stroke::new(3.0, accent));
    }

    let tile = Rect::from_center_size(r.center() - vec2(0.0, 30.0), vec2(72.0, 72.0));
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
    let ty = tile.max.y + 28.0;
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
            pos2(r.center().x, ty + 24.0),
            Align2::CENTER_CENTER,
            t!("open.browse"),
            mono(12.0),
            MUTED,
        );
    }
}
