# ChromaNeg

Film negative conversion for [Chromaforge](https://github.com/emmertex/lightcraft-classic) ("the app"
below), the open-source raw developer. ChromaNeg is a WebAssembly pixel-filter plug-in that turns camera
or scanner captures of colour negatives, black-and-white negatives and slides into positives, in the
spirit of the two-step negative-conversion tools: **measure** the frame once, then every render
**converts** with that measurement. It adds film-stock corrections, creative looks, tone profiles, and Glow / Fade.

It uses the app's pixel-filter ABI (PhotoCraft plug-in ABI v1 with the `pc_analyze` v1.1 proposal; see
the app's `docs/plugins.md` ▸ *Pixel filters*), runs in its sandbox and asks for no permissions.

## Download and install

1. Download `chromaneg-<version>.wasm` from [Releases](../../releases).
2. In the app: File ▸ Plug-ins ▸ Plug-in Manager… ▸ Install…, and pick the file
   (or `lightcraft-classic-cli commands plugin.install '{"path": "…/chromaneg-<version>.wasm"}'`).
3. Installing a newer version replaces the old one (same plug-in id); press **Measure** again on photos
   measured with an older version when its notes say the measurement format changed.

**Compatibility:** Chromaforge v26.43 or newer with pixel-filter plug-ins.

**App features ChromaNeg asks for** (in its manifest; an app without them still runs it):
- **Measuring inside the crop** (`"analyzeArea": "crop"`): the border buffer applies inside your
  crop. In the app since [emmertex/lightcraft-classic#444](https://github.com/emmertex/lightcraft-classic/pull/444);
  older versions measure the whole uncropped photo, and on a cropped photo the *Show border buffer*
  overlay then doesn't match what's measured.
- **No second tone curve on raw files** (`"baseTone": "neutral"`): the app tones a raw after the
  Source stage with the camera's own curve (or its default raw curve), which on top of ChromaNeg's
  conversion adds contrast and saturation and moves the brightness. ChromaNeg asks the app to show
  its result as it is. That's on the `plugin-base-tone` branch of
  [nwh212/chromaforge-plugins](https://github.com/nwh212/chromaforge-plugins/tree/plugin-base-tone)
  and offered upstream; until an app release has it, raw captures (camera RAWs, raw DNGs) come out
  more contrasty than intended. TIFF and JPEG scans aren't affected.
- **Menu sections** (`"groups"` on the Film stock / look menu): the stocks by brand and the creative
  looks as collapsible sections. That's on the `plugin-choice-groups` branch of
  [nwh212/chromaforge-plugins](https://github.com/nwh212/chromaforge-plugins/tree/plugin-choice-groups)
  and offered upstream; until then the menu is one list, in the same order.

## Build from source

```sh
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown -p chromaneg
# → target/wasm32-unknown-unknown/release/chromaneg.wasm (≈170 KB, no imports)
```

Rust 1.88 or newer. `cargo test --workspace` runs everything natively (see *Test* below).

**Layout** (a Cargo workspace):

| Path | What |
|---|---|
| `crates/chromaneg-core/presets.toml` | the tone profiles, film stocks and creative looks as data: add or change one here (each lists only what differs from neutral; a typo stops the build with a message) |
| `crates/chromaneg-core` | the conversion, independent of the plug-in ABI: `params` (what the host sends), `presets` (profiles, stocks), `analysis` (Measure), `plan` (how each sample becomes light), `convert`, `overlay` (border buffer preview) |
| `crates/pixel-filter` | the plug-in plumbing any pixel filter needs (memory blocks, parameters, chunked conversion, the ABI checks), so a new plug-in is only its manifest and its maths |
| `plugins/chromaneg` | the plug-in: `manifest.json` (dialog order; the version and the menus are filled in from the code) and the ABI exports |
| `tools/bench`, `tools/dump-source` | the reference bench and its input converter (see DEVELOPING.md) |

The version lives in `plugins/chromaneg/Cargo.toml` only; the manifest takes it from there.

To change ChromaNeg, see [DEVELOPING.md](DEVELOPING.md): where things are, the reference bench
(`tools/bench`) and releasing.

## Use

1. **Capture:** expose so that no channel clips. The film base transmits a lot of red, so red clips
   first, and a clipped red channel comes out starved and blue. White-balance on the clear film
   border if you like, though the conversion removes the mask either way.
2. In Develop ▸ Plug-in Filters, add **ChromaNeg**. It asks for the **Source** stage, so it runs on
   the linear capture before every other develop step.
3. Crop to the frame if you like, then set **Type** and **Border buffer %**, so the film holder,
   sprocket holes and edge print lie outside the measured area. Tick **Show border buffer** to see it:
   everything outside the measured area is dimmed in safelight red until you press **Measure** with
   that border buffer, so exports of a measured photo never show it. Then press **Measure**. Until you measure, the filter only inverts the image, and the orange cast
   stays. After changing the crop or the border buffer, press Measure again.
4. Pick a **Film stock / look** and a **Tone profile**, then fine-tune with the sliders or with the normal develop
   sliders, which act on the converted positive.
5. **A whole roll:** set up one frame (Measure, stock, sliders), then Copy and Paste (or Sync) its
   settings to the other frames. The filter, its settings *and its measurement* go along, so the roll
   converts consistently, like a lab's roll analysis. A frame that needs its own colour can be
   measured on its own afterwards (select it, Measure).

## Controls

| Group | Parameters |
|---|---|
| Convert | Type (Color Negative, B&W Negative, Positive), Method (Shared range, Per channel), Film stock / look, Pre-saturation 1–5, Border buffer %, Show border buffer |
| Tone | Tone profile, Exposure, Brightness, Contrast, Lights, Darks, Whites, Blacks, Glow, Fade, Soft highs, Soft lows, Auto exposure, White clip, Black clip |
| Colour | WB (Auto-Neutral, Auto-Color, Auto-Warm, Auto-Cool, None), Temp, Tint |

**Glow** and **Fade** (−50 to +50) are curves on the display values, fitted to a reference
conversion's glow and fade settings (they reproduce them within about 4 levels of 255). *Glow* brightens
the mids and settles the whites near 80 %, for a soft, luminous print; below 0 it darkens the mids and
opens the highlights. *Fade* lifts the blacks to about 20 % and dips the lower mids, a matte print;
below 0 it deepens the shadows. +50 matches the references as captured; the slider values used for
them aren't known, so the scale may change.

**Auto exposure** (off by default) lifts a dark frame's median brightness toward mid grey, by at most
2 stops, never darker, and rolls the highlights off so a bright sky doesn't clip. It needs a measurement.

**Method** (colour negatives). *Shared range* (the default, since 0.10) gives the three dye layers
the same contrast and balances them by their offsets, as one film's layers respond alike: blue skies
stay blue, greens stay green, and colours keep their saturation instead of being stretched.
*Per channel* is the method of 0.9 and earlier: each layer stretched to its own range, balanced by
the edges in the frame. It suits some indoor and portrait frames better (warmer, closer to a lab's
look on those) but can oversaturate and tint frames dominated by one colour.

**WB modes.** *Auto-Neutral* (the default) removes the mask and balances the channels: with Shared
range, half of the remaining difference between the channels' averages; with Per channel, from the
edges in the frame, each channel's black neutral (see below). *Auto-Color* stretches each channel
between its own darkest and brightest values: stronger, but a large or very saturated object (blue
sky, a red sign) can tilt the result. *Warm* and *Cool* are Auto-Neutral with a bias. *None* only
removes the film base.

**Film stocks** gently correct a stock's known tendencies. Auto-Neutral already removes a frame's
overall cast, so a stock corrects what it can't: dye cross-talk (stronger for films without an
orange mask), contrast and the shadow toe, casts that differ between shadows and highlights, and
saturation by hue. *Generic* is the former "Basic" colour model and *None (as scanned)* the former "None".
Stocks are corrections; the creative looks at the end of the menu are bolder (below). Measure already maps each frame's
own black and white, so stocks barely touch contrast: a "contrasty" or "flat" stock is mostly
corrected by the measurement (v0.4's larger contrast values double-corrected, washing Phoenix out).

| Stock | Reported tendency | Correction |
|---|---|---|
| Kodak Portra 160 | flat, pastel, low saturation; slight magenta-blue bias | a little more contrast and saturation, tint toward green |
| Kodak Portra 400 | natural, low saturation; slight magenta-blue bias | tint toward green |
| Kodak Portra 800 | warmer, more saturated and contrasty than 160/400 | softer, cooler highlights |
| Kodak Ektar 100 | the most saturated C-41 stock; warm orange lean; reddish skin | less red and orange, slightly less saturation and warmth |
| Kodak Gold 200 | warm, reds and yellows pushed, contrasty | less red and yellow, cooler, softer |
| Kodak UltraMax 400 | saturated, punchier than Portra | slightly less saturation and contrast |
| Kodak ColorPlus 200 | soft colour, flat in dull light | a little more contrast and saturation |
| Kodak Vision3 50D / 250D / 500T | cine stocks, flat by design (made to be graded) | more contrast and saturation |
| CineStill 50D / 800T | Vision3 without remjet: flat-ish, red halation | more contrast (halation is optical and can't be undone per pixel) |
| CineStill 400D | medium contrast, occasional mint-green cast | tint away from green, mostly in the shadows |
| Fujifilm Superia X-TRA 400 | magenta casts reported in scans | tint toward green, less magenta |
| Fujicolor C200 | cool, green-leaning | slightly warmer, tint away from green |
| Fujifilm 200 (2022 on) | datasheet matches Kodak Gold 200 | corrected like Gold |
| Fujifilm 400 (US, 2023 on) | widely thought to be UltraMax-like (unconfirmed) | corrected like UltraMax |
| Fujifilm Pro 400H | fourth colour layer; pastel, greenish-blue cast | tint away from green, cooler greens and cyans |
| Harman Phoenix 200 | **no orange mask**, limited antihalation; high contrast, shadows fall off fast; bold reds and blues, weak yellows and skin; warm highlights over cold greenish shadows | stronger cross-talk correction, a little less red/blue and more yellow, a slight opposite split cast, cooler (its contrast is handled by Measure) |
| Harman Phoenix II | **no orange mask**; more normal contrast and colour than Phoenix I, cooler blues and greens, shadows still fall off fast | stronger cross-talk correction, slightly warmer |
| Lomography 400 / 800 | 400: balanced, slightly muted; 800: medium-high saturation | 400: a little more saturation; 800: slightly less |

These are starting points from published descriptions, not measurements: the datasheets' curves
and spectral dye densities are figures, and reviews depend on lab and scanner. Tune them against
your own scans. Sources:
- Masking chemistry: [C-41 process](https://en.wikipedia.org/wiki/C-41_process); Brian Pritchard,
  [Why colour negatives are orange](https://brianpritchard.com/wp/motion-picture-technology/__trashed-6/).
- Kodak: [technical documents index](https://125px.com/techdocs/kodak/) (Portra 400 E-4050, Ektar
  100 E-4046, Gold 200 E-7022, UltraMax 400 E-7023); [Ektar vs Portra](https://gridfiti.com/ektar-100-vs-portra);
  [Ektar 100 test](https://www.adorama.com/alc/product-review-kodak-ektar-100-120-roll-film-test);
  [Portra 160/400/800](https://www.l-camera-forum.com/topic/335509-portra-160-400-800-differences);
  [Gold 200](https://www.focusnordic.com/product-reviews/kodak-gold-200-review-by-bjorn-joachimsen/);
  [UltraMax 400](https://www.digitalcameraworld.com/cameras/film-cameras/kodak-ultramax-400-film-review);
  [UltraMax vs Gold](https://shootitwithfilm.com/film-comparison-kodak-ultramax-400-vs-kodak-gold-200-pushed-one-stop/);
  [ColorPlus 200](https://www.analog.cafe/r/kodak-colorplus-200-film-review-fygr);
  [Vision3 250D](https://www.35mmc.com/03/07/2019/c41-or-ecn2-to-process-kodak-vision-3-250d-movie-colour-negative-a-quick-comparison-by-phil-harrison).
- CineStill: [800T](https://brooklynfilmcamera.com/blogs/guides/cinestill-800t-review-the-night-photography-film),
  [400D](https://www.japancamerahunter.com/2022/10/film-review-cinestill-400d/).
- Fujifilm: [Superia](https://en.wikipedia.org/wiki/Fujifilm_Superia);
  [Superia X-TRA 400](https://www.analog.cafe/r/fujifilm-superia-x-tra-400-film-review-zc0y);
  [Pro 400H](https://analog.cafe/r/fujifilm-fujicolor-pro-400h-film-review-rau5);
  [Fujicolor 200 and Gold 200](https://petapixel.com/2022/01/10/fujifilms-new-fujicolor-200-looks-to-be-kodak-gold-200-in-disguise);
  [Fujifilm 400 and UltraMax](https://www.35mmc.com/11/07/2023/fujifilm-400-made-in-the-usa-vs-kodak-ultramax-400-the-same-or-different/).
- Harman: [Phoenix 200 store page](https://cinestillfilm.com/products/phoenix200-color-negative-film-35mm);
  [Phoenix 200 review](https://casualphotophile.com/2023/12/01/harman-phoenix-200-the-review/);
  [Phoenix 200 slide test](https://www.photrio.com/forum/threads/slide-with-harman-phoenix-200.212735/post-2882422);
  [Phoenix II release](https://kosmofoto.com/2025/07/harman-technology-releases-phoenix-ii-colour-negative-film/);
  [Phoenix II review](https://bluemooncameracodex.com/film-fridays/harman-phoenix-ii).
- Lomography: [Color 400](https://parallaxphotographic.coop/lomography-color-negative-400-film-review/),
  [Color 800](https://www.analog.cafe/r/lomography-color-negative-800-film-review-y45b).

**Creative looks** sit at the end of the same menu, as "Creative: …". They're own designs, bolder
than the stock corrections, and layered on the sliders, so every slider still starts at 0:

| Creative look | Aims for |
|---|---|
| Golden Hour | consumer gold film: warm, yellow highlights, saturated |
| Portrait Warm | professional portrait film: low contrast, warm skin, gentle colour |
| Portrait Airy | the bright, lifted, pastel version |
| Crisp Scan | lab scanner, crisp: cyan-leaning blues, firm greens |
| Night Cine | tungsten cine stock in daylight: teal shadows, warm highlights |
| Archive Print | optical print on archival paper: deep blacks, rich reds, warm |

Before 0.11 these were a separate Look menu. An edit that used one shows without it after the
update (the app passes a plug-in only the settings its manifest still lists): pick the same
"Creative: …" entry again, which renders exactly as the look did. Neutral, Rich, Clean Contrast,
Fashion Cool, Green Shadows and Vivid Green are gone.

**Tone profiles** turn the normalised value `t` into light. *Standard* was fitted to reference
conversions (Negative Lab Pro, its standard settings): linear light ≈ `t`^1.68 with Per channel
(`t`^2.1 with Shared range, whose `t` is spaced differently), with the top rolled off instead of
clipped. The others vary it: Soft, Hard; Highlight Soft / Hard and Shadow
Soft / Hard (shoulder or toe only); Cine Log (flat and lifted, for grading), Cine Rich, Cine Flat;
Linear (light = `t`), Linear Gamma (brighter mids), Linear Deep (darker), Linear Flat (brighter).

**Sharpening** is not part of this filter. A filter that reads neighbouring pixels makes the host
run it on the whole frame at full resolution after every parameter change, which is slow in the
plug-in interpreter. Use Develop ▸ Detail ▸ Sharpening on the converted image instead.

## How it works

Sources: own design on textbook film sensitometry; Per channel's colour balance from the grey-edge
hypothesis (van de Weijer, Gevers, Gijsenij, "Edge-Based Color Constancy", IEEE TIP 2007), applied to
densities; Shared range from fitting reference conversions (below). Details are in `src/convert.rs`.

**Shared range** (colour negatives, the default):
1. **Mask:** each channel is divided by its own thin-end level (5 % from the thin end), which removes
   the orange mask: a grey's three channels then agree whatever the mask's colour.
2. **Domain:** the result's cube root, a gamma-like scale on which reference conversions turned out
   to give every channel the same range.
3. **Ends:** black at green's thin end (1 %), white at its dense end (0.5 %), the same range for
   every channel. White / Black clip move these along 0, 0.1, 0.25, 0.5, 1, 1.5, 2, 3, 5, 8, 12 %.
4. **Balance:** half of the remaining difference between the channels' averages is removed (a
   frame's overall cast), the rest kept (a sunset stays warm).
5. The normalised value goes through the stock and the tone profile, then into light.

**Per channel** (0.9 and earlier; also slides and B&W), in density (`−log10` of the linear sample):
1. **Base:** the film base (orange mask plus fog) is removed as a per-channel density offset.
2. **Balance:** most density changes across edges are changes of light, not of colour, so the
   channels' edge strengths give each dye layer's contrast relative to green. Only edges between
   mid-tone pixels count (green density between its 10th and 80th percentiles).
3. **Ends:** each channel's black is its own darkest balanced density, so the shadows come out
   neutral. White comes from the brightness histogram, shared by the channels. The brightest 5 % of
   the frame is then neutralised when it's nearly neutral already (a white wall), and left alone when
   it's clearly coloured, with a fade between. At 0, white is at 5 % and black at 3 %.
4. The normalised value goes through the stock and the tone profile, then into light.

**Checked against references.** Twelve frames also converted with Negative Lab Pro (its standard
settings), each cropped to the picture and measured with a 3 % border buffer; the mean difference in
levels of 255. Eight are camera and scanner raws (Fuji 400, Kodak and others; the defaults are picked
against all twelve together, so none is tuned to a single photo); the last four are the earlier
references, from 8-bit exports of the negatives:

| Frame | 0.9 (= Per channel) | 0.10 (Shared range) |
|---|---|---|
| Canyon landscape, flatbed TIFF | 31.1 | 24.6 |
| Desert rocks and sky, flatbed TIFF | 28.9 | 21.5 |
| Lodge under a deep blue sky, Fuji 400 | 63.3 | 30.2 |
| Portrait outdoors, Fuji 400 | 20.1 | 18.6 |
| Street panorama, half frames | 21.9 | 12.7 |
| Indoor group portrait | 10.1 | 18.3 |
| Street and lawn, scanner DNG | 19.2 | 17.1 |
| Street, red car, scanner DNG | 27.8 | 16.8 |
| Colour negative portrait, indoors | 13.0 | 19.1 |
| Harman Phoenix 200 landscape, flatbed | 6.6 | 7.4 |
| Kodak 200 forest, daylight | 10.5 | 15.0 |
| Fuji 400 portrait outdoors | 35.8 | 24.4 |
| **Mean** | **24.0** | **18.8** |

Shared range is darker and less saturated where 0.9 overexposed and oversaturated (the blue sky came
out cyan-white, a lawn orange); on the indoor portraits it's cooler than the references, where
*Per channel* stays closer. The renders in the app match these figures within a few levels once the
app leaves the raw base tone out (above). Negative Lab Pro has no stock-specific corrections, so
these figures can only check that a stock stays gentle, not that its corrections are right.

Memory: the image is converted in chunks and measured in place, never copied whole, and the
working memory is reserved when the host allocates the pixel block. Growing memory in the middle of
a render or Measure can fail on the host's instruction budget depending on the heap's layout (0.7.0's
Measure failed that way on some photos with some settings). Measured margins on a 2.7 MP frame: Measure
uses about 1/40 of its budget, a render band about 1/9.

Speed: the logarithm and the tone curve are lookup tables. In the app's interpreter, a
2.7 MP render takes about 1–2 s (v0.1: 3.6 s), and Measure about 0.5 s: it reads at most 300 000
pixels, with each density computed once.

## Not done yet

- **Warmth of indoor frames.** Shared range balances half a frame's overall cast away, which cools
  tungsten-lit portraits more than the references do. More reference pairs (your scan + a conversion
  you like, across stocks and light) will tell how much of a cast to keep.
- **Roll analysis:** one shared measurement for a whole roll needs `_analysis` from several photos,
  which the ABI doesn't offer yet. The workaround is to measure a typical frame and copy the filter
  with its analysis.
- Scanner (gamma-encoded) input, per-stock film profiles, mids / highs / shadows colour balance, HSL, glow.

## Test

`cargo test --workspace` runs the maths and the plumbing natively:
- a synthetic orange-masked grey ramp comes out neutral and monotonic in each balance mode;
- a small saturated patch doesn't tint the neutrals;
- sprocket holes in the measured area are ignored, and the border preview hides once measured;
- every stock × profile renders finite values, and a film stock stays gentle;
- the lookup tables match the exact functions;
- analysis JSON round-trips, and the manifest's menus match the code;
- hostile input never panics.

`cargo run --release -p chromaneg-core --example develop -- in.f32 w h '{"borderBuffer":20}' out.f32`
runs Measure and a conversion on a raw linear `f32` image, for tuning by eye;
`cargo run -p chromaneg --example manifest` prints the manifest the app reads.
