//! Tone profiles, looks and film stocks: the menus' entries and their values.

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

pub(crate) const fn profile(name: &'static str, slopes: [f32; 3], power: f32, lift: f32, gamma: f32, sat: f32) -> Profile {
    Profile { name, slopes, power, lift, gamma, sat }
}

/// The tone profiles, in menu order. The first is the default.
pub const PROFILES: [Profile; 14] = [
    // fitted to three reference conversions: linear light ≈ t^1.68, highlights rolled off at the top
    profile("Standard", [1.0, 1.0, 1.0], 1.68, 0.0, 1.0, 1.0),
    profile("Soft", [1.15, 0.88, 1.15], 1.57, 0.0, 1.0, 0.97),
    profile("Hard", [0.8, 1.25, 0.8], 1.79, 0.0, 1.0, 1.04),
    profile("Highlight Soft", [1.0, 1.05, 0.6], 1.68, 0.0, 1.0, 1.0),
    profile("Highlight Hard", [1.0, 0.95, 1.3], 1.68, 0.0, 1.0, 1.0),
    profile("Shadow Soft", [1.3, 0.95, 1.0], 1.62, 0.0, 1.0, 1.0),
    profile("Shadow Hard", [0.7, 1.1, 1.0], 1.74, 0.0, 1.0, 1.0),
    // flat starting points for grading: not scaled with Standard
    profile("Cine Log", [1.2, 0.8, 1.2], 1.0, 0.05, 1.0, 0.85),
    profile("Cine Rich", [0.75, 1.3, 0.75], 1.74, 0.0, 1.0, 1.12),
    profile("Cine Flat", [1.1, 0.9, 1.1], 1.34, 0.02, 1.0, 0.92),
    profile("Linear", [1.0, 1.0, 1.0], 1.0, 0.0, 1.0, 1.0),
    profile("Linear Gamma", [1.0, 1.0, 1.0], 1.0, 0.0, 0.85, 1.0),
    profile("Linear Deep", [1.0, 1.0, 1.0], 2.0, 0.0, 1.0, 1.0),
    profile("Linear Flat", [1.0, 1.0, 1.0], 1.4, 0.0, 1.0, 1.0),
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
pub(crate) const fn look(
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
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn stock(
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

pub(crate) const FLAT: [f32; 6] = [1.0; 6];
pub(crate) const NONE3: [f32; 3] = [0.0; 3];

/// The film stocks, in menu order. The first is the default. Each corrects the stock's reported
/// tendencies gently; the comments say what (sources in the README). Tint < 0 counters magenta.
pub const STOCKS: [Stock; 24] = [
    // the generic masked-film correction (the former "Basic" colour model)
    stock("Generic", 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, NONE3, NONE3, FLAT),
    // no correction at all (the former "None")
    stock("None (as scanned)", 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, NONE3, NONE3, FLAT),
    // Kodak Portra 160: flat, pastel, slightly desaturated by design; slight magenta-blue bias
    stock("Kodak Portra 160", 1.0, 1.05, 1.4, 0.0, 0.0, -2.0, NONE3, [-0.003, 0.0, 0.003], [1.0, 1.03, 1.02, 1.0, 1.0, 1.0]),
    // Kodak Portra 400: natural, low saturation; slight magenta-blue bias
    stock("Kodak Portra 400", 1.0, 1.0, 0.0, 0.0, 0.0, -2.0, NONE3, NONE3, FLAT),
    // Kodak Portra 800: warmer, more saturated and contrasty than its siblings
    stock("Kodak Portra 800", 1.0, 0.97, -1.4, 0.0, -2.0, 0.0, NONE3, [-0.004, 0.0, 0.004], FLAT),
    // Kodak Ektar 100: the most saturated C-41 stock, warm orange lean, reddish skin
    stock("Kodak Ektar 100", 1.1, 0.93, -1.0, 0.0, -3.0, 0.0, NONE3, NONE3, [0.88, 0.94, 1.0, 1.0, 1.0, 0.96]),
    // Kodak Gold 200: warm, reds and yellows pushed, contrasty; weak on overcast days
    stock("Kodak Gold 200", 1.0, 1.0, -1.4, 0.0, -4.0, 0.0, NONE3, NONE3, [0.92, 0.9, 1.02, 1.0, 1.0, 1.0]),
    // Kodak UltraMax 400: saturated and a bit punchier than Portra, slightly cooler than Gold
    stock("Kodak UltraMax 400", 1.0, 0.95, -1.0, 0.0, -1.0, 0.0, NONE3, NONE3, FLAT),
    // Kodak ColorPlus 200: soft colour, moderate contrast, flat in dull light
    stock("Kodak ColorPlus 200", 1.0, 1.06, 1.4, 0.0, 0.0, 0.0, NONE3, NONE3, FLAT),
    // Kodak Vision3 50D (ECN-2 cine stock): flat by design, meant to be graded
    stock("Kodak Vision3 50D", 1.0, 1.04, 2.8, 0.0, 0.0, 0.0, NONE3, NONE3, FLAT),
    // Kodak Vision3 250D: flat by design
    stock("Kodak Vision3 250D", 1.0, 1.06, 3.5, 0.0, 0.0, 0.0, NONE3, NONE3, FLAT),
    // Kodak Vision3 500T: flat, tungsten-balanced (Auto-Neutral removes the daylight blue)
    stock("Kodak Vision3 500T", 1.0, 1.05, 3.5, 0.0, 0.0, 0.0, NONE3, NONE3, FLAT),
    // CineStill 50D (Vision3 50D without remjet): fine grain, accurate; red halation
    stock("CineStill 50D", 1.0, 1.03, 2.1, 0.0, 0.0, 0.0, NONE3, NONE3, FLAT),
    // CineStill 400D: medium contrast, occasional mint-green cast
    stock("CineStill 400D", 1.0, 1.04, 2.1, 0.0, 0.0, 4.0, [0.0, -0.004, 0.0], NONE3, FLAT),
    // CineStill 800T (Vision3 500T without remjet): flat-ish, tungsten-balanced; red halation
    stock("CineStill 800T", 1.0, 1.04, 2.8, 0.0, 0.0, 0.0, NONE3, NONE3, FLAT),
    // Fujifilm Superia X-TRA 400: magenta casts reported in scans
    stock("Fujifilm Superia X-TRA 400", 1.0, 1.0, 0.0, 0.0, 0.0, -5.0, NONE3, NONE3, [1.0, 1.0, 1.04, 1.0, 1.0, 0.9]),
    // Fujicolor C200 (the classic Fuji emulsion): cool, green-leaning
    stock("Fujicolor C200", 1.0, 1.02, 0.0, 0.0, 2.0, 2.0, NONE3, NONE3, [1.0, 1.0, 0.97, 1.0, 1.0, 1.0]),
    // Fujifilm 200 (2022 on): its datasheet matches Kodak Gold 200's, so corrected like Gold
    stock("Fujifilm 200", 1.0, 1.0, -1.0, 0.0, -3.0, 0.0, NONE3, NONE3, [0.93, 0.92, 1.02, 1.0, 1.0, 1.0]),
    // Fujifilm 400 (US, 2023 on): widely thought to be UltraMax-like; corrected like UltraMax
    stock("Fujifilm 400", 1.0, 0.96, -1.0, 0.0, -1.0, 0.0, NONE3, NONE3, FLAT),
    // Fujifilm Pro 400H (discontinued, fourth colour layer): pastel, greenish-blue cast
    stock("Fujifilm Pro 400H", 1.0, 1.03, 0.0, 0.0, 2.0, 3.0, NONE3, NONE3, [1.0, 1.0, 0.96, 0.97, 1.0, 1.0]),
    // Harman Phoenix 200 (I): no orange mask, high contrast, shadows fall off fast; bold reds and
    // blues, weak yellows and skin; warm highlights over cold, greenish shadows
    stock("Harman Phoenix 200", 1.4, 1.0, 0.0, 0.0, -3.0, 0.0, [0.003, -0.004, 0.001], [-0.003, 0.0, 0.003], [0.92, 1.08, 1.0, 0.97, 0.94, 1.0]),
    // Harman Phoenix II: no orange mask; more normal contrast and colour than Phoenix I, cooler
    // blues and greens, shadows still fall off fast
    stock("Harman Phoenix II", 1.3, 1.0, 0.0, 0.0, 2.0, 1.0, [0.002, -0.003, 0.0], NONE3, [0.97, 1.03, 0.98, 0.97, 0.97, 1.0]),
    // Lomography Color Negative 400 (thought to be Kodak-made): balanced, slightly muted
    stock("Lomography 400", 1.0, 1.06, 0.0, 0.0, 0.0, 0.0, NONE3, NONE3, FLAT),
    // Lomography Color Negative 800: medium-high saturation, natural skin
    stock("Lomography 800", 1.0, 0.98, -0.7, 0.0, 0.0, 0.0, NONE3, NONE3, FLAT),
];
