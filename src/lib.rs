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
  "version": "0.10.0",
  "kind": "filter",
  "stage": "source",
  "analyzeArea": "crop",
  "baseTone": "neutral",
  "description": "Converts film negatives and slides. Add it to a photo, set Type and Border buffer, then press Measure. Measure again after changing Border buffer.",
  "author": "",
  "overlap": 0,
  "params": {
    "filmType":     {"type": "choice", "label": "Type", "options": ["Color Negative", "B&W Negative", "Positive (Slide)"], "default": "Color Negative"},
    "method":       {"type": "choice", "label": "Method", "options": ["Shared range", "Per channel"], "default": "Shared range"},
    "look":         {"type": "choice", "label": "Look", "options": ["Standard", "Neutral", "Rich", "Clean Contrast", "Clean Contrast +", "Golden Hour", "Portrait Warm", "Portrait Airy", "Fashion Cool", "Crisp Scan", "Green Shadows", "Vivid Green", "Night Cine", "Archive Print"], "default": "Standard"},
    "stock":        {"type": "choice", "label": "Film stock", "options": ["Generic", "None (as scanned)", "Kodak Portra 160", "Kodak Portra 400", "Kodak Portra 800", "Kodak Ektar 100", "Kodak Gold 200", "Kodak UltraMax 400", "Kodak ColorPlus 200", "Kodak Vision3 50D", "Kodak Vision3 250D", "Kodak Vision3 500T", "CineStill 50D", "CineStill 400D", "CineStill 800T", "Fujifilm Superia X-TRA 400", "Fujicolor C200", "Fujifilm 200", "Fujifilm 400", "Fujifilm Pro 400H", "Harman Phoenix 200", "Harman Phoenix II", "Lomography 400", "Lomography 800"], "default": "Generic"},
    "preSaturation":{"type": "int",    "label": "Pre-saturation", "min": 1, "max": 5, "default": 3},
    "borderBuffer": {"type": "number", "label": "Border buffer %", "min": 0, "max": 25, "default": 10},
    "showBorder":   {"type": "bool",   "label": "Show border buffer (until Measure)", "default": false},

    "toneProfile":  {"type": "choice", "label": "Tone profile", "options": ["Standard", "Soft", "Hard", "Highlight Soft", "Highlight Hard", "Shadow Soft", "Shadow Hard", "Cine Log", "Cine Rich", "Cine Flat", "Linear", "Linear Gamma", "Linear Deep", "Linear Flat"], "default": "Standard"},
    "exposure":     {"type": "number", "label": "Exposure",   "min": -5,  "max": 5,   "default": 0},
    "brightness":   {"type": "number", "label": "Brightness", "min": -50, "max": 50,  "default": 0},
    "contrast":     {"type": "number", "label": "Contrast",   "min": -50, "max": 50,  "default": 0},
    "lights":       {"type": "number", "label": "Lights",     "min": -50, "max": 50,  "default": 0},
    "darks":        {"type": "number", "label": "Darks",      "min": -50, "max": 50,  "default": 0},
    "whites":       {"type": "number", "label": "Whites",     "min": -50, "max": 50,  "default": 0},
    "blacks":       {"type": "number", "label": "Blacks",     "min": -50, "max": 50,  "default": 0},
    "glow":         {"type": "number", "label": "Glow",       "min": -50, "max": 50,  "default": 0},
    "fade":         {"type": "number", "label": "Fade",       "min": -50, "max": 50,  "default": 0},
    "softHighs":    {"type": "bool",   "label": "Soft highs", "default": false},
    "softLows":     {"type": "bool",   "label": "Soft lows",  "default": false},
    "autoExposure": {"type": "bool",   "label": "Auto exposure", "default": false},
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

/// Pixel `i` of an interleaved RGB `f32` block, read in place.
fn pixel_at(bytes: &[u8], i: usize) -> Option<[f32; 3]> {
    let b = bytes.get(i.checked_mul(12)?..i.checked_mul(12)?.checked_add(12)?)?;
    let f = |k: usize| b.get(k..k + 4).and_then(|s| <[u8; 4]>::try_from(s).ok()).map(f32::from_le_bytes);
    Some([f(0)?, f(4)?, f(8)?])
}

/// Pixels converted per chunk: the image is never copied whole, so a call needn't grow memory.
const CHUNK_PX: usize = 65_536;

/// Working memory reserved when the host allocates the pixel block. Growing a module's memory in the
/// middle of `pc_filter` / `pc_analyze` can fail on the host's instruction budget in a way that
/// depends on the heap's layout, so everything those calls allocate is reserved up front, in
/// `pc_alloc`, which has a budget of its own.
const SCRATCH_BYTES: usize = 16 << 20;

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
    if n >= CHUNK_PX {
        // the pixel block: reserve working memory too (freed at once, kept by the allocator)
        drop(Vec::<u8>::with_capacity(SCRATCH_BYTES));
    }
    let v = vec![0u8; n.max(1)];
    let p = addr(&v);
    BLOCKS.with(|b| b.borrow_mut().insert(p, v));
    p as i32
}

#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn pc_filter(buf: i32, len: i32, w: i32, h: i32, channels: i32, format: i32, params: i32, params_len: i32) -> i32 {
    let Some((band_w, _)) = rgb_args(len, w, h, channels, format) else { return BAD_ARGS };
    let json = read_params(params, params_len);
    let p = convert::Params::from_json(&json);
    let conv = convert::Converter::new(&p);
    // the border buffer preview: where this band sits in the frame, and the measured area
    let overlay = p.shows_border().then(|| convert::BorderOverlay::new(json.get("_image"), band_w, p.border));
    let done = with_block(buf, len, |bytes| {
        let mut px: Vec<f32> = Vec::with_capacity(CHUNK_PX * 3);
        for (n, chunk) in bytes.chunks_mut(CHUNK_PX * 12).enumerate() {
            px.clear();
            px.extend(chunk.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)));
            conv.run(&mut px);
            if let Some(o) = &overlay {
                o.apply(&mut px, n * CHUNK_PX);
            }
            for (d, v) in chunk.as_chunks_mut::<4>().0.iter_mut().zip(px.iter()) {
                *d = v.to_le_bytes();
            }
        }
    });
    if done.is_some() { OK } else { BAD_ARGS }
}

#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn pc_analyze(buf: i32, len: i32, w: i32, h: i32, channels: i32, format: i32, params: i32, params_len: i32) -> i64 {
    let Some((w, h)) = rgb_args(len, w, h, channels, format) else { return 0 };
    let p = convert::Params::from_json(&read_params(params, params_len));
    let Some(a) = with_block(buf, len, |bytes| convert::analyze_with(|i| pixel_at(bytes, i), w, h, &p)) else { return 0 };
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
        let stocks: Vec<String> = convert::STOCKS.iter().map(|s| s.name.to_string()).collect();
        assert_eq!(options("stock"), stocks);
    }
}
