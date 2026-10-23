//! The resolved parameters: what the host sends, read into typed values.

use crate::{analysis::*, presets::*};
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilmType {
    ColorNegative,
    BwNegative,
    Positive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wb {
    /// Grey-edge balance (the default).
    Neutral,
    /// Each channel stretched between its own ends: strong, can over-correct.
    Color,
    Warm,
    Cool,
    /// Base removed, no balance.
    None,
}

/// The resolved parameters (the host has clamped them to the manifest's ranges already).
#[derive(Clone, Debug)]
pub struct Params {
    pub film: FilmType,
    pub look: Look,
    pub stock: Stock,
    pub pre_saturation: f32,
    pub border: f32,
    pub profile: Profile,
    pub exposure: f32,
    pub brightness: f32,
    pub contrast: f32,
    pub lights: f32,
    pub darks: f32,
    pub whites: f32,
    pub blacks: f32,
    pub fade: f32,
    pub glow: f32,
    pub soft_highs: bool,
    pub soft_lows: bool,
    pub show_border: bool,
    /// The 0.9 method for colour negatives: each layer stretched to its own range, balanced by
    /// edge contrast (`method` = "Per channel").
    pub per_channel: bool,
    pub auto_exposure: bool,
    pub white_clip: i64,
    pub black_clip: i64,
    pub wb: Wb,
    pub temp: f32,
    pub tint: f32,
    pub analysis: Option<Analysis>,
}

impl Default for Params {
    fn default() -> Self {
        Params::from_json(&Value::Null)
    }
}

impl Params {
    /// Whether to draw the border buffer preview: while "Show border buffer" is ticked and the
    /// photo isn't measured with this border buffer yet. The host can't tell a filter that it is
    /// exporting, so the preview goes away on Measure rather than staying in exports.
    pub fn shows_border(&self) -> bool {
        self.show_border && self.analysis.as_ref().and_then(|a| a.border).is_none_or(|b| (b - self.border).abs() > 1e-3)
    }

    pub fn from_json(v: &Value) -> Params {
        let num = |k: &str, d: f64| v.get(k).and_then(Value::as_f64).filter(|f| f.is_finite()).unwrap_or(d) as f32;
        let int = |k: &str| v.get(k).and_then(Value::as_f64).filter(|f| f.is_finite()).unwrap_or(0.0).round() as i64;
        let flag = |k: &str| v.get(k).and_then(Value::as_bool).unwrap_or(false);
        let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("");
        Params {
            film: match s("filmType") {
                "B&W Negative" => FilmType::BwNegative,
                "Positive (Slide)" => FilmType::Positive,
                _ => FilmType::ColorNegative,
            },
            look: LOOKS.iter().find(|l| l.name == s("look")).copied().unwrap_or(LOOKS[0]),
            // `colorModel` is the setting before film stocks: "None" maps to no correction
            stock: STOCKS.iter().find(|k| k.name == s("stock")).copied().unwrap_or(if s("colorModel") == "None" { STOCKS[1] } else { STOCKS[0] }),
            pre_saturation: num("preSaturation", 3.0),
            border: num("borderBuffer", 10.0).clamp(0.0, 45.0),
            profile: PROFILES.iter().find(|p| p.name == s("toneProfile")).copied().unwrap_or(PROFILES[0]),
            exposure: num("exposure", 0.0),
            brightness: num("brightness", 0.0),
            contrast: num("contrast", 0.0),
            lights: num("lights", 0.0),
            darks: num("darks", 0.0),
            whites: num("whites", 0.0),
            blacks: num("blacks", 0.0),
            fade: num("fade", 0.0),
            glow: num("glow", 0.0),
            soft_highs: flag("softHighs"),
            soft_lows: flag("softLows"),
            show_border: flag("showBorder"),
            per_channel: s("method") == "Per channel",
            auto_exposure: flag("autoExposure"),
            white_clip: int("whiteClip"),
            black_clip: int("blackClip"),
            wb: match s("wbMode") {
                "Auto-Color" => Wb::Color,
                "Auto-Warm" => Wb::Warm,
                "Auto-Cool" => Wb::Cool,
                "None" => Wb::None,
                _ => Wb::Neutral,
            },
            temp: num("temp", 0.0),
            tint: num("tint", 0.0),
            analysis: v.get("_analysis").and_then(Analysis::from_json),
        }
    }
}
