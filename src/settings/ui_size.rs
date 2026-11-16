//! The Interface size setting.

/// How big the interface is drawn. Auto enlarges it to 125% on screens at 1x (Windows at 100%),
/// where 12 px text is small next to the OS's own 14 px; HiDPI screens stay at 100%.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UiSize {
    #[default]
    Auto,
    P100,
    P125,
    P150,
}

impl UiSize {
    /// egui's zoom factor on a screen with `native` pixels per point.
    pub fn zoom(self, native: f32) -> f32 {
        match self {
            Self::Auto if native < 1.25 => 1.25,
            Self::Auto | Self::P100 => 1.0,
            Self::P125 => 1.25,
            Self::P150 => 1.5,
        }
    }
}
