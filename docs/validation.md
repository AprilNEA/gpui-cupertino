# Foundation validation

Measured on macOS 26.4 (25E246), Apple M5 Max, on 2026-09-28–29. These are
independent numerical and behavioral acceptance checks. They do not establish
visual equivalence with Apple's native material styles.

## Independent rendering reference

The CPU reference integrates a continuous Gaussian using Simpson quadrature.
It shares no kernel weights, sampling strides, scale interpolation, or filtering
implementation with the Metal path. Captured pixels are constant unit cells;
convolution is sampled at destination pixel centers. The production filter uses
`libm::erf`, normalized four-sigma support, and paired linear texture sampling.

Coverage is checked against intersected pixel-cell area on straight edges.
Composition is checked analytically in linear RGB, including saturation, tint,
brightness, and ancestor opacity. Thresholds were fixed before the initial run
and were retained when the initial implementation failed.

| Worst observed metric | Initial implementation | Corrected implementation | Acceptance limit |
| --- | ---: | ---: | ---: |
| Gaussian energy error | 0.425% | 0.238% | 1.5% |
| Gaussian centroid drift | 0 px | 0 px | 0.05 px |
| Gaussian variance error | 21.894% | 0.694% | 5% |
| Normalized kernel L1 distance | 0.38404 | 0.01691 | 0.10 |
| Step maximum linear-light error | 0.08247 | 0.00435 | 0.015 |
| Fractional-edge error, encoded channel levels | 188 | 0 | 1 |
| Color/opacity error, encoded channel levels | 0 | 0 | 1 |

Variance is compared with the integrated reference's discrete second moment,
which includes the pixel-cell convention; it is not compared directly with
sigma squared. The matrix covers sigma 0.5/1/2/4/8 logical pixels, 1×/2×,
both axes, two odd extents, and three translated impulses. All 197 reported
metric lines matched across three independent runs. The exported six profiles
contained 1,542 rows and were byte-identical across those runs.

A separate refraction-coordinate oracle uses quantized linear-light ramps.
It checks four straight-edge normals, signed displacements, two optical widths,
and 1×/2× scaling. All 160 sampled coordinates agree within one encoded channel
level, below the preselected two-level limit. This verifies the coordinate
mapping, not whether its profile matches an undocumented Apple style.

The comparisons exposed and fixed:

- Small-radius and kernel-shape errors from blending Gaussian scale levels.
  Uniform blur now evaluates its requested kernel directly.
- Lost fractional-pixel coverage caused by rasterizing only the geometric
  rectangle. Raster and scene bounds now include the AA fringe while preserving
  output clipping and rejecting empty geometry.
- A spring clock bypass: the retained spring element used wall time instead of
  GPUI's scheduler clock, preventing deterministic frame progression in tests.

## Actual element and motion checks

The adapter tests execute GPUI layout, prepaint, and paint. They verify that
background precedes capture and content follows it, and exercise parent clipping
and opacity, all three outlines, both scales, and opaque/contrast fallbacks.
System preferences are injected into the cached application state for these
checks; the tests do not mutate macOS settings or claim to test delivery of a
real AppKit notification.

An additional full-image check exercises two overlapping tinted materials with
ancestor opacity, in both paint orders at 1×/2×. It changes the background A→B→A
while reusing the renderer and cached paint operations. Actual `Scene::replay`
through nested layers must match fresh rendering byte-for-byte, and each pixel
must agree with independently calculated sequential linear-light composition
within two encoded levels.

Retained GPUI spring tests use scheduler seeds 0, 1, and 42. They check position
continuity and forward momentum during reversal, stopping frame requests after
settling, mid-flight reduced-motion changes, and resuming with no stale velocity.
Common-time positions at 60/120 Hz, including a dropped frame, differ by less
than 0.05 logical pixels. The resumed trajectory agrees with a fresh spring
within 0.01 logical pixels.

## Performance sample

An optimized build rendered 20 measured frames after three warmups. Temporary
instrumentation read public Metal command-buffer GPU start/end timestamps after
completion; it was removed after collection. Times include every pass in that
command buffer, not just the glass fragment shader. Initialization took 21.862 ms
and may benefit from driver caches.

| Device size / sigma | Effects | GPU median / p95 | Submission + GPU + readback median / p95 |
| --- | ---: | ---: | ---: |
| 1024×768 / 6 px | 0 | 0.098 / 0.131 ms | 1.112 / 1.291 ms |
| 1024×768 / 6 px | 1 | 0.688 / 1.079 ms | 2.079 / 2.473 ms |
| 1024×768 / 6 px | 2 | 1.780 / 2.774 ms | 3.297 / 4.337 ms |
| 2048×1536 / 12 px | 0 | 0.228 / 0.942 ms | 4.490 / 5.374 ms |
| 2048×1536 / 12 px | 1 | 3.527 / 4.098 ms | 8.320 / 9.089 ms |
| 2048×1536 / 12 px | 2 | 6.043 / 8.291 ms | 12.777 / 23.440 ms |

This short sample is not a sustained frame-rate guarantee. Two Retina effects
already approach the 8.33 ms budget for 120 Hz before other UI and display work.
Full-window filtering remains the primary optimization target. Cropping or
sharing captures must preserve the current numerical and paint-order checks.

## Reproduction and native comparison

```sh
devenv test
devenv shell -- cargo test -p gpui-cupertino --test numeric_reference --locked -- --nocapture
devenv shell -- cargo bench -p gpui-cupertino --bench backdrop --locked
```

Set `CUPERTINO_REFERENCE_OUTPUT` to an absolute CSV filename with an existing
parent directory to export the measured Gaussian curves from the numerical
check. The benchmark prints synchronous render/readback timing; the isolated
GPU measurements above used the temporary diagnostic instrumentation described
above.

On macOS 26 or later, with an **unlocked, visible desktop session** and screen
recording permission:

```sh
devenv shell -- cargo run -p gpui-cupertino --example native_compare --locked -- work/comparison
```

This creates an owned AppKit window: public `NSGlassEffectView` on the left,
Metal readback using demo calibration parameters on the right. Both receive the
same background, logical bounds, and corner-radius inputs. AppKit does not
promise the same contour or antialiasing as our analytic outlines. The example
captures light/dark, regular/clear, two background phases, and two repeated
snapshots of each case. It checks before and after every capture that the window
is visible, inactive, non-key, and at the original scale. Reduce Transparency
and Increase Contrast must both be disabled; the example checks these settings
without changing them. A failed state check invalidates that capture run.

## Native comparison results

Three independent process runs produced 48 captures at 2× on the system listed
above. All state checks passed. The initial exploratory run had a focus
transition and a background mismatch, so it is excluded from these results.
After fixing the capture state, all 252,232 far-background control pixels in
every capture matched exactly between the two halves.

The Metal half was pixel-identical in all 24 within-run repeat pairs and all 48
cross-run comparisons of matching cases/snapshots. Native halves were identical
in 19/24 and 30/48 comparisons respectively; every remaining channel difference
was at most one encoded level. Worst native mean difference was 0.029712 over the
content region and 0.083116 over the combined shape interiors. Thus the measured
style differences are substantially larger than the observed capture variation.

Each screenshot is 1536×696 pixels, with a 56-pixel title bar and two 768×640
content regions. Registration was checked against the displayed Metal source.
Screenshots contain the display ICC profile; source PNGs are untagged. Metrics
therefore compare left and right within the same screenshot, without treating
raw source-to-screenshot differences as shader errors.

The following ranges cover both background phases, both snapshots, and all three
runs. Values are mean absolute encoded RGB channel differences on a 0–255 scale,
averaged over pixels at least two device pixels inside each analytic shape.
These are diagnostic differences, not perceptual similarity scores or a newly
chosen pass threshold.

| Appearance / style | Rounded rectangle | Capsule | Circle |
| --- | ---: | ---: | ---: |
| Light / regular | 11.69–12.61 | 12.02–12.37 | 5.47–5.73 |
| Light / clear | 10.59–10.84 | 11.94–13.65 | 13.19–13.44 |
| Dark / regular | 6.47–6.63 | 6.53–7.05 | 5.87–5.87 |
| Dark / clear | 10.47–10.57 | 11.06–11.55 | 11.35–11.45 |

Visual inspection confirms different filtering and color response, with an
excessively bright directional highlight in the dark Metal examples. The demo
parameters have not been fitted to Apple's styles. These results **do not pass
native visual-equivalence acceptance**; the mathematical reference checks above
verify our documented primitives, not Apple's undocumented style parameters.

Native evidence is limited to stationary, inactive windows at 2×. It does not
cover active windows, 1× displays, hover/press, animation, or fusion. Three
neighboring shapes also do not isolate Apple's sampling/grouping behavior.
Calibration needs isolated-shape captures and separate fitting/validation
backgrounds before changing style presets; arbitrary tuning against one image
would not establish a reusable match.

Local captures and logs are in `work/comparison-inactive-{1,2,3}` and
`work/native-inactive-{1,2,3}.log`; diagnostic JSON files sit beside those
directories. `work/` is ignored and is not part of the distributed library.

## Isolated calibration probes

A final aggregation of five fixed Clear research candidates retained all 24
borderless captures and 104 declared regions. None passed the one-code RGB
channel limit. The candidate with the most passing regions, first copy-base,
passed 74/104: 12/24 centers and 62/80 edges. Its maximum error was 4.340418
codes, and 6.0299% of the 4,780,032 scored RGB channel samples exceeded one
code. These counts describe research replays, not the production GPUI renderer
or a percentage of implementation completion.

All five candidates failed all 20 regions with the custom orange/blue palette.
The all-raster candidate reduced the maximum to 4.118915 but passed only 44/104
regions and increased average error. At the fixed light witness pair, the
native green values differ by four codes while the baseline predicts identical
RGB. A pointwise correction of that identical input has a worst-error lower
bound of two codes. Upstream spatial or source information remains unresolved;
this result does not identify a unique cause. The aggregation verified 2,172
unique frozen files without rescoring images. The report and denominators are
retained in `internal-docs/research/clear-gap-audit-2026-09-30`.

The comparison example also places a single native shape beside the same input
without glass. This separates color response, filtering, and displacement:

```sh
devenv shell -- cargo run -p gpui-cupertino --example native_compare --locked -- \
  work/probes probe --background step:v --shape roundrect --appearance light --style regular
```

All four probe options are required. Backgrounds are `solid:RRGGBB`, `step:h[:phase]`,
`step:v[:phase]`, `ramp`, or `checker:N[:phase]`. Shapes are `roundrect`, `capsule`, and
`circle`; appearances are `light`/`dark`; styles are `regular`/`clear`. The ramp
encodes x in red, y in green, and constant 128 in blue. Checker cells and phase
use integer logical pixels. `checker:N:phase:RRGGBB:RRGGBB` supplies explicit
colors independent of appearance, for example `checker:32:7:000000:ffffff`.
At phase zero the top-left cell uses the first color. Metadata records the
actual colors, and explicit colors distinguish output names.
Step phase is a signed offset from the panel center,
in half-logical-pixel increments; positive moves right/down. A step must fall
strictly inside the panel and exactly on a device-pixel boundary at the capture
scale. Optional `--size WIDTHxHEIGHT` centers integer dimensions within the
384×320 panel. Rounded rectangles use radius `min(20, short_side / 2)`, capsules
use `short_side / 2`, and circles require equal dimensions. Defaults remain
256×128 for rectangles/capsules and 128×128 for circles. Optional `--offset DX,DY`
translates the glass from that centered position by integer logical pixels;
positive values move right/down, and the background stays fixed. The translated
shape must remain entirely inside the panel. For example,
`--size 240x128 --offset -8,0` places it at `[64, 96, 240, 128]`. Nonzero offsets add
`-offset{dx}x{dy}` after the size in output names. Nondefault dimensions and nonzero
background phases also distinguish names; equivalent default inputs, including
`--offset 0,0`, retain the original names. Metadata records the actual glass bounds.

Optional `--capture-count N` records 2 through 32 frames (default 2), with the
existing 0.3-second delay after each capture completes. Nondefault counts append
`-countN` to the case name. Every frame is listed in metadata and retained.
`TIMING` JSON log lines record `elapsed_since_prepare_ms` from a monotonic clock
at `deactivate`, `before_capture`, and `after_capture`; capture stages include the
zero-based `repeat` index. `after_capture` records the screenshot process's
successful return, before the delay and subsequent native state check.

Each case writes the exact input PNG, its captures, and JSON
containing geometry, scale, and the measured content offset. Probe mode uses a
borderless window so the source image is not masked by rounded window corners;
the comparison demo keeps its titled window. State checks apply
to every capture. The example runs AppKit's application event loop; its delegate
schedules preparation, deactivation, and each capture on a separate turn.
The first capture waits at least five seconds after deactivation for rendering to
settle; inactive/non-key state has a ten-second deadline. Checks before and after
each screenshot are never bypassed when the state is wrong. This fixed delay
does not select frames or disable animations.

The first isolation matrix contains 68 cases and 136 successful captures:
five grays, three primaries, two appearance-specific checker colors, two step
directions, a coordinate ramp, and two checker phases for each style/appearance;
capsule and circle ramps supplement the rounded rectangle. Phase 7 and the
checker colors must remain independent validation inputs when parameters are
identified from pure colors and steps.

The calibration target is matched-state output within the native repeatability
envelope (currently at most one encoded level per channel), including edges,
across independent inputs. The current renderer has not met that target. A
smaller average error on a training checkerboard is not sufficient acceptance.

Local read-only research also inspected actual objects attached to an owned
native view, rather than assuming that an on-disk recipe was selected. For an
inactive 256×128 logical-pixel rectangle with radius 20, the observed backdrop
capture scales were 0.25 for regular and 0.5 for clear. Their filter blur-radius
inputs were approximately 3.428571 and 10 respectively. These values are **not
Gaussian sigma**, and do not establish the final GPU buffer or sampler mapping.

Six geometry configurations showed that changing radius 20→64 at a fixed
256×128 size did not change those inputs. Changing the short dimension did:

| Short dimension | Regular blur-radius input | Clear blur-radius input |
| --- | ---: | ---: |
| 64 | 2.285714 | 6.111111 |
| 72 | 2.428571 | 6.666667 |
| 96 | 2.857143 | 8.333333 |
| 128 | 3.428571 | 10 |

A subsequent 32-case sweep included the transposed pair 128×96 and 96×128
with radius 20. Both produced regular blur-radius 2.857143 and clear 8.333333
in both appearances. This rejects a height-only rule and supports minimum-
dimension dependence in the sampled range; it does not establish a universal
interpolation rule. The inactive
key/fill highlight layer had zero opacity and zero color alpha, whereas the
demo used a nonzero directional highlight. The observed refraction-opacity
input of zero does not establish that all inner refraction is disabled: the
archived shader has a separate branch flag that has not been traced to final
uniforms.

Color-managed measurements found zero difference in every far-background
control region, and at most one encoded level of repeat noise. A proposed
per-channel formula using the observed white/black/fill inputs reproduced all
five grays within one level, but failed every colored validation input. Worst
channel errors were 43.125 (light regular), 85.55 (dark regular), 18.40 (light
clear), and 48.45 (dark clear). Applying that same formula in linear RGB was
worse. For example, light regular over pure green predicted [172,247,172] but
measured [129,255,139]. No preset was accepted from this falsified model.

Further measurements separated the transform from the output color conversion.
A fixed luminance/chroma formula, using the observed filter inputs and Rec.709
weights `(0.2126, 0.7152, 0.0722)`, predicted all 64 solid-color cases and both
captures within one encoded level. This includes 24 additional midtone cases;
no coefficients were fitted:

```text
luma = dot(weights, input)
face = input + ((white - black - 1) * luma + black)
output = clamp((1 - fill_alpha) * face + fill_alpha * fill_color, 0, 1)
```

The scalar adjustment in `face` is added to all three channels, preserving
color differences before fill. The inputs and outputs here are the original
captured **display-profile RGB code values**, with each capture's no-glass
control as input. Applying the earlier scalar contrast model after conversion
to sRGB tests a different hypothesis. Maximum channel errors for light regular,
dark regular, light clear, and dark clear were 0.565, 0.538408, 0.475, and
0.533513 levels respectively. Clamping `face` before fill was falsified by the
dark regular blue sample (12.25 levels).

This establishes a color-response model for the measured inactive state and
display profile. It does not establish the compositor's texture format, a
portable sRGB implementation, blur behavior, or a complete native preset.
AppKit independently decoded the untagged input PNGs as sRGB, matching explicit
sRGB `CGColor` backgrounds; the screen captures use the display's ICC profile.

Using independently captured black/white outputs as threshold endpoints,
the step probes gave the following 10–90% output linear-luminance widths. Both
axes and both repeats agreed at the displayed precision. These widths include
the native color response and must not be labeled Gaussian sigma.

| Appearance / style | Transition width, logical pixels |
| --- | ---: |
| Light / regular | 21.228 |
| Dark / regular | 18.912 |
| Light / clear | 47.858 |
| Dark / clear | 44.727 |

Repeating the thresholds in encoded RGB, which removes the measured gray
transfer from the width statistic, gave 10–90% widths of 21.125 / 21.250 logical
pixels for light/dark regular and 48.200 / 47.850 for light/dark clear. Horizontal
and vertical widths and repeats agreed. The ratio of 10–90% to 25–75% width was
2.055–2.099 for regular and 1.740–1.763 for clear, compared with approximately
1.900 for a Gaussian step response. These measurements do not establish a
Gaussian kernel; an equivalent sigma from one width alone would miss the
observed transition shape.

With the analysis environment available, a bounded least-squares comparison
trained one Gaussian CDF and one generalized-normal CDF per material on only
light/vertical/capture0. The window was fixed at ±40 logical pixels from the
step, at least 24 pixels from the nearest contour. Color coefficients remained
fixed. All other axis/appearance/repeat combinations were held out, with no
parameter changes after validation:

| Material | Response model | Holdout maximum code error | Holdout MAE |
| --- | --- | ---: | ---: |
| Regular | Gaussian | 1.735 | 0.484 |
| Regular | Generalized normal | 0.878 | 0.302 |
| Clear | Gaussian | 2.553 | 0.805 |
| Clear | Generalized normal | 2.489 | 0.807 |

Regular's generalized-normal response (`beta=1.454379`, `scale=9.697687`
logical pixels) stayed within one raw Display RGB code value on all seven
holdout profiles. Clear did not improve meaningfully; its residual structure
persisted while the sampled step strips repeated exactly. This evaluates the
fitted baselines, not the minimax feasibility of every Gaussian parameter set.
It is a one-dimensional inactive step-response model, not a recovered native
blur implementation or validation on other shapes, settings, or full images.

Clear's remaining residual led to a prospective phase/size experiment. All
models were frozen before new pixels were inspected: six vertical phases
(`0`, `0.5`, `1`, `1.5`, `2`, `3` logical pixels) at 256×128, horizontal phase 1
in light and dark appearance, and vertical phases 0/1 at 256×96. Each condition
was captured twice. The new phase-zero capture checks run consistency; it is
not a previously unseen condition. The complete batch used the corrected
AppKit application lifecycle described above. Forty window-state checks passed;
input/control step boundaries, capture dimensions, and ICC profiles agreed.
All ten repeated center strips were pixel-identical.

The fixed recursive filter candidate uses the axis projection of a 13-point
downsample kernel. Its five weights are
`[0.05633544921875, 0.244384765625, 0.3985595703125, 0.244384765625, 0.05633544921875]`
at offsets `[-4, -2, 0, 2, 4]` in the preceding level's texel coordinates.
Each destination sample is centered at source index `2*k + 0.5`, with linear
interpolation, implementing 2× downsampling. This candidate has **no fitted
parameters**. For one interior level-four output node, the recovered source-domain
kernel has unit mass, zero center offset, and variance 340.6640625 logical pixels
squared, matching an independent moment calculation.

The predeclared size rule uses effective pitch `1.6 * radius`: pitch 16 for
the measured radius input 10, and pitches 8/16 with upper weight 0.7369655942
for input 25/3 at height 96. The internal surface scale and actual mip index
remain unidentified; this is a tested logical-coordinate model. A single-grid
Gaussian and a Gaussian sampled at these mip levels were also frozen on the
old light/vertical/phase-zero capture. The baseline and single-grid sigma scale
with the radius input; the mip Gaussian's sigma scales with each level's pitch.
Color coefficients remained fixed:

| Response model | Maximum error, 256×128 | Maximum error, 256×96 | Profiles within one code |
| --- | ---: | ---: | ---: |
| Continuous Gaussian baseline | 2.532 | 3.353 | 0 / 20 |
| Single-grid Gaussian | 1.325 | 3.038 | 4 / 20 |
| Mip-level Gaussian | 1.325 | 1.139 | 6 / 20 |
| Fixed recursive filter | **0.946** | **0.870** | **20 / 20** |

Errors are maximum absolute single-channel Display RGB code differences;
raw-pixel maxima equal the mean-strip maxima. No model or threshold changed
after validation. The result supports this one-dimensional inactive Clear
response across the measured phases, axes, appearances, and two sizes. It does
not establish actual compute dispatch, a portable working color space,
two-dimensional separability, contour optics, or active-state equivalence. The
library's Gaussian contract is unchanged. The complete experiment, including
failed attempts, models, raw captures, plots, and source snapshots, is archived
under `internal-docs/research/clear-1d-2026-09-29`. All 621 original files passed
SHA-256 verification; replay in a temporary copy reproduced the complete result
JSON without reading the repository's original `work/` directory.

A subsequent frozen two-dimensional experiment used seven checker conditions
and both captures, comparing the full nonseparable 13-point kernel with the
outer product of its one-dimensional projection. A separate edge experiment
used light/dark ramps and two gray checker frequencies, with four straight-edge
bands per capture. The full-panel input hypothesis failed: maximum single-channel
errors were 13.248 levels in the fixed depth-at-least-24 interior and 27.548 in
the fixed 2–20 logical-pixel edge bands. Matching one-dimensional steps had not
identified the background boundary behavior.

Post-observation diagnosis found that replicating the base image's first/last
inside rows and columns **before generating the mip levels** explains most of
these residuals. This ordering has independent static control-flow evidence;
clamping each already-filtered mip level gives a different result. The revised
candidate keeps the same kernel, color response, radius, optical profile
`1 - sqrt(t * (2 - t))`, amount `-60`, width `20`, ROIs, and one-level threshold,
with no fitted parameters. Its maximum interior error is 1.054 levels, versus
11.203 for the separable counterexample with the same boundary rule. Maximum
edge error is 1.063 levels; 46 of 48 edge bands are within one level, while the
zero-refraction counterexample fails every band and reaches 55.509 levels.

These revised results are **development evidence, not prospective acceptance**.
The light colored checker still exceeds one level at some interior samples;
dark ramp bottom-edge samples also exceed it. No failed sample was removed and
no threshold was relaxed. Repeated captures agree in every measured center,
left, right, and top ROI, but ten of eleven conditions have up to one level of
bottom-edge variation. The independent zero-repeat-noise check therefore fails
and remains separate from model scores; neither averaging nor subtracting
noise is used to manufacture a pass.

Raw captures, input and state checks, all failed boundary candidates, static
evidence, scripts, and residual plots are retained under
`internal-docs/research/clear-2d-edge-2026-09-29`,
`internal-docs/research/clear-2d-2026-09-29`, and
`internal-docs/research/clear-edge-2026-09-29`. These directories remain locally
ignored research, and none of these diagnostic candidates changes the product
renderer or establishes native equivalence.

A frozen follow-up tested ten new phase/size conditions after an unlocked
40-configuration host sweep confirmed the radius inputs. The glass-local grid
candidate failed: maximum center/edge errors were 4.980 / 4.550 levels. A single
post-observation diagnostic retained the background coordinate phase while
keeping base-edge replication and every other parameter fixed. Errors fell to
1.430 / 1.342, but colored checker and ramp failures remained. This diagnostic
does not replace the original failed prospective result.

A second experiment froze that background-grid candidate before collecting six
new gray-checker conditions. All shapes remained 240×128; integer translations
isolated grid phase from size. Both captures were scored independently, with
unchanged depth ≥24 center ROIs and four depth 2–20 straight-edge bands:

| Frozen candidate | Center maximum error | Edge maximum error | Center / edge regions within one level |
| --- | ---: | ---: | ---: |
| Background-grid full 13-point | **0.955** | **0.952** | **12/12 / 48/48** |
| Glass-local full 13-point | 4.814 | 4.852 | 6/12 / 24/48 |
| Background-grid separable projection | 9.261 | 8.506 | 0/12 / 0/48 |
| Background-grid zero refraction | 0.955 | 73.710 | 12/12 / 0/48 |

Aligned positions are consistency controls where the two grid candidates
coincide. Discriminating translations support retaining source-coordinate phase
instead of restarting at each glass origin. The center cannot distinguish
refraction because it is outside the optical band. Neither the precise native
coordinate frame nor the actual GPU dispatch, texture format, or working color
space is identified by this fixed-window experiment.

All input, ICC, geometry, and native/session checks passed. However, six bottom
bands contained 90 repeat-differing pixels, each differing by at most one RGB
level. The frozen zero-noise check and **overall acceptance remain failed**;
model-error acceptance alone passed. Earlier color/ramp failures remain open.
Plans, immutable model hashes, raw images, counterexamples, plots, and reports
are preserved in `internal-docs/research/clear-2d-edge-prospective-2026-09-29`
and `internal-docs/research/clear-grid-translation-2026-09-29`. No product
renderer or Gaussian-contract change follows from these research results.

The repeat discrepancy was subsequently investigated with six fixed cases and
16 frames per case. The late comparison interval was declared before capture:
frames 8–15 against frame 8, all at least three seconds after deactivation.
All 96 images and their timing/state logs were retained. Early frames differed
by as much as 20 encoded levels; every predetermined late comparison was
byte-identical across the entire native panel, including corners and regions
outside the sampled edge bands. Independent analysis reproduced the result.
This identifies a capture-timing problem, without identifying which AppKit or
compositor transition caused it. The probe now waits a fixed five seconds after
deactivation before its first capture; it does not select frames or disable
native animations. The old failed captures remain unchanged.

Numerical diagnostics on all 27 earlier two-dimensional cases retained the
same 54 center regions, 200 edge bands, and continuous one-level limit.
Explicit half arithmetic, high-precision ColorSync input, alternative color
conversion order, and intermediate UNORM8 storage did not produce a model that
passed every region. Some reduced the average error while increasing the worst
error or the number of failed regions. Those results do not justify a renderer
change or a weaker acceptance limit. Timing evidence is in
`internal-docs/research/clear-settling-2026-09-29`; numerical diagnostics are in
the adjacent `clear-precision`, `clear-color-domain`, `clear-color-stage`, and
`clear-storage` directories dated 2026-09-29.

Independent public Metal programs then tested actual GPU mip computation and
final float sampling/refraction with statically derived half face matrices.
The matrix derivation reproduced six system CPU matrix compositions
byte-for-byte on owned inputs. Synthetic texel, LOD, clamp, displacement, and
repeat checks passed. Neither RGBA16Float nor RGBA8Unorm produced a uniformly
passing native model: the complete GPU float-final candidates reached maximum
center/edge errors of 1.367/1.277 and 1.618/1.344, respectively. All 254 earlier
baseline regions were reproduced. These conditional implementations do not
identify the native frame's shader variant, working color domain, actual
texture format, or final output conversion. Sources, raw GPU readbacks, and
failed comparisons are retained in `clear-metal-compute-2026-09-29` and
`clear-metal-final-2026-09-29` under the same research directory.

A new 12-case batch froze 592 inputs before acquisition, including all 13 model
columns and the five-second capture protocol. All 24 original images passed
source, ICC, timing, 48 native-state and 24 unlocked-session checks. Both the
declared ROIs and the entire native panel were byte-identical between each
pair, independently confirmed from raw RGBA. Repeat stability therefore passed
on this unseen batch. Optical acceptance still failed for every model. The
primary's eight gray center regions and 32 gray edge bands all passed, but a
new orange/blue palette exposed maximum errors of 4.404 in the center and
3.993 at an edge. Default-color and ramp failures also remain. The deep-center
counterexample cannot be attributed solely to edge refraction or repeat noise;
the color response and filtering domain still require isolation. The frozen
primary, every failed column, raw captures and GPU outputs are retained in
`internal-docs/research/clear-stable-prospective-2026-09-29`.

An exact-arithmetic check further limits post-color calibration. Two center
pixels in the validated borderless baseline have identical full13 mixture
fractions but differ by four native RGB codes. Any position-independent function
of that same fraction must predict one color; at least one pixel then has an
error of two codes or more. Both appearances and repeats contain this
counterexample. The current spatial model therefore cannot meet the one-code
gate by changing only a final color curve. This does not identify the missing
native operation. Exact fractions, integer computations and source hashes are
retained in `internal-docs/research/clear-chromatic-invariants-2026-09-30`.

A prospective six-case solid-color isolation used the orange/blue endpoints
and their exact source midpoint, with both appearances and two raw captures
each. All state, source, ICC, five-second timing and complete native/control
repeat gates passed. Every center was spatially uniform. The original Display
face formula and the statically derived half coefficients both passed every
center pixel, with maxima 0.520460 and 0.495239 respectively. Thus these solid
colors do not reproduce the checker failure. A separate diagnostic allowed an
arbitrary common RGB mixing fraction at every checker center pixel: even the
best point on the measured pure-native endpoint segment left maximum errors
of 3.846154 (light) and 3.932584 (dark). Changing only nonnegative normalized
filter weights in captured Display codes cannot explain this counterexample;
another working domain followed by a nonlinear output conversion remains
possible. This diagnostic does not select a domain or pass optical acceptance.
Raw captures, independent review and scripts are retained in
`internal-docs/research/clear-solid-color-2026-09-29`.

A single conditional encoded Display P3 filtering domain was also falsified.
Source sRGB → P3 → unchanged full13/refraction → captured Display ICC → face
produced center/edge maxima of 8.716038/7.586206; only 8/24 centers and 38/80
edge regions passed. All 104 baseline regions were reproduced, and the prior
13 columns were retained unchanged. Public window/screen observations matched
the capture ICC, but do not identify an intermediate blend or blur space.
The P3 result, raw Float32 conversions and static evidence are sealed in
`internal-docs/research/clear-p3-domain-2026-09-29`; public observations and the
separately preserved observer startup failure are in `clear-color-space-2026-09-29`.

The subsequent face-opacity intervention has not passed its protocol gates.
Two attempts stopped before capture when an auxiliary filter identity changed.
A separate revision with one deactivation per appearance completed all 12
A0/B/A1 captures without a native assertion failure, but independent validation
rejected their 1536×704 window images: the frozen reference requires 1536×696.
The Swift and Rust executables were linked against SDK 26.5 and SDK 14.4,
respectively. Building the Swift probe against the real installed SDK 14.4
requires a compatible Swift toolchain; the installed Swift 6.3.3 rejected that
SDK's Swift 5.10 interfaces. No crop or acceptance threshold was changed, and
these captures were not optically scored. All three attempts and their failures
are preserved in `internal-docs/research/clear-face-bypass*-2026-09-29`.

Using the installed Xcode toolchain, an isolated Rust probe was rebuilt against
SDK 26.5/minimum macOS 26.0 to match the Swift probe. A new baseline froze 1,065
inputs before capture and verified all six fresh host parameter conditions.
All 24 captures passed source, ICC, timing, state and complete-panel zero-repeat
checks at the new 1536×704 geometry. Every center ROI was byte-identical to the
old SDK cohort; all 13 models retained the same center/edge maximum errors and
pass counts. The primary still fails at 4.403819/3.993046. This removes the SDK
mismatch from the next intervention's prerequisites; it does not repair the
colored-background discrepancy. Build provenance, source snapshots, raw images,
all model outputs and independent reviews are retained in
`internal-docs/research/clear-xcode-baseline-2026-09-29`.

The same-SDK face intervention completed 12 captures but still failed its hard
opacity gate: the titled window's bottom corners contain 584 nonopaque content
pixels, including six inside the original control ROI. Separate raw comparisons
also found up to 15 codes of A0/A1 difference around the glass outline and up to
13 codes between the dark B repeats; these pixels are opaque and distinct from
the window corners. The original checker remains unchanged and no optical score
was run. This failure is retained in `clear-face-bypass-current-2026-09-29`.
The borderless probe removes window decorations from acquisition while retaining
the full-content opacity and zero-repeat requirements. Its first batch stopped
at the final system screenshot and is retained as incomplete. A complete fresh
12-case batch then passed all 24 full-content alpha checks and every native and
control repeat gate. The incomplete batch is not merged into this new baseline,
retained in `internal-docs/research/clear-borderless-retry-2026-09-29`.

Static allocation tracing found conditional lossy-texture requests in both
backdrop capture and blur mip construction. The captured frames' actual format
and compression mode remain unbound. A public render-pass comparison compiled,
but its synthetic upload check stopped before native-image scoring: lossless
render and the old compute upload matched exactly, while both differed from
the check's NumPy nearest-even conversion by at most 0.124512 codes. Metal's
default texture-write mode is hardware-native; the available feature table
does not specify that mode for this GPU. The failed assertion and raw outputs
remain unchanged in `internal-docs/research/clear-lossy-texture-2026-09-30`.

After approval, a separate experiment compared lossless upload with the frozen
compute upload. Both synthetic fixtures passed exact upload and repeat checks;
all five lossless levels also matched the compute result. The frozen comparison
then evaluated both compression modes on all 24 original captures:

| RGBA16Float render path | Center maximum / passed regions | Edge maximum / passed regions |
| --- | ---: | ---: |
| Lossless | 4.347033 / 12 of 24 | 3.937689 / 60 of 80 |
| Lossy | 4.392967 / 12 of 24 | 3.941699 / 60 of 80 |

Both columns fail the unchanged one-code threshold. Lossless results exactly
reproduce the earlier GPU half-precision column across all 104 regions. Lossy
compression predicts less than 0.071 code of separation at the fixed equal-input
witnesses, whose native green channels differ by four codes. This particular
compression change does not explain the residual. The experiment uses five
independent textures; native multi-mip allocation, format and compression remain
unbound. Sources, raw texels, scores and an independent hash review are sealed in
`internal-docs/research/clear-lossy-corrected-2026-09-30`.

Further system tracing distinguishes requested bounds from allocated texture
dimensions and identifies conditional mixed-format capture and mip storage.
The observed Clear filter inputs also satisfy the radius condition for skipping
the base mip, assuming one glass group and unit working scale. That branch
changes the first sampling pass and halves the later sampling scale. Simply
removing a pyramid level would not reproduce the operation. These are conditional
implementation findings, not measurements of the captured frame's selected path.
The ordinary MetalContext also inherits a null `read_surface` implementation,
so the inspected `CA_DUMP_BACKDROPS` path cannot export its texture. The print
option reports logical bounds and scale, not format or allocated dimensions.
Read-only evidence is retained in
`internal-docs/research/clear-native-storage-2026-09-30`.

A subsequent binary audit traced capture bit-depth reduction to
`CABackdropLayer.reducesCaptureBitDepth` and the force-half mask to scene EDR
summary flags. The mask can include other contexts, so owned SDR layer getters
cannot identify the compositor's capture format. Metal's lossy setter selects
footprint 3; relevant format capabilities depend on Apple8 support. Private
format 550 has the implementation name `MTLPixelFormatRGB10A8_2P_XR10`, distinct
from public format 552. Neither capability nor naming establishes frame storage.

For ordinary source-surface capture at scale 0.5, the recovered pixel-center
formula supports the existing 2×2 average when the source grid and values match.
The implementation does not justify a free sampling phase. Glass edge extent
uses the largest absolute amount without testing opacity, giving 75 for the
observed Clear inputs. That extent affects conditional auxiliary shapes; it
does not justify expanding the source crop by 75 pixels. No replacement source
model or production parameter was selected, and the optical failure is unchanged.
The research is retained in
`internal-docs/research/clear-capture-binding-2026-09-30`. Manual binary review
and workspace checks passed. The SDK declaration parser now recognizes
availability annotations. The repaired check passed all four symbol anchors,
18 jump targets and six name-address checks. Three incorrect SDK enum values
were rejected by the original equality assertions. The original failed parser
remains sealed; the repair and results are retained in
`internal-docs/research/clear-metal-parser-repair-2026-09-30`. These checks verify
the evidence mapping, not native optical equivalence.

Further tracing recovered the dynamic source-surface stack and the separate
detached-texture path. Ordinary surface binding does not create a ColorProgram;
ordinary unbinding clears that image slot's Program. Direct Render::Texture
binding has a conditional source-color-space lookup. The first surface's entry
state and the captured frames' source paths remain unbound. A generic shader
ColorProgram branch therefore does not justify adding an unconditional color
conversion to the replay. The static evidence and verified unbind vtable slot
are retained in `internal-docs/research/clear-capture-path-2026-09-30`.

A frozen read-only observation then measured the relevant owned layer getters
on a new borderless Clear window. All four observations passed provenance,
session, timing and repeated-tree stability checks against 103 frozen inputs.
Both appearances reported backdrop scale 0.5, reducesCaptureBitDepth false,
captureOnly false, and isInverseMeshed false. All 22 owned model layers had nil
meshTransform, standard preferredDynamicRange, legacy EDR opt-in false, and
contentsHeadroom zero. These values constrain the traced client-side producers;
they do not measure scene-wide EDR, shared-group state or the compositor's source
texture. In particular, RGBA8 contentsFormat is a storage hint, not a capture
format measurement. No optical candidate or production parameter changed.
Sources, raw observations and checks are retained in
`internal-docs/research/clear-capture-properties-2026-09-30`.

A fixed-node upstream diagnosis then checked the complete local dependency
graphs of the existing equal-input witnesses. The transform
`T(x,y)=(y+64,x-64)` preserves their source values and corresponding weights
through levels 0–4, with zero unequal nodes at every level. The rectangular
clamp changes some coordinates but no source colors in these dependencies.
The equality therefore predates final reconstruction. A spatially uniform
pointwise color operation inserted into this exact, exchange-equivariant
pipeline cannot separate the witnesses. Actual GPU rounding, source fields,
coverage and coordinates remain separate conditions.

Static tracing also recovered ColorProgram construction, six serial opcode
slots and two 32,768-entry half gamma tables. No active frame's program was
bound. Ordinary capture can sample individual surfaces before source-over
composition. With matched linear sampling and identity source conversion,
this differs from sampling an already-composited screenshot when source alpha
and background have nonzero local covariance. One opaque source
eliminates that difference, and multiple sources alone do not break the witness
symmetry. The required native source condition has not been established.
Member rectangles, group Shape, selected source bounds and texture allocation
have distinct producers. These findings constrain the next source/coordinate
diagnosis without selecting a new color, phase or crop parameter. Scripts,
exact results, table decoders and independent reviews are sealed in
`internal-docs/research/clear-upstream-2026-10-01`.

A subsequent frozen owned-window observation passed all session, timing,
repeated-tree and provenance checks against 162 inputs. All four observations
reported nil groupName, owningContext namespace, allowsInPlaceFiltering false,
zero backdropRect and zero marginWidth. These values were also present in the
presentation tree. The traced submission path therefore supplies no explicit
group name or margin expansion; the member-rectangle producer falls back to
the layer bounds when participating. This constrains the new observation, not
the historical frames or the server's complete group, prepared Shape and source
stack. No new optical score or production change followed. The observer,
raw records and independent review are retained in
`internal-docs/research/clear-group-properties-2026-10-01`.

A further trace resolved the final glass shader's `_lph` selection and the
CPU uniform conversion into half precision. A fixed replay retained the
first-copy-base textures and added half UV, refraction, LOD, sample cleanup,
alpha division and face-matrix operations. Synthetic checks passed after an
explicitly approved one-line oracle correction: two sub-threshold source
values must become zero before exact comparison. The shader and exact
assertions remained unchanged; the original failure was sealed separately.

The complete replay passed only 60/104 regions: 8/24 centers and 52/80 edges.
Maximum errors were 4.274902 and 3.978516 codes respectively. All 20 custom
orange/blue regions still failed. Compared with the previous first-copy-base
candidate, 14 passing regions became failures and no failed region passed.
An independent CPU review verified 2,216 frozen inputs, all 24 mappings and
all 104 metrics from the saved GPU outputs, without rounding predictions.

Both fixed witness pairs still had equal predicted green values despite a
four-code native difference. Other predicted channels were not all equal,
so this result does not exclude every cross-channel mapping. It rejects
this fixed final-stage replay as a sufficient correction. The actual source,
capture transform, SDF and output attachment remain unbound. No production
parameter changed. Evidence and the independent review are retained in
`internal-docs/research/clear-lph-final-corrected-2026-10-01`.

A fixed window-origin experiment then stopped before its first moved capture.
The two light A0 captures were byte-identical and exactly reproduced the old
baseline. Static analysis located the trap in the immediate CGWindow bounds
comparison. A separate copy added only raw registration logging and retained
every assertion. After moving right by eight points, AppKit reported origin
`(108,200)` while CGWindow still reported the original x coordinate `100`.
The same window, display and scale were retained. The probe stopped before
the fixed five-second settling interval; no later synchronization was observed.
Neither failed run supplies a window-origin optical comparison. The raw data,
crash reports and independent checks are retained in
`internal-docs/research/clear-window-origin-2026-10-01` and
`internal-docs/research/clear-window-origin-trace-2026-10-01`.
The logging copy also failed its exact old-baseline gate: the bottom ROI
changed by one code at 48 of 2,304 pixels, while the center and other three
edge ROIs matched. Its two new captures repeated exactly. The full native
panel changed by up to 15 codes outside the scored regions. These differences
remain failures; the identical witnesses do not waive the baseline gate.

An approved checkpoint correction completed all 28 window-origin captures.
Only three early checkpoints observe CG position without requiring equality;
window identity, size, AppKit registration and every settled or capture-boundary
position check remain exact. All 186 required CG position checks passed.
Of 42 early observations, 26 still showed the preceding position. These fixed
observations do not measure the actual synchronization latency.

The pixel quality and interpretation gates still failed. Dark X8 repeats
differed at 1,250 native-panel pixels, with a maximum of 13 codes; 12/24 dark
restoration comparisons also differed. The exact old-baseline gate failed in
28/32 comparisons, with one-code changes in the bottom ROI. All 96 moved
center comparisons and all controls were unchanged. Both fixed witness pairs
retained their earlier values. No forced redraw occurred, so zero differences
do not exclude screen-dependent processing after a redraw.

Spatial diagnostics retained every changed pixel in three declared comparisons.
Dark repeat and restoration changes spanned signed distances -18.25 to +0.25
logical points from the glass boundary. Light A0 versus the old baseline
spanned -18.75 to +0.25, with 1,111 changed pixels and a maximum of 15 codes.
All three centers were unchanged. This location does not identify a cause or
waive a failed gate. An independent CPU review verified 2,037 frozen inputs,
all 28 PNGs and every reported comparison. The complete results are sealed in
`internal-docs/research/clear-window-origin-settled-2026-10-01`.
No production parameter or optical acceptance score changed.

A further static and CPU audit tested compact capture allocation with half UV.
It reproduced all 104 archived first-copy and LPH metrics. With fixed mip
contents, the compact layout changed custom-checker center green values by
at most 0.073656 codes in light and 0.077744 in dark. This conditional
sensitivity cannot explain the four-code witness separation. It does not
establish the actual native texture contents, allocation, or resource binding.

The recovered collector excludes null sources from its valid surface list.
The traced half-pixel constants belong to border copies, not interior capture
UVs. Neither path supplies an arbitrary phase correction. A separate trace
also followed `gradientOvalization` from its getter through the render subclass
and uniform packing into the shader. The value mixes shape and radial normals;
the traced branch retains its original distance. This direction adjustment
cannot change a sample with zero displacement magnitude. These findings and
the complete CPU sensitivity results are sealed in
`internal-docs/research/clear-source-geometry-2026-10-01`.

A declared inner-refraction intervention froze 2,259 reviewed inputs after
strict compilation, an owned-filter API check, and 45 checker rejection cases.
The intervention would change only inner amount from -60 to 0 and restore it.
Static analysis confirmed that this also changes the complex-refraction branch;
it is not a displacement-only test.

The first A0 screenshot failed before any intervention or PNG output.
`screencapture` returned 1 with "could not create image from window"; the
probe stopped at its original capture-success assertion. The independent
checker retained the failed process gate. Before and after sessions were
unlocked, but a later public CoreGraphics query found both online displays
asleep and inactive. A separate screen-capture permission query returned true.
These later observations do not establish the screenshot's exact display state.
New captures require awake displays and a new experiment directory. No A/B
comparison or new optical score exists. Frozen inputs, the crash report,
diagnostics and independent review are retained in
`internal-docs/research/clear-inner-zero-2026-10-01`.

The omitted-base first-pass investigation recovered two specific operations:
explicit half-precision 2×2 averaging before the compute kernel, and a raster
kernel with source-texel offsets 1.960085 / 3.920676. Static shader-name tables
select the half-precision raster variant. Independent public MSL compilation
also reproduces its sampler constant. A new frozen experiment replaced only
the first pass, preserving the source proxy, later AGX2 passes and final optics:

| First pass | Center maximum / passed regions | Edge maximum / passed regions |
| --- | ---: | ---: |
| Explicit half average and full13 | 4.340418 / 12 of 24 | 3.929278 / 62 of 80 |
| Noninteger raster sampling | 4.339235 / 14 of 24 | 3.927384 / 58 of 80 |

Both candidates still fail. The fixed witness pairs retain a native green
difference of four codes, while these predictions differ by at most 0.005125
green code. Synthetic formula, impulse-support, alpha and exact-repeat checks
passed before the image matrix; all 104 baseline regions were reproduced.
The first-pass formulas therefore supply reproducible corrections to this
conditional model, but do not explain the native mismatch. The separate
baseline, source texture and native branch limitations remain. Evidence is in
`internal-docs/research/clear-skip-base-2026-09-30`.

A separate frozen candidate also reproduced the superclass path's raster
operation at all four generated levels. Its first output matched the preceding
raster candidate exactly; the later three levels used the statically bound
noninteger kernel and transparent-zero border. Independent RGBA checks covered
that boundary before scoring. Every final ROI and witness query then passed a
strict check that all positive-weight source texels had alpha exactly one.
No boundary pixels were normalized or removed from the acceptance regions.

The all-raster candidate still failed: center maximum 4.118915 with 4/24 regions
passing, and edge maximum 4.096423 with 40/80 passing. Center/edge MAE increased
to 0.515770/0.521585 from 0.379754/0.399241 for the preceding raster-first candidate.
The lower center maximum therefore did not represent a consistent improvement.
All 24 original captures and 104 baseline regions remained in the comparison.
The fixed source proxy, native format and runtime branch remain unresolved;
these experiments do not establish a native style preset. Sources, raw RGBA,
scores and the independent review are retained in
`internal-docs/research/clear-raster-chain-2026-09-30`.

The borderless face intervention passed its opacity, source, state and parameter
hard gates, permitting the predeclared per-image diagnostics. Its quality and
restoration gates still failed: both B repeat pairs change around the glass
outline, and A1 does not exactly recover A0. Centers remain unchanged between
repeats, but the original bottom edge ROI contains one-code differences. All A0
native panels exactly reproduce the Rust baseline. The no-face model reaches
5.318159/5.333851 center/edge error; applying the fixed face formula to B reaches
1.176658/1.099652 against A. Both numerical gates remain failed. A separate
analytic diagnostic places a B color 4.476190 codes outside any common mixture
of the two captured control endpoints. Thus changing only Display-domain mixing
weights cannot explain that observed color. The intervention remains conditional
because its complete restoration gate failed; no candidate was promoted. Raw
images, independent numerical/spatial reviews and all failures are retained in
`internal-docs/research/clear-face-bypass-borderless-2026-09-29`.

Updating `filters.glassBackground.inputFaceOpacity` through the owning layer's
key path did not fix restoration. Eleven of twelve RGBA captures exactly
reproduced the explicit array-replacement run; the remaining light B frame
matched that run's second B frame. Every A0/A1 image was unchanged, and both
numerical gates retained the same maximum errors. Switching between these
mutation routes did not resolve the observed restoration or scoring failures.
The separate protocol and raw captures are retained in
`internal-docs/research/clear-face-bypass-owner-kvc-2026-09-29`.

A separate 1× offscreen feasibility probe stopped before creating Metal or
CARenderer resources: public layout of a fresh, never-displayed NSView/glass tree
did not provide the required layers. No raw frames or optical scores were
produced. This does not establish whether CARenderer can render glass from an
otherwise valid tree. The fixed single-attempt protocol and failure are retained
in `internal-docs/research/clear-carenderer-2026-09-29`.

A separate frozen attempt added only explicit layer backing to the glass view.
The layer-existence check passed, but the tree contained no `glassBackground`
filter and both CAContexts were nil. The original generation gate stopped the
probe before Metal or CARenderer creation. No frames or optical scores exist;
explicit backing alone did not make this construction renderable. The source,
observations and failure are sealed in
`internal-docs/research/clear-carenderer-backed-2026-09-30`.

A post-hoc diagnostic separated the source color path from spatial weights:
16,385 and 65,537 fixed sRGB endpoint mixtures were converted with the existing
ColorSync helper into the capture ICC. Among 2,109 distinct B colors, only six
had a sampled point within one code, all in the bottom band; no center pixel had
such a witness. The fine grid's worst nearest sampled distance was 5.558503.
This argues against that fixed path as a sufficient explanation, but sampled
minima are upper bounds on continuous minima, not proofs of unreachability.
All earlier failures remain unchanged. Raw Float32 curves and per-color/ROI
results are retained in `internal-docs/research/clear-source-mixture-curve-2026-09-29`.

A read-only owned-window observation also found that the first backdrop-aware
vibrant layer has an opaque white Extended sRGB background, despite nil contents.
Its zero host matrix alpha row therefore cannot by itself establish that the
branch contributes nothing. All 17 model layers and the owned image/content
views shared a CAContext whose color-space ICC matched the captured display.
These are client-side observations, not proof of the GPU's intermediate working
space. Sources, raw model/presentation trees and state checks are retained in
`internal-docs/research/clear-vibrant-inputs-2026-09-29`.

A later opacity-only intervention disabled that white backdrop-aware vibrant
layer while retaining every filter and parameter. All A0/B native panels were
pixel-identical in both appearances, and the equal-full13-input counterexamples
remained unchanged. Every stage repeated exactly. Light restoration and twelve
old-baseline ROI comparisons still failed the zero-code gate with one-code
differences near the bottom outline. The experiment therefore supplies no
evidence that this layer explains the center mismatch, and its overall
interpretation gate remains failed. Original captures and the independent review
are retained in `internal-docs/research/clear-vibrant-ablation-2026-09-30`.

Coordinate ramps also showed a slope reversal in the inner 2–10 logical-pixel
edge band, while the central slope remained positive. The reversed region moved
with the circle boundary; this cannot be explained by one position-independent
color mapping. It is evidence of spatial optical behavior. The now-verified
color response permits an inverse check, but blur, clipped channels, and the
source/display conversion must still be accounted for before attributing a
pixel displacement to a particular optical parameter.

An inverse check of the fixed color formula across 600 ramp samples supports
the observed inner-refraction amount/height `-60 / 20` in logical pixels with
the candidate profile `1 - sqrt(t * (2 - t))`. At depths 2, 4, and 8 logical
pixels, predicted inward offsets were 33.90, 24.02, and 12.01; measured ranges
were 32.33–35.34, 22.56–25.57, and 10.53–14.29 across appearances and shapes.
No sampled output channel was clipped, and repeats were identical. Maximum
residuals were 2.542 logical pixels for regular and 3.295 for clear. Perturbing
the captured channels by one encoded level produced coordinate spans of
6.02–12.78 logical pixels after inversion and 8-bit ICC conversion. This is
support for the sign, scale, and decay of the candidate, not proof of exact
curve equality or an attribution of the remaining residual to one cause.

A same-view gray→green→gray sweep produced 12 valid inactive observations.
None of the 163 inspected filter/effect/layer fields changed in any of the four
style/appearance combinations. This supports stable host-side inputs for that
experiment; it does not prove that later GPU processing ignores the background.

The comparison's display path is also part of the calibration boundary. The
headless renderer exports untagged RGB PNG bytes, which AppKit interprets as
sRGB before display. GPUI creates a `BGRA8Unorm` `CAMetalLayer` without explicitly
setting its color space. Although Apple's documented creation default is `nil`
(no color matching), the **actual attached GPUIView layer** reported
`kCGColorSpaceSRGB` in the running example. The two measured display paths thus
both specify sRGB on this machine. Creation defaults alone would have led to
an incorrect conclusion; no renderer color-space change was needed.

The live inspection is repeatable and exits after reading the owned window:

```sh
devenv shell -- cargo run -p gpui-cupertino --example glass --locked -- --inspect-color-space
```

This inspects the renderer-owning `GPUIView` inside AppKit's content view,
checks that its layer is a `CAMetalLayer`, and prints its actual color-space
name. It neither sets the property nor assumes an expected value. A formula
verified in display RGB still cannot be copied directly into the renderer's
linear RGB stage: the source/display transform remains part of the model.
See [Apple's colorspace contract](https://developer.apple.com/documentation/quartzcore/cametallayer/colorspace).

No private filter calls, system shader binaries, or host recipes are used by the
library. Local inspection tools, raw logs, and provenance remain in the ignored
research directory. The repository's devenv includes Python with NumPy, SciPy,
Pillow/ImageCms, and Matplotlib for measurement, fitting, and plotting. Run local
analysis scripts with `devenv shell -- python3 work/<script>.py`; package versions
come from the pinned Nix inputs. The recorded fixed-formula results did not use
an optimizer; nonlinear fitting is no longer blocked on the analysis environment.
Active-state parameters were subsequently obtained with `active=true`,
`key=true`, and `visible=true` for all four styles/appearances at 256×128/r20.
Three valid cases came from one run; a separate single-case run supplied dark
clear after the earlier run lost focus. Failed state samples were excluded.
The differences are material: active regular enables refraction opacity 0.3,
bleed opacity approximately 0.3333333 (light) / 0.5333334 (dark), and shadow
opacity approximately 0.3214286 / 0.2928571. Active clear changes the blur-radius
input from 10 to 1, white/black/saturation to 1.15 / 0.075 / 1.06, and fill alpha
to zero; a holding-tone stage is also enabled. All active key/fill highlight
layers have opacity 1 and color alpha 1. The inactive color-model verification
must not be generalized to these active recipes without new pixel probes.
