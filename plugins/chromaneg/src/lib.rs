//! ChromaNeg, the plug-in: its manifest and the ABI exports. The conversion is
//! `chromaneg-core`; the ABI plumbing is `pixel-filter`.
//!
//! The workflow follows the familiar two-step negative-conversion plug-ins:
//!
//! 1. **Analyze** (`pc_analyze`, the *Measure* button in Develop ▸ Plug-in Filters): the host
//!    hands over the photo (inside the crop, with `"analyzeArea": "crop"`); we measure it inside the
//!    border buffer and return the measurement as JSON.
//! 2. **Convert** (`pc_filter`, every render): each band is converted with the stored measurement
//!    (`_analysis`), so crops, zoom and preview size don't change the result, and every develop
//!    slider after it acts on a normal positive.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use chromaneg_core::{BorderOverlay, Converter, PROFILES, Params, STOCKS, analyze_with};

/// The manifest as written, in dialog order; `@version` and the menus of stocks (with the creative
/// looks) and profiles are filled in from the code by [`manifest`].
const TEMPLATE: &str = include_str!("../manifest.json");

/// The manifest the host reads.
pub fn manifest() -> String {
    let names = |n: Vec<&str>| serde_json::Value::from(n).to_string();
    TEMPLATE
        .replace("\"@version\"", &format!("\"{}\"", env!("CARGO_PKG_VERSION")))
        .replace("\"@stocks\"", &names(STOCKS.iter().map(|s| s.name).collect()))
        .replace("\"@stock_groups\"", &names(STOCKS.iter().map(|s| s.group).collect()))
        .replace("\"@profiles\"", &names(PROFILES.iter().map(|p| p.name).collect()))
}

#[unsafe(no_mangle)]
pub extern "C" fn pc_abi_version() -> i32 {
    pixel_filter::ABI_VERSION
}

#[unsafe(no_mangle)]
pub extern "C" fn pc_manifest() -> i64 {
    pixel_filter::reply(manifest().into_bytes())
}

#[unsafe(no_mangle)]
pub extern "C" fn pc_alloc(len: i32) -> i32 {
    pixel_filter::alloc(len)
}

#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn pc_filter(buf: i32, len: i32, w: i32, h: i32, channels: i32, format: i32, params: i32, params_len: i32) -> i32 {
    pixel_filter::filter(buf, len, w, h, channels, format, params, params_len, |json, band_w| {
        let p = Params::from_json(json);
        let conv = Converter::new(&p);
        // the border buffer preview: where this band sits in the frame, and the measured area
        let overlay = p.shows_border().then(|| BorderOverlay::new(json.get("_image"), band_w, p.border));
        move |px: &mut [f32], start: usize| {
            conv.run(px);
            if let Some(o) = &overlay {
                o.apply(px, start);
            }
        }
    })
}

#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn pc_analyze(buf: i32, len: i32, w: i32, h: i32, channels: i32, format: i32, params: i32, params_len: i32) -> i64 {
    pixel_filter::analyze(buf, len, w, h, channels, format, params, params_len, |json, w, h, pixel| {
        Some(analyze_with(pixel, w, h, &Params::from_json(json)).to_json())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn the_manifest_is_valid_and_its_menus_are_the_codes() {
        let m: Value = serde_json::from_str(&manifest()).unwrap();
        assert_eq!(m["version"], env!("CARGO_PKG_VERSION"));
        let options =
            |key: &str| -> Vec<String> { m["params"][key]["options"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect() };
        assert_eq!(options("stock"), STOCKS.iter().map(|s| s.name).collect::<Vec<_>>());
        assert_eq!(options("toneProfile"), PROFILES.iter().map(|p| p.name).collect::<Vec<_>>());
        // one menu group per stock, each group's stocks next to each other
        let groups: Vec<&str> = m["params"]["stock"]["groups"].as_array().unwrap().iter().map(|g| g.as_str().unwrap()).collect();
        assert_eq!(groups.len(), STOCKS.len());
        let mut seen: Vec<&str> = Vec::new();
        for w in groups.windows(2).filter(|w| w[0] != w[1]) {
            assert!(!seen.contains(&w[1]), "group {:?} is split", w[1]);
            seen.push(w[0]);
        }
        // every choice's default is one of its options
        for (k, spec) in m["params"].as_object().unwrap() {
            if let Some(opts) = spec["options"].as_array() {
                assert!(opts.contains(&spec["default"]), "{k}");
            }
        }
    }
}
