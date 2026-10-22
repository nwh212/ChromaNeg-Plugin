//! ChromaNeg: a pixel filter (PhotoCraft plug-in ABI v1, plus the v1.1 `pc_analyze` proposal)
//! that turns a camera or scanner capture of film into a positive.
//!
//! The workflow follows the familiar two-step negative-conversion plug-ins:
//!
//! 1. **Analyze** (`pc_analyze`, the *Measure* button in Develop ▸ Plug-in Filters): the host
//!    hands over the whole uncropped photo once; we measure, per channel, the film-base end and
//!    the densest end of the density histogram (inside the border buffer) and return them as JSON.
//! 2. **Convert** (`pc_filter`, every render): each band is converted with the stored measurement
//!    (`_analysis`), so crops, zoom and preview size don't change the result, and every develop
//!    slider after it acts on a normal positive.
//!
//! The ABI glue lives here and has no `unsafe` blocks: like the plug-in SDK, buffers handed out by
//! `pc_alloc` are ordinary `Vec`s kept in a table by address. The colour science is in
//! [`convert`], which is plain Rust and unit-tested natively.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod convert;

use std::cell::RefCell;
use std::collections::BTreeMap;

use serde_json::Value;

/// The manifest. Parameter order is dialog order; the groups mirror the Convert / Edit tabs.
pub const MANIFEST: &str = r#"{
  "id": "io.github.nwh212.chromaneg",
  "name": "ChromaNeg",
  "version": "0.2.0",
  "kind": "filter",
  "stage": "source",
  "description": "Converts film negatives and slides. Add it to a photo, set Type and Border buffer, then press Measure. Measure again after changing Border buffer.",
  "author": "",
  "overlap": 0,
  "params": {
    "filmType":     {"type": "choice", "label": "Type", "options": ["Color Negative", "B&W Negative", "Positive (Slide)"], "default": "Color Negative"},
    "look":         {"type": "choice", "label": "Look", "options": ["Standard", "Neutral", "Rich", "Clean Contrast", "Clean Contrast +", "Golden Hour", "Portrait Warm", "Portrait Airy", "Fashion Cool", "Crisp Scan", "Green Shadows", "Vivid Green", "Night Cine", "Archive Print"], "default": "Standard"},
    "colorModel":   {"type": "choice", "label": "Color model", "options": ["Basic", "None"], "default": "Basic"},
    "preSaturation":{"type": "int",    "label": "Pre-saturation", "min": 1, "max": 5, "default": 3},
    "borderBuffer": {"type": "number", "label": "Border buffer %", "min": 0, "max": 25, "default": 10},

    "toneProfile":  {"type": "choice", "label": "Tone profile", "options": ["Standard", "Soft", "Hard", "Highlight Soft", "Highlight Hard", "Shadow Soft", "Shadow Hard", "Cine Log", "Cine Rich", "Cine Flat", "Linear", "Linear Gamma", "Linear Deep", "Linear Flat"], "default": "Standard"},
    "exposure":     {"type": "number", "label": "Exposure",   "min": -5,  "max": 5,   "default": 0},
    "brightness":   {"type": "number", "label": "Brightness", "min": -50, "max": 50,  "default": 0},
    "contrast":     {"type": "number", "label": "Contrast",   "min": -50, "max": 50,  "default": 0},
    "lights":       {"type": "number", "label": "Lights",     "min": -50, "max": 50,  "default": 0},
    "darks":        {"type": "number", "label": "Darks",      "min": -50, "max": 50,  "default": 0},
    "whites":       {"type": "number", "label": "Whites",     "min": -50, "max": 50,  "default": 0},
    "blacks":       {"type": "number", "label": "Blacks",     "min": -50, "max": 50,  "default": 0},
    "fade":         {"type": "number", "label": "Fade",       "min": 0,   "max": 50,  "default": 0},
    "softHighs":    {"type": "bool",   "label": "Soft highs", "default": false},
    "softLows":     {"type": "bool",   "label": "Soft lows",  "default": false},
    "whiteClip":    {"type": "int",    "label": "White clip", "min": -2, "max": 4, "default": 0},
    "blackClip":    {"type": "int",    "label": "Black clip", "min": -2, "max": 4, "default": 0},

    "wbMode":       {"type": "choice", "label": "WB", "options": ["Auto-Neutral", "Auto-Color", "Auto-Warm", "Auto-Cool", "None"], "default": "Auto-Neutral"},
    "temp":         {"type": "number", "label": "Temp", "min": -50, "max": 50, "default": 0},
    "tint":         {"type": "number", "label": "Tint", "min": -50, "max": 50, "default": 0}
  }
}"#;

/// `pc_filter` result codes.
const OK: i32 = 0;
const BAD_ARGS: i32 = 1;

thread_local! {
    /// Blocks handed to the host by `pc_alloc`, by address. Each call runs in a fresh instance,
    /// so nothing here outlives one call.
    static BLOCKS: RefCell<BTreeMap<usize, Vec<u8>>> = const { RefCell::new(BTreeMap::new()) };
    /// Replies (manifest, analysis) kept alive for the host to read.
    static REPLIES: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
}

fn addr(v: &[u8]) -> usize {
    v.as_ptr() as usize
}

fn pack(v: Vec<u8>) -> i64 {
    let packed = ((v.len() as i64) << 32) | (addr(&v) as u32 as i64);
    REPLIES.with(|r| r.borrow_mut().push(v));
    packed
}

/// Runs `f` on the block at `ptr` (the host has written into it), if there is one of at least `len` bytes.
fn with_block<R>(ptr: i32, len: i32, f: impl FnOnce(&mut [u8]) -> R) -> Option<R> {
    let len = usize::try_from(len).ok()?;
    BLOCKS.with(|b| {
        let mut b = b.borrow_mut();
        let block = b.get_mut(&(ptr as u32 as usize))?;
        Some(f(block.get_mut(..len)?))
    })
}

fn read_params(ptr: i32, len: i32) -> Value {
    with_block(ptr, len, |bytes| serde_json::from_slice(bytes).ok()).flatten().unwrap_or(Value::Null)
}

fn read_pixels(bytes: &[u8]) -> Vec<f32> {
    bytes.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect()
}

/// The ABI checks shared by `pc_filter` and `pc_analyze`: 3-channel f32 RGB and a buffer that fits.
fn rgb_args(len: i32, w: i32, h: i32, channels: i32, format: i32) -> Option<(usize, usize)> {
    let depth = format & 0xffff;
    let (w, h) = (usize::try_from(w).ok()?, usize::try_from(h).ok()?);
    let need = w.checked_mul(h)?.checked_mul(3)?.checked_mul(4)?;
    (channels == 3 && depth == 32 && usize::try_from(len).ok()? >= need).then_some((w, h))
}

#[unsafe(no_mangle)]
pub extern "C" fn pc_abi_version() -> i32 {
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn pc_manifest() -> i64 {
    pack(MANIFEST.as_bytes().to_vec())
}

#[unsafe(no_mangle)]
pub extern "C" fn pc_alloc(len: i32) -> i32 {
    let Ok(n) = usize::try_from(len) else { return 0 };
    let v = vec![0u8; n.max(1)];
    let p = addr(&v);
    BLOCKS.with(|b| b.borrow_mut().insert(p, v));
    p as i32
}

#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn pc_filter(buf: i32, len: i32, w: i32, h: i32, channels: i32, format: i32, params: i32, params_len: i32) -> i32 {
    let Some(_) = rgb_args(len, w, h, channels, format) else { return BAD_ARGS };
    let p = convert::Params::from_json(&read_params(params, params_len));
    let done = with_block(buf, len, |bytes| {
        let mut px = read_pixels(bytes);
        convert::convert(&mut px, &p);
        for (d, v) in bytes.as_chunks_mut::<4>().0.iter_mut().zip(px) {
            *d = v.to_le_bytes();
        }
    });
    if done.is_some() { OK } else { BAD_ARGS }
}

#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn pc_analyze(buf: i32, len: i32, w: i32, h: i32, channels: i32, format: i32, params: i32, params_len: i32) -> i64 {
    let Some((w, h)) = rgb_args(len, w, h, channels, format) else { return 0 };
    let p = convert::Params::from_json(&read_params(params, params_len));
    let Some(px) = with_block(buf, len, |bytes| read_pixels(bytes)) else { return 0 };
    let a = convert::analyze(&px, w, h, &p);
    pack(a.to_json().to_string().into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(key: &str) -> Vec<String> {
        let m: Value = serde_json::from_str(MANIFEST).unwrap();
        m["params"][key]["options"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect()
    }

    #[test]
    fn the_manifest_menus_list_exactly_the_looks_and_profiles() {
        let looks: Vec<String> = convert::LOOKS.iter().map(|l| l.name.to_string()).collect();
        let profiles: Vec<String> = convert::PROFILES.iter().map(|p| p.name.to_string()).collect();
        assert_eq!(options("look"), looks);
        assert_eq!(options("toneProfile"), profiles);
    }
}
