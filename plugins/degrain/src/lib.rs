//! Degrain, the plug-in: its manifest and the ABI exports. The grain reduction is `degrain-core`;
//! the ABI plumbing is `pixel-filter`.
//!
//! It reads neighbouring pixels (manifest `overlap`), so the host runs it on the whole frame at the
//! source's resolution and keeps the result: a preview looks like the export, and other sliders
//! don't run it again.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use degrain_core::{OVERLAP, Params, degrain};

const TEMPLATE: &str = include_str!("../manifest.json");

/// Working memory reserved with the pixel block: the three planes, the transform's scratch and
/// the band as `f32` take about eight times the block.
const SCRATCH_FACTOR: usize = 8;

/// The manifest the host reads (`@version` and `@overlap` filled in from the code).
pub fn manifest() -> String {
    TEMPLATE.replace("\"@version\"", &format!("\"{}\"", env!("CARGO_PKG_VERSION"))).replace("\"@overlap\"", &OVERLAP.to_string())
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
    let scratch = usize::try_from(len).unwrap_or(0).saturating_mul(SCRATCH_FACTOR).max(pixel_filter::SCRATCH_BYTES);
    pixel_filter::alloc_reserving(len, scratch)
}

#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn pc_filter(buf: i32, len: i32, w: i32, h: i32, channels: i32, format: i32, params: i32, params_len: i32) -> i32 {
    pixel_filter::filter_band(buf, len, w, h, channels, format, params, params_len, |json, px, w, h| {
        degrain(px, w, h, &Params::from_json(json));
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn the_manifest_is_valid() {
        let m: Value = serde_json::from_str(&manifest()).unwrap();
        assert_eq!(m["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(m["overlap"], OVERLAP);
        assert_eq!(m["stage"], "source");
        for (k, spec) in m["params"].as_object().unwrap() {
            if let Some(opts) = spec["options"].as_array() {
                assert!(opts.contains(&spec["default"]), "{k}");
            }
        }
    }
}
