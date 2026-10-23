//! The plumbing every pixel-filter plug-in needs (PhotoCraft plug-in ABI v1, plus the v1.1
//! `pc_analyze` proposal; see the app's `docs/plugins.md` ▸ *Pixel filters*), so a plug-in crate
//! is only its manifest and its maths. A plug-in exports the ABI's functions and forwards them here:
//!
//! ```ignore
//! #[unsafe(no_mangle)]
//! pub extern "C" fn pc_alloc(len: i32) -> i32 {
//!     pixel_filter::alloc(len)
//! }
//! ```
//!
//! No `unsafe`: like the plug-in SDK, buffers handed out by [`alloc`] are ordinary `Vec`s kept in
//! a table by address. Each host call runs in a fresh instance, so nothing outlives one call.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::cell::RefCell;
use std::collections::BTreeMap;

use serde_json::Value;

/// The ABI version every plug-in reports from `pc_abi_version`.
pub const ABI_VERSION: i32 = 1;

/// `pc_filter` result codes.
pub const OK: i32 = 0;
pub const BAD_ARGS: i32 = 1;

/// Pixels handled per chunk: an image is never copied whole, so a call needn't grow memory.
pub const CHUNK_PX: usize = 65_536;

/// Working memory reserved when the host allocates the pixel block. Growing a module's memory in the
/// middle of `pc_filter` / `pc_analyze` can fail on the host's instruction budget in a way that
/// depends on the heap's layout, so everything those calls allocate is reserved up front, in
/// [`alloc`], which has a budget of its own.
pub const SCRATCH_BYTES: usize = 16 << 20;

thread_local! {
    /// Blocks handed to the host by [`alloc`], by address.
    static BLOCKS: RefCell<BTreeMap<usize, Vec<u8>>> = const { RefCell::new(BTreeMap::new()) };
    /// Replies (manifest, analysis) kept alive for the host to read.
    static REPLIES: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
}

fn addr(v: &[u8]) -> usize {
    v.as_ptr() as usize
}

/// Hands `v` to the host: the `len << 32 | ptr` that `pc_manifest` and `pc_analyze` return.
pub fn reply(v: Vec<u8>) -> i64 {
    let packed = ((v.len() as i64) << 32) | (addr(&v) as u32 as i64);
    REPLIES.with(|r| r.borrow_mut().push(v));
    packed
}

/// `pc_alloc`: a zeroed block of `len` bytes for the host to write into (0 on a bad length).
pub fn alloc(len: i32) -> i32 {
    let Ok(n) = usize::try_from(len) else { return 0 };
    if n >= CHUNK_PX {
        // the pixel block: reserve working memory too (freed at once, kept by the allocator)
        drop(Vec::<u8>::with_capacity(SCRATCH_BYTES));
    }
    let v = vec![0u8; n.max(1)];
    // keyed by the address as the host sees it (32 bits: all of it in wasm32), so the same lookup
    // also works in native tests
    let p = addr(&v) as u32;
    BLOCKS.with(|b| b.borrow_mut().insert(p as usize, v));
    p as i32
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

/// The parameters the host sent (`Null` when they're missing or not JSON).
pub fn read_params(ptr: i32, len: i32) -> Value {
    with_block(ptr, len, |bytes| serde_json::from_slice(bytes).ok()).flatten().unwrap_or(Value::Null)
}

/// Pixel `i` of an interleaved RGB `f32` block, read in place.
fn pixel_at(bytes: &[u8], i: usize) -> Option<[f32; 3]> {
    let b = bytes.get(i.checked_mul(12)?..i.checked_mul(12)?.checked_add(12)?)?;
    let f = |k: usize| b.get(k..k + 4).and_then(|s| <[u8; 4]>::try_from(s).ok()).map(f32::from_le_bytes);
    Some([f(0)?, f(4)?, f(8)?])
}

/// The checks `pc_filter` and `pc_analyze` share: 3-channel f32 RGB and a buffer that fits.
/// `(width, height)` when they pass.
pub fn rgb_args(len: i32, w: i32, h: i32, channels: i32, format: i32) -> Option<(usize, usize)> {
    let depth = format & 0xffff;
    let (w, h) = (usize::try_from(w).ok()?, usize::try_from(h).ok()?);
    let need = w.checked_mul(h)?.checked_mul(3)?.checked_mul(4)?;
    (channels == 3 && depth == 32 && usize::try_from(len).ok()? >= need).then_some((w, h))
}

/// `pc_filter` for a point filter on interleaved RGB `f32`. `setup` gets the parameters and the
/// band's width and returns the per-chunk worker, which gets interleaved samples (converted in place)
/// and the index of the chunk's first pixel in the band.
#[allow(clippy::too_many_arguments)]
pub fn filter<F: FnMut(&mut [f32], usize)>(
    buf: i32,
    len: i32,
    w: i32,
    h: i32,
    channels: i32,
    format: i32,
    params: i32,
    params_len: i32,
    setup: impl FnOnce(&Value, usize) -> F,
) -> i32 {
    let Some((band_w, _)) = rgb_args(len, w, h, channels, format) else { return BAD_ARGS };
    let json = read_params(params, params_len);
    let mut work = setup(&json, band_w);
    let done = with_block(buf, len, |bytes| {
        let mut px: Vec<f32> = Vec::with_capacity(CHUNK_PX * 3);
        for (n, chunk) in bytes.chunks_mut(CHUNK_PX * 12).enumerate() {
            px.clear();
            px.extend(chunk.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)));
            work(&mut px, n * CHUNK_PX);
            for (d, v) in chunk.as_chunks_mut::<4>().0.iter_mut().zip(px.iter()) {
                *d = v.to_le_bytes();
            }
        }
    });
    if done.is_some() { OK } else { BAD_ARGS }
}

/// `pc_analyze` on interleaved RGB `f32`: `measure` gets the parameters, the image size and a
/// reader of pixel `i` (`y · w + x`, read in place), and returns the analysis (or `None`).
#[allow(clippy::too_many_arguments)]
pub fn analyze(
    buf: i32,
    len: i32,
    w: i32,
    h: i32,
    channels: i32,
    format: i32,
    params: i32,
    params_len: i32,
    measure: impl FnOnce(&Value, usize, usize, &dyn Fn(usize) -> Option<[f32; 3]>) -> Option<Value>,
) -> i64 {
    let Some((w, h)) = rgb_args(len, w, h, channels, format) else { return 0 };
    let json = read_params(params, params_len);
    match with_block(buf, len, |bytes| measure(&json, w, h, &|i| pixel_at(bytes, i))).flatten() {
        Some(a) => reply(a.to_string().into_bytes()),
        None => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostile_arguments_are_refused_not_panicked_on() {
        for (len, w, h, ch, fmt) in
            [(0, 1, 1, 3, 32), (12, 2, 1, 3, 32), (12, 1, 1, 4, 32), (12, 1, 1, 3, 16), (12, -1, 1, 3, 32), (12, i32::MAX, i32::MAX, 3, 32)]
        {
            assert!(rgb_args(len, w, h, ch, fmt).is_none(), "{len} {w} {h} {ch} {fmt}");
        }
        assert_eq!(rgb_args(12, 1, 1, 3, 32), Some((1, 1)));
        // a block the host never allocated
        assert_eq!(filter(12345, 12, 1, 1, 3, 32, 0, 0, |_, _| |_: &mut [f32], _| {}), BAD_ARGS);
        assert_eq!(analyze(12345, 12, 1, 1, 3, 32, 0, 0, |_, _, _, _| Some(Value::Null)), 0);
        assert_eq!(read_params(-1, 5), Value::Null);
        assert_eq!(alloc(-1), 0);
    }

    #[test]
    fn a_filter_sees_every_pixel_once_in_order() {
        let (w, h) = (300, 250); // more than one chunk
        let n = w * h;
        let buf = alloc((n * 12) as i32);
        let mut seen = Vec::new();
        let r = filter(buf, (n * 12) as i32, w as i32, h as i32, 3, 32, 0, 0, |_, band_w| {
            assert_eq!(band_w, w);
            |px: &mut [f32], start: usize| {
                seen.push((start, px.len() / 3));
                px.iter_mut().for_each(|v| *v = 0.5);
            }
        });
        assert_eq!(r, OK);
        let mut next = 0;
        for (start, count) in seen {
            assert_eq!(start, next);
            next += count;
        }
        assert_eq!(next, n);
        let mean = analyze(buf, (n * 12) as i32, w as i32, h as i32, 3, 32, 0, 0, |_, w, h, px| {
            Some(Value::from((0..w * h).filter_map(px).map(|p| f64::from(p[1])).sum::<f64>() / (w * h) as f64))
        });
        assert_ne!(mean, 0);
    }
}
