# Changelog

Measurements made with one version can need **Measure** again in a later one; the notes say when.
"Reference error" is the mean difference from a Negative Lab Pro conversion of the same scan, in
levels of 255 (see the README).

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
