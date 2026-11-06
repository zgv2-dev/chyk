use super::*;

impl App {
    /// Show a scan for the current file and ignored areas: the one already loaded, a cached one,
    /// or a new one if neither exists. A scan already running is left alone.
    pub(super) fn ensure_scan(&mut self, ctx: &egui::Context) {
        let current = self.strip.scan_crop == self.crop;
        if current && (self.strip.scan.is_some() || self.strip.job.is_some()) {
            return;
        }
        // Made for another crop: it no longer describes what will be exported.
        self.cancel_scan();
        self.strip.scan = None;
        let Some(v) = &self.video else { return };
        if let Some((zones, crop, scan)) = self.scans.get(&v.path)
            && *zones == self.strip.zones
            && *crop == self.crop
        {
            self.strip.scan_crop = self.crop;
            self.strip.scan = Some(scan.clone());
            self.strip.progress = 1.0;
            return;
        }
        self.start_scan(ctx);
    }

    /// Scan now, replacing any result (the Rescan button).
    pub(super) fn start_scan(&mut self, ctx: &egui::Context) {
        let Some(v) = &self.video else { return };
        if self.strip.job.is_some() {
            return;
        }
        if self.job.is_some() {
            self.status = Some((t!("status.wait_export").into(), true));
            return;
        }
        let (src, dur) = (v.path.clone(), v.dur);
        // Only the cropped part ends up in the export, so changes outside it don't count.
        let area = strip::Area {
            aspect: v.aspect,
            size: v.size,
            crop: self.crop,
            zones: self.strip.zones.clone(),
        };
        self.strip.scan_crop = self.crop;
        self.strip.scan = None;
        self.strip.progress = 0.0;
        self.strip.partial.clear();
        self.strip.scanned = 0.0;
        self.strip.front = Glide::default();
        self.status = None;
        self.strip.job = Some(strip::scan(src, dur, area, ctx.clone()));
    }

    pub(super) fn cancel_scan(&mut self) {
        if let Some(job) = self.strip.job.take() {
            job.cancel();
        }
        self.strip.progress = 0.0;
        self.strip.partial.clear();
        self.strip.scanned = 0.0;
        self.strip.front = Glide::default();
    }

    /// Open the Strip panel (turning Strip on), or close it, which keeps Strip on.
    pub(super) fn toggle_strip(&mut self, ctx: &egui::Context) {
        self.end_zone_edit();
        if self.panel == Some(Panel::Strip) {
            self.panel = None;
            return;
        }
        if self.panel == Some(Panel::Crop) {
            // Closes like the Crop button does, so an untouched full-frame box isn't kept.
            self.toggle_crop(ctx);
        }
        self.panel = Some(Panel::Strip);
        self.strip.enabled = true;
        // Rescans only if the crop (or the file) changed since the last scan.
        self.ensure_scan(ctx);
    }

    /// Turn Strip off (the bar's own button). Its scan stays saved for this session.
    pub(super) fn strip_off(&mut self) {
        self.end_zone_edit();
        self.strip.enabled = false;
        if self.panel == Some(Panel::Strip) {
            self.panel = None;
        }
    }

    pub(super) fn end_zone_edit(&mut self) {
        self.strip.edit = false;
        self.strip.selected = None;
        self.strip.drag = None;
        self.drag = None;
    }

    pub(super) fn toggle_zone_edit(&mut self) {
        if !self.strip.enabled {
            return;
        }
        self.strip.edit = !self.strip.edit;
        self.strip.selected = None;
        self.strip.drag = None;
        self.drag = None;
        // Editing zones makes a running scan obsolete.
        if self.strip.edit && self.strip.job.is_some() {
            self.cancel_scan();
            self.strip.scan = None;
            self.status = Some((t!("status.scan_cancelled").into(), false));
        }
    }

    pub(super) fn add_zone(&mut self) {
        if !self.strip.enabled {
            return;
        }
        // Middle of what's on screen, so it's visible when zoomed in or cropped.
        let shown = self.shown_src();
        let zone = Rect::from_center_size(shown.center(), shown.size() * 0.2);
        self.strip.zones.push(zone);
        self.strip.selected = Some(self.strip.zones.len() - 1);
        self.strip.edit = true;
        self.zones_changed();
    }

    pub(super) fn clear_zones(&mut self) {
        if self.strip.zones.is_empty() {
            return;
        }
        self.strip.zones.clear();
        self.strip.selected = None;
        self.zones_changed();
    }

    pub(super) fn remove_selected_zone(&mut self) {
        let Some(i) = self.strip.selected else {
            return;
        };
        if i < self.strip.zones.len() {
            self.strip.zones.remove(i);
            self.strip.selected = None;
            self.zones_changed();
        }
    }

    /// Any zone change invalidates a scan: the detected activity was measured with the old mask.
    pub(super) fn zones_changed(&mut self) {
        let had = self.strip.scan.is_some() || self.strip.job.is_some();
        self.cancel_scan();
        self.strip.scan = None;
        if had {
            self.status = Some((t!("status.zones_changed").into(), false));
        }
    }

    /// Intervals of the current trim that survive stripping. `None` until a scan has run.
    pub(super) fn kept(&self) -> Option<Vec<(f64, f64)>> {
        let scan = self.strip.scan.as_ref()?;
        Some(strip::keep(
            &scan.active,
            self.strip.pad,
            self.trim.0,
            self.trim.1,
        ))
    }
}
