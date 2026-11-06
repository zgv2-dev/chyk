use super::*;

impl Player {
    /// Draw the normalized part `src` of the current frame into `view`.
    /// The FBO is sized to what the zoomed view needs, so zooming in shows real source pixels.
    pub fn paint(&mut self, ui: &egui::Ui, view: Rect, src: Rect, source: [u32; 2], aspect: f32) {
        let ppp = ui.ctx().pixels_per_point();
        let view_px = (view.width() * ppp, view.height() * ppp);
        if view_px.0 < 1.0 || view_px.1 < 1.0 || src.width() <= 0.0 || src.height() <= 0.0 {
            return;
        }
        let aspect = if aspect.is_finite() && aspect > 0.0 {
            aspect
        } else if source[1] > 0 {
            source[0] as f32 / source[1] as f32
        } else {
            16.0 / 9.0
        };
        let display_w = if source[0] > 0 {
            source[0] as f32
        } else {
            view_px.0
        };
        let display_h = (display_w / aspect).max(1.0);
        let need_w = view_px.0 / src.width();
        let need_h = view_px.1 / src.height();
        let max_tex = ui.input(|i| i.max_texture_side as f32).max(1.0);
        let scale = (need_w / display_w)
            .max(need_h / display_h)
            .min(1.0)
            .min(MAX_RENDER / display_w)
            .min(MAX_RENDER / display_h)
            .min(max_tex / display_w)
            .min(max_tex / display_h);
        let (w, h) = (
            (display_w * scale).round().max(1.0) as i32,
            (display_h * scale).round().max(1.0) as i32,
        );
        let gl = &self.gl;
        let resized = self.fbo.is_none_or(|f| (f.2, f.3) != (w, h));
        // SAFETY: eframe calls `ui` on the thread that owns the GL context.
        unsafe {
            if resized {
                if let Some((fb, tex, ..)) = self.fbo.take() {
                    gl.delete_framebuffer(fb);
                    gl.delete_texture(tex);
                }
                let (Ok(tex), Ok(fb)) = (gl.create_texture(), gl.create_framebuffer()) else {
                    return;
                };
                gl.bind_texture(glow::TEXTURE_2D, Some(tex));
                gl.tex_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    glow::RGBA8 as i32,
                    w,
                    h,
                    0,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(None),
                );
                gl.bind_texture(glow::TEXTURE_2D, None);
                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fb));
                gl.framebuffer_texture_2d(
                    glow::FRAMEBUFFER,
                    glow::COLOR_ATTACHMENT0,
                    glow::TEXTURE_2D,
                    Some(tex),
                    0,
                );
                gl.bind_framebuffer(glow::FRAMEBUFFER, None);
                self.fbo = Some((fb, tex, w, h));
            }
        }
        let Some((fb, ..)) = self.fbo else { return };
        // Render on every UI pass: gating on mpv's update flag misses the frame after a paused seek.
        let _ = self.render.update();
        let _ = self.render.render::<GetProc>(fb.0.get() as i32, w, h, true);
        // GL's source rect is bottom-left based; `src` is top-left normalized.
        let sx0 = ((src.min.x * w as f32).round() as i32).clamp(0, w - 1);
        let sx1 = ((src.max.x * w as f32).round() as i32).clamp(sx0 + 1, w);
        let sy0 = (((1.0 - src.max.y) * h as f32).round() as i32).clamp(0, h - 1);
        let sy1 = (((1.0 - src.min.y) * h as f32).round() as i32).clamp(sy0 + 1, h);
        let cb = egui_glow::CallbackFn::new(move |info, painter| unsafe {
            let gl = painter.gl();
            let prev = gl.get_parameter_i32(glow::READ_FRAMEBUFFER_BINDING) as u32;
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(fb));
            let s = info.pixels_per_point;
            let sh = info.screen_size_px[1] as f32;
            gl.blit_framebuffer(
                sx0,
                sy0,
                sx1,
                sy1,
                (view.min.x * s) as i32,
                (sh - view.max.y * s) as i32,
                (view.max.x * s) as i32,
                (sh - view.min.y * s) as i32,
                glow::COLOR_BUFFER_BIT,
                glow::LINEAR,
            );
            gl.bind_framebuffer(
                glow::READ_FRAMEBUFFER,
                std::num::NonZeroU32::new(prev).map(glow::NativeFramebuffer),
            );
        });
        ui.painter().add(egui::PaintCallback {
            rect: view,
            callback: Arc::new(cb),
        });
    }
}
