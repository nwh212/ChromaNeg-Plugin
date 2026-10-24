//! ChromaLab's conversion, independent of the plug-in ABI (natively testable).
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
//! the film stock and the sliders, and goes back to linear light at the end.
//!
//! Modules: [`params`] (what the host sends), [`presets`] (profiles, stocks and creative looks), [`analysis`]
//! (Measure), [`plan`] (how each sample becomes light), [`convert`] (running it), [`overlay`]
//! (the border buffer preview), and small numeric helpers in `math`.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod analysis;
pub mod convert;
mod math;
pub mod overlay;
pub mod params;
pub mod plan;
pub mod presets;
#[cfg(test)]
mod tests;

pub use analysis::{Analysis, NP, PERCENTILES, analyze, analyze_with};
pub use convert::{Converter, convert};
pub use overlay::BorderOverlay;
pub use params::{FilmType, Params, Wb};
pub use presets::{PROFILES, Profile, STOCKS, Stock};
