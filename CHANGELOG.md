# Changelog

Measurements made with one version can need **Measure** again in a later one; the notes say when.
"Reference error" is the mean difference from a Negative Lab Pro conversion of the same scan, in
levels of 255 (see the README).

## 0.10.0 (Measure again)
- **New default method for colour negatives, "Shared range"**: the dye layers get the same
  contrast and are balanced by their offsets, as fitted to Negative Lab Pro conversions of eight
  camera and scanner raws. Blue skies stay deep blue instead of cyan-white, lawns green instead of
  orange, and frames are no longer overexposed or oversaturated. Reference error over twelve frames:
  24.0 → 18.8. The 0.9 method stays as **Method ▸ Per channel**, closer on some indoor portraits.
- **Standard tone profile darker in the mids** with Shared range (its fitted curve).
- **Asks the app not to tone raw files a second time** (`"baseTone": "neutral"`): with an app that
  supports it, a converted RAW is shown as ChromaNeg renders it, without the camera's contrast curve
  on top (see the README).
- **Border buffer preview leaves exports alone**: it shows until you press Measure with that border
  buffer, so a measured photo never exports with it.

## 0.9.1 (Measure again)
- **Sprocket holes are ignored**: bare light inside the measured area (sprocket holes, gaps in the
  film holder) no longer sets the black point. Before, holes in a narrow border buffer gave a
  squashed, oversaturated, orange result. Reference error on a Fuji 400 frame at the default border
  buffer: 28.6 → 22.3; the earlier references are unchanged.

## 0.9.0 (Measure again)
- **Measures inside your crop**: the border buffer now applies after cropping, in apps that support
  `"analyzeArea": "crop"` (see the README); others still measure the whole photo.
- **Show border buffer**: a checkbox that dims everything outside the measured area in safelight red
  while you set the border buffer.

## 0.8.0
- **Glow** (new) and **Fade** (now −50 to +50), fitted to reference conversions: Glow brightens the
  mids and settles the whites; Fade lifts the blacks to a matte print. Negative values push the other way.

## 0.7.1
- Fix: Measure could fail in the app ("filter exceeded its instruction budget") on some photos with
  some settings. The image is no longer copied whole, and working memory is reserved up front.

## 0.7.0 (Measure again)
- Tone curve fitted to three reference conversions (Standard: light ≈ t^1.68).

## 0.6.0 (Measure again)
- Default black / white points 3 % / 5 %, fitted to two references; stocks no longer double-correct
  contrast (the Harman Phoenix stocks were washed out).

## 0.5.0 (Measure again)
- Output curve and colour balance fitted to a reference conversion; mid-tone-only edge balance; gated
  highlight neutralisation. Auto exposure off by default. Reference error 19.6 → 9.9 on the first frame.

## 0.4.0
- **Film stock** menu (24 stocks: Kodak, Fujifilm, CineStill, Harman Phoenix, Lomography), replacing
  the Basic / None colour model.

## 0.3.0 (Measure again)
- Neutral shadows (per-channel black) and auto exposure; Copy / Paste carries the measurement (roll workflow).

## 0.2.1
- Fix: Measure ran out of the app's instruction budget on full-size photos.

## 0.2.0
- Renamed ChromaNeg. Grey-edge colour balance, brightness-based ends, 14 looks, 14 tone profiles,
  lookup tables for speed.

## 0.1.0
- First version (as "Negative Lab").
