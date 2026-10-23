//! Tone profiles and film stocks (the creative looks are the stocks named "Creative: …"): the
//! menus' entries and their values. The values live in `presets.toml` (edit that); the build turns
//! them into [`PROFILES`] and [`STOCKS`],
//! in menu order, the first of each being the default.

/// A tone profile: a curve through (0,0), (½,½), (1,1) with these slopes at the shadows, the
/// midpoint and the highlights (a monotone cubic Hermite), the power that turns `t` into linear light
/// spans, a floor that lifts black, a midtone gamma and a saturation factor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Profile {
    pub name: &'static str,
    pub slopes: [f32; 3],
    pub power: f32,
    pub lift: f32,
    pub gamma: f32,
    pub sat: f32,
}

/// A film stock's gentle correction: what Auto-Neutral can't fix on its own (it already removes a
/// frame's overall cast, and Measure already maps each frame's own black and white, so a stock's
/// contrast and toe values stay small: a "contrasty" stock is mostly corrected by the measurement). Offsets are layered under the look and the sliders.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stock {
    pub name: &'static str,
    /// Dye cross-talk correction strength: 1 for a masked film; more for a film without an orange
    /// mask, whose dyes' unwanted absorptions nothing else corrects.
    pub crosstalk: f32,
    pub sat: f32,
    pub contrast: f32,
    /// Added to the Darks slider: lifts a toe that falls off fast.
    pub darks: f32,
    pub temp: f32,
    pub tint: f32,
    /// Added to R, G, B in the shadows and the highlights (counters a tone-dependent cast).
    pub shadows: [f32; 3],
    pub highlights: [f32; 3],
    /// Saturation by hue: red, yellow, green, cyan, blue, magenta (1 = unchanged).
    pub hue_sat: [f32; 6],
    /// Lifts black (in `t`, as a profile's lift; the creative looks' matte).
    pub fade: f32,
}

include!(concat!(env!("OUT_DIR"), "/presets.rs"));
