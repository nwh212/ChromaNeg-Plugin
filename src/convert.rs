//! The conversion itself, independent of the ABI.
//!
//! Sources: own design on textbook film sensitometry (density = −log10 transmittance; a dye
//! layer's density is close to linear in log exposure over its useful range), with the colour
//! balance from the grey-edge hypothesis (J. van de Weijer, T. Gevers, A. Gijsenij, "Edge-Based
//! Color Constancy", IEEE TIP 16(9), 2007), applied to densities instead of intensities.
//!
//! Everything works in **density**, `d = −log10(x)` per channel, where `x` is the linear sample
//! the host hands over:
//!
//! 1. The film base (orange mask plus fog) is a per-channel density offset: subtract it.
//! 2. Each dye layer has its own contrast. Over a scene, most density changes across edges are
//!    changes of light, not of colour, so the ratio of the channels' mean edge strengths is the ratio
//!    of their contrasts. Dividing by it gives each channel's *green-equivalent* density.
//! 3. The black and white points come from the brightness (mean) histogram of those densities,
//!    never from one channel alone, so a red sign or a blue sky can't tilt the colour.
//!
//! The result `t` (0 = black, 1 = white, a normalised log exposure) is shaped by the tone profile,
//! the look and the sliders, and goes back to linear light at the end.

use serde_json::{Value, json};

/// Percentiles (in %) stored by the analysis for each end of each histogram. `whiteClip` /
/// `blackClip` pick one: index = clip + 2, so 0 → 0.1 %.
pub const PERCENTILES: [f64; 7] = [0.0, 0.05, 0.1, 0.25, 0.5, 1.0, 2.0];

/// The analysis format; an older measurement is ignored (the photo is measured again).
const ANALYSIS_VERSION: i64 = 3;

/// Density histogram range and resolution.
const D_MIN: f32 = -0.5;
const D_MAX: f32 = 5.0;
const BINS: usize = 5500;
/// Smallest linear sample considered (density 6): keeps `log10` finite on black pixels.
const FLOOR: f32 = 1e-6;
/// Edges stronger than this (in density, any channel) are film holder, sprocket holes or text, not scene.
const MAX_EDGE: f32 = 0.6;

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

/// A tone profile: a curve through (0,0), (½,½), (1,1) with these slopes at the shadows, the
/// midpoint and the highlights (a monotone cubic Hermite), the stops of scene range `t = 0..1`
/// spans, a floor that lifts black, a midtone gamma and a saturation factor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Profile {
    pub name: &'static str,
    pub slopes: [f32; 3],
    pub range: f32,
    pub lift: f32,
    pub gamma: f32,
    pub sat: f32,
}

const fn profile(name: &'static str, slopes: [f32; 3], range: f32, lift: f32, gamma: f32, sat: f32) -> Profile {
    Profile { name, slopes, range, lift, gamma, sat }
}

/// The tone profiles, in menu order. The first is the default.
pub const PROFILES: [Profile; 14] = [
    profile("Standard", [0.6, 1.35, 0.6], 6.5, 0.0, 1.0, 1.0),
    profile("Soft", [0.85, 1.12, 0.85], 6.8, 0.0, 1.0, 0.97),
    profile("Hard", [0.4, 1.6, 0.4], 6.2, 0.0, 1.0, 1.04),
    profile("Highlight Soft", [0.6, 1.4, 0.3], 6.5, 0.0, 1.0, 1.0),
    profile("Highlight Hard", [0.6, 1.25, 1.1], 6.5, 0.0, 1.0, 1.0),
    profile("Shadow Soft", [1.0, 1.25, 0.6], 6.5, 0.0, 1.0, 1.0),
    profile("Shadow Hard", [0.3, 1.45, 0.6], 6.5, 0.0, 1.0, 1.0),
    profile("Cine Log", [1.0, 0.8, 1.0], 9.0, 0.07, 0.85, 0.85),
    profile("Cine Rich", [0.45, 1.5, 0.5], 6.0, 0.0, 1.0, 1.15),
    profile("Cine Flat", [0.9, 0.95, 0.9], 8.0, 0.03, 0.95, 0.92),
    profile("Linear", [1.0, 1.0, 1.0], 6.5, 0.0, 1.0, 1.0),
    profile("Linear Gamma", [1.0, 1.0, 1.0], 6.5, 0.0, 0.85, 1.0),
    profile("Linear Deep", [1.0, 1.0, 1.0], 8.0, 0.0, 1.0, 1.0),
    profile("Linear Flat", [1.0, 1.0, 1.0], 5.0, 0.0, 1.0, 1.0),
];

/// A look: offsets layered on top of the sliders (so the sliders still start at 0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub name: &'static str,
    pub sat: f32,
    pub contrast: f32,
    pub temp: f32,
    pub tint: f32,
    /// Added to R, G, B in the shadows and in the highlights (in `t`, after the tone curve).
    pub shadows: [f32; 3],
    pub highlights: [f32; 3],
    /// Dye cross-talk correction strength (0 = none, 1 = the Basic colour model's).
    pub crosstalk: f32,
    pub fade: f32,
}

#[allow(clippy::too_many_arguments)]
const fn look(
    name: &'static str,
    sat: f32,
    contrast: f32,
    temp: f32,
    tint: f32,
    shadows: [f32; 3],
    highlights: [f32; 3],
    crosstalk: f32,
    fade: f32,
) -> Look {
    Look { name, sat, contrast, temp, tint, shadows, highlights, crosstalk, fade }
}

/// The looks, in menu order. The first is the default. Own designs; the comments say what each is after.
pub const LOOKS: [Look; 14] = [
    // balanced, a touch of density
    look("Standard", 1.0, 0.0, 0.0, 0.0, [0.0; 3], [0.0; 3], 1.0, 0.0),
    // as measured: no cross-talk correction, slightly muted
    look("Neutral", 0.92, -4.0, 0.0, 0.0, [0.0; 3], [0.0; 3], 0.0, 0.0),
    // deeper colour and contrast
    look("Rich", 1.18, 8.0, 2.0, 0.0, [0.0; 3], [0.0; 3], 1.3, 0.0),
    // clean whites, firm blacks
    look("Clean Contrast", 1.02, 12.0, 0.0, 0.0, [0.0; 3], [0.0; 3], 1.0, 0.0),
    look("Clean Contrast +", 1.08, 22.0, 0.0, 0.0, [-0.01, -0.01, -0.01], [0.0; 3], 1.2, 0.0),
    // consumer gold film: warm, yellow highlights, saturated
    look("Golden Hour", 1.12, 4.0, 9.0, 1.0, [0.0, -0.004, -0.012], [0.012, 0.006, -0.02], 1.1, 0.0),
    // professional portrait film: low contrast, warm skin, gentle saturation
    look("Portrait Warm", 0.9, -8.0, 5.0, 2.0, [0.004, 0.0, -0.004], [0.01, 0.004, -0.008], 0.8, 0.01),
    // the airy version: bright, lifted, pastel
    look("Portrait Airy", 0.82, -14.0, 3.0, 2.0, [0.01, 0.01, 0.012], [0.006, 0.004, 0.0], 0.6, 0.05),
    // high-key fashion: cool, cleaner, contrasty
    look("Fashion Cool", 0.88, 10.0, -6.0, -1.0, [-0.004, 0.0, 0.01], [0.0, 0.0, 0.006], 1.0, 0.0),
    // lab scanner, crisp: cyan-leaning blues, firm greens
    look("Crisp Scan", 1.1, 8.0, -2.0, -3.0, [-0.006, 0.002, 0.006], [0.0, 0.0, 0.004], 1.4, 0.0),
    // lab scanner, mellow: green-tinted shadows, warm highlights
    look("Green Shadows", 1.04, 5.0, 2.0, -4.0, [-0.012, 0.012, -0.004], [0.008, 0.004, -0.006], 1.0, 0.0),
    // consumer film with lively greens and cool shadows
    look("Vivid Green", 1.15, 6.0, -3.0, -6.0, [-0.006, 0.006, 0.006], [0.0; 3], 1.2, 0.0),
    // tungsten cine stock in daylight: cool teal shadows, warm highlights
    look("Night Cine", 1.05, 6.0, -8.0, 0.0, [-0.016, 0.004, 0.016], [0.016, 0.004, -0.012], 1.0, 0.0),
    // optical print on archival paper: deep blacks, rich reds, warm
    look("Archive Print", 1.08, 10.0, 4.0, 2.0, [0.006, -0.004, -0.006], [0.006, 0.0, -0.006], 1.2, 0.0),
];

/// A film stock's gentle correction: what Auto-Neutral can't fix on its own (it already removes a
/// frame's overall cast). Offsets are layered under the look and the sliders.
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
}

#[allow(clippy::too_many_arguments)]
const fn stock(
    name: &'static str,
    crosstalk: f32,
    sat: f32,
    contrast: f32,
    darks: f32,
    temp: f32,
    tint: f32,
    shadows: [f32; 3],
    highlights: [f32; 3],
    hue_sat: [f32; 6],
) -> Stock {
    Stock { name, crosstalk, sat, contrast, darks, temp, tint, shadows, highlights, hue_sat }
}

const FLAT: [f32; 6] = [1.0; 6];
const NONE3: [f32; 3] = [0.0; 3];

/// The film stocks, in menu order. The first is the default. Each corrects the stock's reported
/// tendencies gently; the comments say what (sources in the README). Tint < 0 counters magenta.
pub const STOCKS: [Stock; 24] = [
    // the generic masked-film correction (the former "Basic" colour model)
    stock("Generic", 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, NONE3, NONE3, FLAT),
    // no correction at all (the former "None")
    stock("None (as scanned)", 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, NONE3, NONE3, FLAT),
    // Kodak Portra 160: flat, pastel, slightly desaturated by design; slight magenta-blue bias
    stock("Kodak Portra 160", 1.0, 1.05, 4.0, 0.0, 0.0, -2.0, NONE3, [-0.003, 0.0, 0.003], [1.0, 1.03, 1.02, 1.0, 1.0, 1.0]),
    // Kodak Portra 400: natural, low saturation; slight magenta-blue bias
    stock("Kodak Portra 400", 1.0, 1.0, 0.0, 0.0, 0.0, -2.0, NONE3, NONE3, FLAT),
    // Kodak Portra 800: warmer, more saturated and contrasty than its siblings
    stock("Kodak Portra 800", 1.0, 0.97, -4.0, 0.0, -2.0, 0.0, NONE3, [-0.004, 0.0, 0.004], FLAT),
    // Kodak Ektar 100: the most saturated C-41 stock, warm orange lean, reddish skin
    stock("Kodak Ektar 100", 1.1, 0.93, -3.0, 0.0, -3.0, 0.0, NONE3, NONE3, [0.88, 0.94, 1.0, 1.0, 1.0, 0.96]),
    // Kodak Gold 200: warm, reds and yellows pushed, contrasty; weak on overcast days
    stock("Kodak Gold 200", 1.0, 1.0, -4.0, 0.0, -4.0, 0.0, NONE3, NONE3, [0.92, 0.9, 1.02, 1.0, 1.0, 1.0]),
    // Kodak UltraMax 400: saturated and a bit punchier than Portra, slightly cooler than Gold
    stock("Kodak UltraMax 400", 1.0, 0.95, -3.0, 0.0, -1.0, 0.0, NONE3, NONE3, FLAT),
    // Kodak ColorPlus 200: soft colour, moderate contrast, flat in dull light
    stock("Kodak ColorPlus 200", 1.0, 1.06, 4.0, 0.0, 0.0, 0.0, NONE3, NONE3, FLAT),
    // Kodak Vision3 50D (ECN-2 cine stock): flat by design, meant to be graded
    stock("Kodak Vision3 50D", 1.0, 1.04, 8.0, 0.0, 0.0, 0.0, NONE3, NONE3, FLAT),
    // Kodak Vision3 250D: flat by design
    stock("Kodak Vision3 250D", 1.0, 1.06, 10.0, 0.0, 0.0, 0.0, NONE3, NONE3, FLAT),
    // Kodak Vision3 500T: flat, tungsten-balanced (Auto-Neutral removes the daylight blue)
    stock("Kodak Vision3 500T", 1.0, 1.05, 10.0, 0.0, 0.0, 0.0, NONE3, NONE3, FLAT),
    // CineStill 50D (Vision3 50D without remjet): fine grain, accurate; red halation
    stock("CineStill 50D", 1.0, 1.03, 6.0, 0.0, 0.0, 0.0, NONE3, NONE3, FLAT),
    // CineStill 400D: medium contrast, occasional mint-green cast
    stock("CineStill 400D", 1.0, 1.04, 6.0, 0.0, 0.0, 4.0, [0.0, -0.004, 0.0], NONE3, FLAT),
    // CineStill 800T (Vision3 500T without remjet): flat-ish, tungsten-balanced; red halation
    stock("CineStill 800T", 1.0, 1.04, 8.0, 0.0, 0.0, 0.0, NONE3, NONE3, FLAT),
    // Fujifilm Superia X-TRA 400: magenta casts reported in scans
    stock("Fujifilm Superia X-TRA 400", 1.0, 1.0, 0.0, 0.0, 0.0, -5.0, NONE3, NONE3, [1.0, 1.0, 1.04, 1.0, 1.0, 0.9]),
    // Fujicolor C200 (the classic Fuji emulsion): cool, green-leaning
    stock("Fujicolor C200", 1.0, 1.02, 0.0, 0.0, 2.0, 2.0, NONE3, NONE3, [1.0, 1.0, 0.97, 1.0, 1.0, 1.0]),
    // Fujifilm 200 (2022 on): its datasheet matches Kodak Gold 200's, so corrected like Gold
    stock("Fujifilm 200", 1.0, 1.0, -3.0, 0.0, -3.0, 0.0, NONE3, NONE3, [0.93, 0.92, 1.02, 1.0, 1.0, 1.0]),
    // Fujifilm 400 (US, 2023 on): widely thought to be UltraMax-like; corrected like UltraMax
    stock("Fujifilm 400", 1.0, 0.96, -3.0, 0.0, -1.0, 0.0, NONE3, NONE3, FLAT),
    // Fujifilm Pro 400H (discontinued, fourth colour layer): pastel, greenish-blue cast
    stock("Fujifilm Pro 400H", 1.0, 1.03, 0.0, 0.0, 2.0, 3.0, NONE3, NONE3, [1.0, 1.0, 0.96, 0.97, 1.0, 1.0]),
    // Harman Phoenix 200 (I): no orange mask, high contrast, shadows fall off fast; bold reds and
    // blues, weak yellows and skin; warm highlights over cold, greenish shadows
    stock("Harman Phoenix 200", 1.8, 0.92, -14.0, 18.0, -6.0, 0.0, [0.006, -0.008, 0.002], [-0.006, 0.0, 0.006], [0.88, 1.15, 1.0, 0.95, 0.9, 1.0]),
    // Harman Phoenix II: no orange mask; more normal contrast and colour than Phoenix I, cooler
    // blues and greens, shadows still fall off fast
    stock("Harman Phoenix II", 1.6, 0.96, -8.0, 14.0, 4.0, 2.0, [0.004, -0.005, 0.0], NONE3, [0.95, 1.06, 0.97, 0.94, 0.95, 1.0]),
    // Lomography Color Negative 400 (thought to be Kodak-made): balanced, slightly muted
    stock("Lomography 400", 1.0, 1.06, 0.0, 0.0, 0.0, 0.0, NONE3, NONE3, FLAT),
    // Lomography Color Negative 800: medium-high saturation, natural skin
    stock("Lomography 800", 1.0, 0.98, -2.0, 0.0, 0.0, 0.0, NONE3, NONE3, FLAT),
];

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
    pub soft_highs: bool,
    pub soft_lows: bool,
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
            soft_highs: flag("softHighs"),
            soft_lows: flag("softLows"),
            auto_exposure: v.get("autoExposure").and_then(Value::as_bool).unwrap_or(true),
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

/// The whole-image measurement.
#[derive(Clone, Debug, PartialEq)]
pub struct Analysis {
    /// For R, G, B and the mean density (index 3, used for B&W): the density at each of
    /// [`PERCENTILES`] from the thin end (`lo`) and the dense end (`hi`).
    pub lo: [[f32; 7]; 4],
    pub hi: [[f32; 7]; 4],
    /// Each channel's contrast relative to green, from the edges (green = 1).
    pub gain: [f32; 3],
    /// The brightness (mean green-equivalent density above the base) at each of [`PERCENTILES`],
    /// from the thin end and the dense end.
    pub lum_lo: [f32; 7],
    pub lum_hi: [f32; 7],
    /// Per channel, the balanced density above the base at each of [`PERCENTILES`] from the thin
    /// end: where each channel's black lies once the layers are balanced.
    pub ge_lo: [[f32; 7]; 3],
    /// The brightness at the 25th, 50th and 75th percentiles.
    pub lum_mid: [f32; 3],
    /// The sample grid measured (width, height, step in pixels): a diagnostic, not used by the conversion.
    pub measured: [u32; 3],
}

impl Analysis {
    pub fn to_json(&self) -> Value {
        json!({"version": ANALYSIS_VERSION, "percentiles": PERCENTILES, "lo": self.lo, "hi": self.hi,
               "gain": self.gain, "lumLo": self.lum_lo, "lumHi": self.lum_hi, "geLo": self.ge_lo, "lumMid": self.lum_mid,
               "measured": self.measured})
    }

    pub fn from_json(v: &Value) -> Option<Analysis> {
        if v.get("version")?.as_i64()? != ANALYSIS_VERSION {
            return None;
        }
        fn floats<const N: usize>(v: &Value) -> Option<[f32; N]> {
            let a = v.as_array()?;
            let mut out = [0.0; N];
            for (o, x) in out.iter_mut().zip(a) {
                *o = x.as_f64().filter(|f| f.is_finite())? as f32;
            }
            (a.len() == N).then_some(out)
        }
        let table = |k: &str| -> Option<[[f32; 7]; 4]> {
            let a = v.get(k)?.as_array()?;
            Some([floats(a.first()?)?, floats(a.get(1)?)?, floats(a.get(2)?)?, floats(a.get(3)?)?])
        };
        let gain: [f32; 3] = floats(v.get("gain")?)?;
        if gain.iter().any(|g| !(0.05..=20.0).contains(g)) {
            return None;
        }
        Some(Analysis {
            lo: table("lo")?,
            hi: table("hi")?,
            gain,
            lum_lo: floats(v.get("lumLo")?)?,
            lum_hi: floats(v.get("lumHi")?)?,
            ge_lo: {
                let a = v.get("geLo")?.as_array()?;
                [floats(a.first()?)?, floats(a.get(1)?)?, floats(a.get(2)?)?]
            },
            lum_mid: floats(v.get("lumMid")?)?,
            measured: v.get("measured").and_then(|m| serde_json::from_value(m.clone()).ok()).unwrap_or_default(),
        })
    }

    /// What we assume before the photo is measured: no mask, two density units of range, no
    /// balance. The result keeps the film's cast, which is the cue to press Measure.
    fn unmeasured() -> Analysis {
        Analysis {
            lo: [[0.1; 7]; 4],
            hi: [[2.1; 7]; 4],
            gain: [1.0; 3],
            lum_lo: [0.0; 7],
            lum_hi: [2.0; 7],
            ge_lo: [[0.0; 7]; 3],
            lum_mid: [0.5, 1.0, 1.5],
            measured: [0; 3],
        }
    }
}

#[cfg(test)]
fn density(x: f32) -> f32 {
    -x.max(FLOOR).log10()
}

fn bin(d: f32) -> usize {
    (((d - D_MIN) / (D_MAX - D_MIN)) * BINS as f32).clamp(0.0, (BINS - 1) as f32) as usize
}

fn bin_density(i: usize) -> f32 {
    D_MIN + (i as f32 + 0.5) * (D_MAX - D_MIN) / BINS as f32
}

/// A histogram that answers "the value below which `frac` of the samples lie".
struct Hist {
    bins: Vec<u32>,
    count: u64,
}

impl Hist {
    fn new() -> Hist {
        Hist { bins: vec![0; BINS], count: 0 }
    }

    fn add(&mut self, v: f32) {
        if let Some(b) = self.bins.get_mut(bin(v)) {
            *b = b.saturating_add(1);
            self.count += 1;
        }
    }

    fn at(&self, frac: f64) -> f32 {
        let target = (frac * self.count as f64).floor() as u64;
        let mut seen = 0u64;
        for (i, b) in self.bins.iter().enumerate() {
            seen += u64::from(*b);
            if seen > target {
                return bin_density(i);
            }
        }
        bin_density(BINS - 1)
    }

    /// The value at each of [`PERCENTILES`] from the low end and from the high end.
    fn ends(&self) -> ([f32; 7], [f32; 7]) {
        let (mut lo, mut hi) = ([0.0; 7], [0.0; 7]);
        for (i, pct) in PERCENTILES.iter().enumerate() {
            lo[i] = self.at(pct / 100.0);
            hi[i] = self.at(1.0 - pct / 100.0 - 1e-9);
        }
        (lo, hi)
    }
}

/// Auto exposure: where the median brightness should land in `t`, and the most it is lifted.
const AUTO_MID: f32 = 0.42;
const AUTO_MAX_LIFT: f32 = 0.3;

/// Most pixels the measurement reads: plenty for histograms, and it keeps Measure well inside the
/// host's instruction budget whatever size it is handed.
const MAX_SAMPLES: usize = 300_000;

/// Measures `px` (interleaved linear RGB, `w × h`, the whole uncropped photo) inside the border buffer.
pub fn analyze(px: &[f32], w: usize, h: usize, p: &Params) -> Analysis {
    let inset = |n: usize| ((n as f32 * p.border / 100.0) as usize).min(n.saturating_sub(1) / 2);
    let (ix, iy) = (inset(w), inset(h));
    let (x0, x1, y0, y1) = (ix, w.saturating_sub(ix), iy, h.saturating_sub(iy));
    // every `step`-th pixel each way, densities computed once
    let area = (x1 - x0).saturating_mul(y1 - y0);
    let step = ((area as f64 / MAX_SAMPLES as f64).sqrt().ceil() as usize).max(1);
    let (gw, gh) = ((x1 - x0).div_ceil(step), (y1 - y0).div_ceil(step));
    let log = FastLog::new();
    let mut grid: Vec<[f32; 3]> = Vec::with_capacity(gw * gh);
    for gy in 0..gh {
        for gx in 0..gw {
            let i = ((y0 + gy * step) * w + x0 + gx * step) * 3;
            let d = px.get(i..i + 3).map(|c| [log.density(c[0]), log.density(c[1]), log.density(c[2])]);
            grid.push(d.unwrap_or([f32::NAN; 3]));
        }
    }
    // pass 1: per-channel and mean-density histograms, and edge energy per channel
    let mut hists: Vec<Hist> = (0..4).map(|_| Hist::new()).collect();
    let mut edge = [0f64; 3];
    let cell = |gx: usize, gy: usize| grid.get(gy * gw + gx).copied().filter(|d| d[0].is_finite());
    for gy in 0..gh {
        for gx in 0..gw {
            let Some(d) = cell(gx, gy) else { continue };
            for (k, v) in d.into_iter().chain([(d[0] + d[1] + d[2]) / 3.0]).enumerate() {
                if let Some(hk) = hists.get_mut(k) {
                    hk.add(v);
                }
            }
            // differences to the right and below
            for n in [(gx + 1 < gw).then(|| cell(gx + 1, gy)).flatten(), (gy + 1 < gh).then(|| cell(gx, gy + 1)).flatten()].into_iter().flatten() {
                let g = [(n[0] - d[0]).abs(), (n[1] - d[1]).abs(), (n[2] - d[2]).abs()];
                if g.iter().all(|v| *v < MAX_EDGE) {
                    for c in 0..3 {
                        edge[c] += f64::from(g[c] * g[c]);
                    }
                }
            }
        }
    }
    if hists[3].count == 0 {
        return Analysis::unmeasured();
    }
    let mut a = Analysis::unmeasured();
    for (k, hk) in hists.iter().enumerate() {
        (a.lo[k], a.hi[k]) = hk.ends();
    }
    if edge[1] > 0.0 {
        for c in 0..3 {
            a.gain[c] = ((edge[c] / edge[1]).sqrt() as f32).clamp(0.2, 5.0);
        }
    }
    // pass 2: brightness histogram of the balanced densities
    let base = [a.lo[0][2], a.lo[1][2], a.lo[2][2]];
    let mut lum = Hist::new();
    let mut ge: Vec<Hist> = (0..3).map(|_| Hist::new()).collect();
    for d in grid.iter().filter(|d| d[0].is_finite()) {
        let g = [(d[0] - base[0]) / a.gain[0], (d[1] - base[1]) / a.gain[1], (d[2] - base[2]) / a.gain[2]];
        lum.add((g[0] + g[1] + g[2]) / 3.0);
        for (h, v) in ge.iter_mut().zip(g) {
            h.add(v);
        }
    }
    (a.lum_lo, a.lum_hi) = lum.ends();
    for (c, h) in ge.iter().enumerate() {
        a.ge_lo[c] = h.ends().0;
    }
    a.lum_mid = [lum.at(0.25), lum.at(0.5), lum.at(0.75)];
    a.measured = [gw as u32, gh as u32, step as u32];
    a
}

fn clip_index(clip: i64) -> usize {
    (clip + 2).clamp(0, PERCENTILES.len() as i64 - 1) as usize
}

fn bell(t: f32, centre: f32, width: f32) -> f32 {
    let x = (t - centre) / width;
    (-x * x).exp()
}

/// The profile's curve: a cubic Hermite through (0,0), (½,½), (1,1) with the profile's slopes.
fn profile_curve(t: f32, s: &[f32; 3]) -> f32 {
    let (y0, m0, m1) = if t < 0.5 { (0.0, s[0], s[1]) } else { (0.5, s[1], s[2]) };
    let u = ((t - y0) * 2.0).clamp(0.0, 1.0);
    let (u2, u3) = (u * u, u * u * u);
    // h00·y0 + h10·Δ·m0 + h01·y1 + h11·Δ·m1 with y1 = y0 + Δ, Δ = ½, and h00 + h01 = 1
    let h10 = u3 - 2.0 * u2 + u;
    let h01 = -2.0 * u3 + 3.0 * u2;
    let h11 = u3 - u2;
    y0 + 0.5 * (h01 + m0 * h10 + m1 * h11)
}

/// `log2(x)` for positive finite `x`, from the float's exponent and a table of the mantissa (error < 1e-5).
struct FastLog {
    table: [f32; 257],
}

impl FastLog {
    fn new() -> FastLog {
        let mut table = [0.0; 257];
        for (i, v) in table.iter_mut().enumerate() {
            *v = (1.0 + i as f32 / 256.0).log2();
        }
        FastLog { table }
    }

    fn log2(&self, x: f32) -> f32 {
        let bits = x.to_bits();
        let e = ((bits >> 23) & 0xff) as i32 - 127;
        let m = (bits & 0x7f_ffff) as f32 / (1 << 23) as f32 * 256.0;
        let i = (m as usize).min(255);
        let f = m - i as f32;
        let (a, b) = (self.table.get(i).copied().unwrap_or(0.0), self.table.get(i + 1).copied().unwrap_or(1.0));
        e as f32 + a + (b - a) * f
    }

    /// `−log10(x)`, `x` floored at [`FLOOR`] (NaN counts as the floor).
    fn density(&self, x: f32) -> f32 {
        let x = if x >= FLOOR && x.is_finite() {
            x
        } else if x > FLOOR {
            1e30
        } else {
            FLOOR
        };
        -self.log2(x) * std::f32::consts::LOG10_2
    }
}

/// A 1-D function sampled on `lo..=hi` and read back with linear interpolation.
struct Lut {
    lo: f32,
    scale: f32,
    v: Vec<f32>,
}

impl Lut {
    fn new(lo: f32, hi: f32, n: usize, f: impl Fn(f32) -> f32) -> Lut {
        let v = (0..=n).map(|i| f(lo + (hi - lo) * i as f32 / n as f32)).collect();
        Lut { lo, scale: n as f32 / (hi - lo), v }
    }

    fn get(&self, x: f32) -> f32 {
        let last = self.v.len().saturating_sub(1);
        let p = ((x - self.lo) * self.scale).clamp(0.0, last as f32);
        let i = (p as usize).min(last.saturating_sub(1));
        let f = p - i as f32;
        let (a, b) = (self.v.get(i).copied().unwrap_or(0.0), self.v.get(i + 1).copied().unwrap_or(0.0));
        a + (b - a) * f
    }
}

/// How each channel's density becomes `t`: `t = (d − origin) · scale`, then per channel `gamma`.
struct Plan {
    origin: [f32; 3],
    scale: [f32; 3],
    flip: bool,
    bw: bool,
    offset: [f32; 3],
    sat: f32,
    crosstalk: f32,
    hue_sat: Option<[f32; 6]>,
    shadows: [f32; 3],
    highlights: [f32; 3],
    tinted: bool,
    /// `t` (before tone) → `t` (after tone and sliders).
    tone: Lut,
    /// `t` (after tone, tint and fade) → linear light.
    linear: Lut,
}

fn plan(p: &Params) -> Plan {
    let a = p.analysis.clone().unwrap_or_else(Analysis::unmeasured);
    let (wi, bi) = (clip_index(p.white_clip), clip_index(p.black_clip));
    let bw = p.film == FilmType::BwNegative;
    let flip = p.film == FilmType::Positive;
    // on a slide the thin end is white, so the clips swap ends
    let (thin_i, dense_i) = if flip { (wi, bi) } else { (bi, wi) };
    let mut origin = [0.0; 3];
    let mut scale = [1.0; 3];
    // where the frame's median brightness lands in `t` (before the flip), for auto exposure
    let mut mid = 0.5;
    if bw {
        let (thin, dense) = (a.lo[3][thin_i], a.hi[3][dense_i].max(a.lo[3][thin_i] + 0.05));
        origin = [thin; 3];
        scale = [1.0 / (dense - thin); 3];
    } else if p.wb == Wb::Color {
        for c in 0..3 {
            let (thin, dense) = (a.lo[c][thin_i], a.hi[c][dense_i].max(a.lo[c][thin_i] + 0.05));
            origin[c] = thin;
            scale[c] = 1.0 / (dense - thin);
        }
    } else {
        // base removed and layers balanced (unless None); the thin end per channel, so the
        // shadows (of a negative) come out neutral, the dense end shared from the brightness
        let neutral = p.wb != Wb::None;
        let gain = if neutral { a.gain } else { [1.0; 3] };
        let thin = if neutral { [a.ge_lo[0][thin_i], a.ge_lo[1][thin_i], a.ge_lo[2][thin_i]] } else { [a.lum_lo[thin_i]; 3] };
        let thin_mean = (thin[0] + thin[1] + thin[2]) / 3.0;
        let dense = a.lum_hi[dense_i].max(thin_mean + 0.05);
        for c in 0..3 {
            // t = ((d − base)/gain − thin_c)/(dense − thin_mean)
            origin[c] = a.lo[c][2] + thin[c] * gain[c];
            scale[c] = 1.0 / (gain[c] * (dense - thin_mean));
        }
        mid = (a.lum_mid[1] - thin_mean) / (dense - thin_mean);
    }
    let look = p.look;
    let st = p.stock;
    let temp = p.temp
        + look.temp
        + st.temp
        + match p.wb {
            Wb::Warm => 10.0,
            Wb::Cool => -10.0,
            _ => 0.0,
        };
    let tint = p.tint + look.tint + st.tint;
    let offset = if bw { [0.0; 3] } else { [temp / 400.0, -tint / 400.0, -temp / 400.0] };
    let sat = if bw { 0.0 } else { (1.0 + (p.pre_saturation - 3.0) * 0.15) * look.sat * st.sat * p.profile.sat };
    let crosstalk = if bw { 0.0 } else { look.crosstalk * st.crosstalk };
    let pr = p.profile;
    let contrast = p.contrast + look.contrast + st.contrast;
    let mut params = p.clone();
    params.darks += st.darks;
    if p.auto_exposure && p.analysis.is_some() {
        let mid = if flip { 1.0 - mid } else { mid };
        // lift a dark frame's median toward mid grey (at most ~2 stops, never darker), and roll the highlights off
        let lift = (AUTO_MID - mid).clamp(0.0, AUTO_MAX_LIFT);
        params.exposure += lift * p.profile.range;
        params.soft_highs |= lift > 0.0;
    }
    let tone = Lut::new(-0.6, 1.6, 2200, move |t| tone(t, &params, contrast, &pr));
    let fade = pr.lift + (p.fade + look.fade * 100.0) / 200.0;
    let range = pr.range;
    let linear = Lut::new(0.0, 1.0, 1024, move |t| {
        let t = fade + t.clamp(0.0, 1.0) * (1.0 - fade);
        (2f32.powf(range * t) - 1.0) / (2f32.powf(range) - 1.0)
    });
    let shadows = [look.shadows[0] + st.shadows[0], look.shadows[1] + st.shadows[1], look.shadows[2] + st.shadows[2]];
    let highlights = [look.highlights[0] + st.highlights[0], look.highlights[1] + st.highlights[1], look.highlights[2] + st.highlights[2]];
    let tinted = !bw && shadows.iter().chain(highlights.iter()).any(|v| *v != 0.0);
    let hue_sat = (!bw && st.hue_sat != FLAT).then_some(st.hue_sat);
    Plan { origin, scale, flip, bw, offset, sat, crosstalk, hue_sat, shadows, highlights, tinted, tone, linear }
}

/// The saturation factor for the hue of `t` (an RGB triple around its mean), from six sectors
/// (red, yellow, green, cyan, blue, magenta) interpolated linearly.
fn hue_factor(t: &[f32; 3], hs: &[f32; 6]) -> f32 {
    let x = t[0] - 0.5 * (t[1] + t[2]);
    let y = 0.866_025_4 * (t[1] - t[2]);
    if x * x + y * y < 1e-12 {
        return 1.0;
    }
    let deg = y.atan2(x).to_degrees().rem_euclid(360.0);
    let f = deg / 60.0;
    let i = (f as usize).min(5);
    let (a, b) = (hs.get(i).copied().unwrap_or(1.0), hs.get((i + 1) % 6).copied().unwrap_or(1.0));
    a + (b - a) * (f - i as f32)
}

/// The profile and the tone sliders, on one channel's normalised log exposure.
fn tone(mut t: f32, p: &Params, contrast: f32, pr: &Profile) -> f32 {
    let (bp, wp) = (-p.blacks / 250.0, 1.0 - p.whites / 250.0);
    t = (t - bp) / (wp - bp).max(0.05);
    t += p.exposure / pr.range;
    t = 0.5 + (t - 0.5) * (1.0 + contrast / 100.0);
    t += 0.25 * (p.lights / 100.0 * bell(t, 0.75, 0.2) + p.darks / 100.0 * bell(t, 0.25, 0.2));
    let g = pr.gamma * 2f32.powf(-p.brightness / 50.0);
    if t > 0.0 && t < 1.0 && g != 1.0 {
        t = t.powf(g);
    }
    if (0.0..=1.0).contains(&t) {
        t = profile_curve(t, &pr.slopes);
    }
    t = if p.soft_highs && t > 0.85 { 0.85 + 0.15 * ((t - 0.85) / 0.15).tanh() } else { t };
    t = if p.soft_lows && t < 0.15 { 0.15 - 0.15 * ((0.15 - t) / 0.15).tanh() } else { t };
    t.clamp(0.0, 1.0)
}

/// Converts interleaved linear RGB in place.
pub fn convert(px: &mut [f32], p: &Params) {
    let pl = plan(p);
    let log = FastLog::new();
    let (sh, hl, tinted) = (pl.shadows, pl.highlights, pl.tinted);
    for c in px.chunks_exact_mut(3) {
        let mut d = [log.density(c[0]), log.density(c[1]), log.density(c[2])];
        if pl.bw {
            d = [(d[0] + d[1] + d[2]) / 3.0; 3];
        }
        let mut t = [0.0f32; 3];
        for k in 0..3 {
            let v = (d[k] - pl.origin[k]) * pl.scale[k];
            t[k] = if pl.flip { 1.0 - v } else { v };
        }
        if pl.crosstalk != 0.0 {
            // dye cross-talk correction; rows sum to 1 so neutrals stay neutral
            let s = pl.crosstalk;
            t = [
                t[0] + s * (0.08 * t[0] - 0.06 * t[1] - 0.02 * t[2]),
                t[1] + s * (-0.04 * t[0] + 0.08 * t[1] - 0.04 * t[2]),
                t[2] + s * (-0.02 * t[0] - 0.06 * t[1] + 0.08 * t[2]),
            ];
        }
        let m = (t[0] + t[1] + t[2]) / 3.0;
        let sat = match &pl.hue_sat {
            Some(hs) => pl.sat * hue_factor(&t, hs),
            None => pl.sat,
        };
        let mut o = [0.0f32; 3];
        for k in 0..3 {
            o[k] = pl.tone.get(m + (t[k] - m) * sat + pl.offset[k]);
        }
        if tinted {
            let l = (o[0] + o[1] + o[2]) / 3.0;
            let (ws, wh) = (bell(l, 0.12, 0.22), bell(l, 0.88, 0.22));
            for k in 0..3 {
                o[k] += sh[k] * ws * 4.0 + hl[k] * wh * 4.0;
            }
        }
        for k in 0..3 {
            c[k] = pl.linear.get(o[k]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A synthetic colour negative: an orange mask (per-channel base density) plus a scene whose
    /// log exposure is spread over each dye layer with its own contrast; a clear film border around it.
    fn negative(w: usize, h: usize) -> Vec<f32> {
        let base = [0.25f32, 0.55, 0.85];
        let gamma = [0.6f32, 0.65, 0.7];
        let mut px = Vec::with_capacity(w * h * 3);
        for y in 0..h {
            for x in 0..w {
                let border = x < w / 20 || y < h / 20 || x >= w - w / 20 || y >= h - h / 20;
                // grey: a ramp of log exposure 0..3 with a stripe pattern for edges
                let e = if border { 0.0 } else { 3.0 * x as f32 / w as f32 + if (y / 4) % 2 == 0 { 0.1 } else { 0.0 } };
                for c in 0..3 {
                    px.push(10f32.powf(-(base[c] + gamma[c] * e)));
                }
            }
        }
        px
    }

    fn measured(px: &[f32], w: usize, h: usize, json: Value) -> Params {
        let mut p = Params::from_json(&json);
        p.analysis = Some(analyze(px, w, h, &p));
        p
    }

    fn spread(c: &[f32]) -> f32 {
        c.iter().cloned().fold(f32::MIN, f32::max) - c.iter().cloned().fold(f32::MAX, f32::min)
    }

    #[test]
    fn a_grey_negative_comes_out_neutral_and_increasing_in_every_wb_mode() {
        let (w, h) = (200, 40);
        for wb in ["Auto-Neutral", "Auto-Color"] {
            let mut px = negative(w, h);
            let p = measured(&px, w, h, json!({"stock": "None (as scanned)", "look": "Neutral", "wbMode": wb}));
            convert(&mut px, &p);
            let row = &px[(h / 2) * w * 3..(h / 2 + 1) * w * 3];
            let mut last = -1.0;
            for c in row.chunks_exact(3).skip(w / 20 + 1).take(w - w / 10 - 2) {
                assert!(spread(c) < 0.02, "{wb}: not neutral: {c:?}");
                assert!(c[1] >= last - 1e-3, "{wb}: not monotonic");
                last = c[1];
            }
            assert!(last > 0.8, "{wb}: brightest end should be near white, got {last}");
        }
    }

    #[test]
    fn a_small_saturated_object_doesnt_tint_the_neutrals() {
        // the grey negative plus a strongly red-dense patch (a sign) in one corner
        let (w, h) = (200, 80);
        let mut px = negative(w, h);
        for y in 10..20 {
            for x in 150..160 {
                px[(y * w + x) * 3] = 10f32.powf(-3.0);
            }
        }
        let p = measured(&px, w, h, json!({"stock": "None (as scanned)", "look": "Neutral"}));
        convert(&mut px, &p);
        let c = &px[(60 * w + 100) * 3..(60 * w + 100) * 3 + 3];
        assert!(spread(c) < 0.03, "mid grey tinted: {c:?}");
    }

    #[test]
    fn the_tone_tables_match_the_functions() {
        let log = FastLog::new();
        for x in [1e-6f32, 0.001, 0.0371, 0.5, 0.999, 1.0, 3.7] {
            assert!((log.density(x) - density(x)).abs() < 1e-4, "{x}");
        }
        for pr in PROFILES {
            let mut last = -1.0;
            for i in 0..=100 {
                let y = profile_curve(i as f32 / 100.0, &pr.slopes);
                assert!(y >= last - 1e-6, "{}: not monotonic", pr.name);
                last = y;
            }
            assert!(profile_curve(0.0, &pr.slopes).abs() < 1e-6 && (profile_curve(1.0, &pr.slopes) - 1.0).abs() < 1e-6, "{}", pr.name);
            assert!((profile_curve(0.5, &pr.slopes) - 0.5).abs() < 1e-6, "{}", pr.name);
        }
    }

    #[test]
    fn analysis_round_trips_through_json() {
        let (w, h) = (64, 32);
        let px = negative(w, h);
        let a = analyze(&px, w, h, &Params::default());
        assert_eq!(Analysis::from_json(&a.to_json()), Some(a));
    }

    #[test]
    fn every_look_and_profile_renders_finite_values() {
        let (w, h) = (64, 32);
        let px0 = negative(w, h);
        for l in LOOKS {
            for pr in PROFILES {
                let mut px = px0.clone();
                let p = measured(&px, w, h, json!({"look": l.name, "toneProfile": pr.name}));
                convert(&mut px, &p);
                assert!(px.iter().all(|v| v.is_finite() && (-0.01..=1.01).contains(v)), "{} / {}", l.name, pr.name);
            }
        }
    }

    #[test]
    fn every_stock_renders_finite_values_and_none_matches_the_old_none() {
        let (w, h) = (64, 32);
        let px0 = negative(w, h);
        for st in STOCKS {
            let mut px = px0.clone();
            let p = measured(&px, w, h, json!({"stock": st.name}));
            convert(&mut px, &p);
            assert!(px.iter().all(|v| v.is_finite() && (-0.01..=1.01).contains(v)), "{}", st.name);
        }
        assert_eq!(Params::from_json(&json!({"colorModel": "None"})).stock.name, "None (as scanned)");
        assert_eq!(Params::from_json(&json!({"colorModel": "Basic"})).stock.name, "Generic");
    }

    #[test]
    fn hue_sectors_land_where_they_should() {
        let hs = [0.5, 1.0, 1.0, 1.0, 2.0, 1.0];
        assert!((hue_factor(&[0.2, -0.1, -0.1], &hs) - 0.5).abs() < 1e-3, "red");
        assert!((hue_factor(&[-0.1, -0.1, 0.2], &hs) - 2.0).abs() < 1e-3, "blue");
        assert_eq!(hue_factor(&[0.0; 3], &hs), 1.0, "grey");
    }

    #[test]
    fn a_harman_stock_softens_contrast_and_lifts_the_shadows() {
        let (w, h) = (200, 40);
        let render = |stock: &str| {
            let mut px = negative(w, h);
            let p = measured(&px, w, h, json!({"stock": stock, "autoExposure": false}));
            convert(&mut px, &p);
            let at = |x: usize| px[((h / 2) * w + x) * 3 + 1];
            (at(w / 5), at(w / 2))
        };
        let (gen_dark, gen_mid) = render("Generic");
        let (ph_dark, ph_mid) = render("Harman Phoenix 200");
        assert!(ph_dark > gen_dark, "shadows should lift: {ph_dark} vs {gen_dark}");
        assert!(ph_mid / ph_dark < gen_mid / gen_dark, "contrast should soften");
    }

    #[test]
    fn hostile_input_never_panics_or_goes_non_finite() {
        let mut px = vec![f32::NAN, f32::INFINITY, -1.0, 0.0, 1e30, -0.0];
        let a = analyze(&px, 2, 1, &Params::default());
        let mut p = Params::from_json(&json!({"_analysis": a.to_json(), "exposure": 5, "contrast": 50, "softHighs": true}));
        convert(&mut px, &p);
        assert!(px.iter().all(|v| v.is_finite()));
        p.analysis = Analysis::from_json(&json!({"version": 2, "lo": [], "hi": [[1]], "gain": [0, 1, 1e9]}));
        assert!(p.analysis.is_none());
        assert_eq!(analyze(&[], 0, 0, &p), Analysis::unmeasured());
        let _ = analyze(&[0.5; 3], 5, 5, &p); // short buffer
    }

    #[test]
    fn auto_exposure_lifts_a_dark_frame_and_leaves_a_normal_one() {
        // a frame that is mostly deep shadow (low density on the negative) with a few bright pixels
        let (w, h) = (100, 100);
        let base = [0.25f32, 0.55, 0.85];
        let dark: Vec<f32> = (0..w * h)
            .flat_map(|i| {
                let e = if i % 50 == 0 { 3.0 } else { 0.3 + 0.2 * ((i % 7) as f32 / 7.0) };
                (0..3).map(move |c| 10f32.powf(-(base[c] + 0.65 * e)))
            })
            .collect();
        let median = |auto: bool, px: &[f32], border: f32| {
            let mut px = px.to_vec();
            let p = measured(&px, w, h, json!({"autoExposure": auto, "borderBuffer": border}));
            convert(&mut px, &p);
            let mut g: Vec<f32> = px.chunks_exact(3).map(|c| c[1]).collect();
            g.sort_by(f32::total_cmp);
            g[g.len() / 2]
        };
        assert!(median(true, &dark, 0.0) > median(false, &dark, 0.0) * 1.5, "a dark frame should be lifted");
        // the grey ramp, its clear film border outside the border buffer
        let normal = negative(w, h);
        assert!((median(true, &normal, 10.0) - median(false, &normal, 10.0)).abs() < 1e-4, "a normal frame should be left alone");
    }

    #[test]
    fn a_slide_keeps_its_orientation() {
        // a positive: bright on the left, dark on the right
        let (w, h) = (100, 10);
        let mut px: Vec<f32> = (0..w * h).flat_map(|i| [1.0 - (i % w) as f32 / w as f32 * 0.99; 3]).collect();
        let p = measured(&px, w, h, json!({"filmType": "Positive (Slide)", "borderBuffer": 0}));
        convert(&mut px, &p);
        assert!(px[3] > px[(w - 2) * 3]);
    }
}
