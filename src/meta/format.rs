use super::*;

pub(super) fn push(
    out: &mut Vec<Section>,
    icon: &'static str,
    title: impl Into<String>,
    rows: Vec<(impl Into<String>, String)>,
) {
    if !rows.is_empty() {
        out.push(Section {
            icon,
            title: title.into(),
            rows: rows.into_iter().map(|(k, v)| (k.into(), v)).collect(),
        });
    }
}

/// "H.264 · High" from mpv's codec description and profile.
pub(super) fn codec(s: &impl Fn(&str) -> Option<String>, track: &str) -> Option<String> {
    let name = s(&format!("{track}/codec-desc"))
        .map(|d| d.split(" / ").next().unwrap_or(&d).trim().to_string())
        .or_else(|| s(&format!("{track}/codec")).map(|c| c.to_uppercase()))?;
    Some(match s(&format!("{track}/codec-profile")) {
        Some(p) => format!("{name} · {p}"),
        None => name,
    })
}

pub(super) fn bitrate(bps: f64) -> String {
    if bps >= 1e6 {
        format!("{:.1} {}", bps / 1e6, t!("unit.mbps"))
    } else {
        format!("{:.0} {}", bps / 1e3, t!("unit.kbps"))
    }
}

/// "16:9" for common shapes, else "1.85:1".
pub(super) fn ratio(w: i64, h: i64) -> String {
    fn gcd(a: i64, b: i64) -> i64 {
        if b == 0 { a } else { gcd(b, a % b) }
    }
    let g = gcd(w, h).max(1);
    let (a, b) = (w / g, h / g);
    if a <= 32 && b <= 32 {
        format!("{a}:{b}")
    } else {
        format!("{}:1", trim_float(w as f64 / h as f64, 2))
    }
}

/// `29.970` → "29.97", `48.0` → "48".
pub(super) fn trim_float(v: f64, places: usize) -> String {
    let s = format!("{v:.places$}");
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}
