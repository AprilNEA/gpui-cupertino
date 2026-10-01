# Material rendering

`Glass` wraps an existing GPUI element. Its child determines layout; glass is
painted first, and the child is painted afterward. Keep the child's background
transparent. Put clipping and group opacity on an enclosing element. Child text
and controls remain sharp and do not enter their own backdrop.

## Inactive Clear

`ClearGlassMaterial::new(shape, blur_radius)` validates the outline and host blur
radius in logical pixels. `Glass::clear(material, content)` selects the inactive
recipe for the window's light or dark appearance. The radius controls an
encoded-RGB mip filter and is not a Gaussian sigma. The observed radii 10 and
25/3 belong to the captured 128- and 96-point-high cases; they do not define a
general size-to-radius rule.

Clear uses its own ordered scene primitive and initializes Metal pipelines on
first use. Capture and mip levels use private lossy BGRA8 render targets.
Half-float staging preserves the filter's half arithmetic before each stored
mip. Allocation uses the source bounds, blur radius, and 64-pixel granularity.
Small radii retain the base mip. The filter uses bottom-origin coordinates;
compositing maps those coordinates into GPUI's top-origin framebuffer.

For a tagged live layer, public Core Graphics exports the actual layer and
display ICC profiles. Public ColorSync supplies parametric RGB matrices and
curves; independent Metal kernels execute both conversions. The renderer caches
conversion parameters by the actual color-space pair. Nil or equal spaces need
no conversion. Unsupported profile components return an error.

The forward conversion preserves the observed half-precision input and output
boundaries before writing display BGRA8. Material composition uses RGBA16Float
and clamps to the normalized target range. The return conversion writes layer
BGRA8, and the renderer restores only covered pixels. The attached layer keeps
its original format and color space.

The production headless renderer and actual GPUI WindowServer captures each
pass the original 24-capture, 104-region inactive Clear matrix. Maximum
individual RGB error is one code, and every complete control panel is exact.
The live gate retains the original source PNGs, display profile, frame, scale 2,
and inactive state. Active and Regular recipes, HDR, and native matching at
other scales are outside that result. Clear requires an Apple GPU and an
opaque SDR target; tagged color spaces must support parametric RGB conversion.

## Gaussian capture and filtering

`Window::paint_backdrop` and `Window::paint_clear_backdrop` insert scene ordering
boundaries. Earlier primitives
must finish before capture; later primitives cannot batch across that boundary.
Replaying cached paint commands inserts a new boundary and takes a new snapshot,
so scrolling and changing backgrounds do not reuse stale pixels. Consecutive
glass elements compose in paint order: a later glass can see an earlier one.

The Metal renderer ends its render pass, copies the window texture, and decodes
it into linear RGB, half-float storage. Uniform blur uses two separable Gaussian
passes. Captured pixels are treated as constant unit cells: weights integrate a
continuous Gaussian over each source cell, then sample at destination centers.
The CPU computes weights with `libm::erf`; paired linear texture samples combine
adjacent taps. Ordinary support ends at four standard deviations and is
normalized. When the window edge limits support, the remaining tail is folded
into clamp-to-edge taps.

A previous scale-pyramid approximation failed the independent Gaussian oracle,
particularly for small radii. Uniform blur now evaluates the requested kernel
directly; variable-radius LOD remains future work. The background snapshot and
two working textures use approximately 20 bytes per window pixel during blur.

An analytic field defines continuous circle, capsule, or rounded rectangle
coverage. The corner radius expands by `1.528665`; each axis blends toward a
circular corner when the expanded radius exceeds the available half-size.
The field retains half-precision quantization of normalized radial distance.
Distances outside half's finite range remain float, preserving coverage for
very small radii. Refraction strength follows inward edge distance; optical
direction can interpolate between the shape's optical direction and a radial
direction. The shape's optical direction is separate from the field's derivative. Blur,
refraction, dispersion, edge bleed, tint, and directional highlights have separate controls.
Edge bleed mixes filtered color from just outside the contour into its inner
edge band, using the refraction band's width and an independent strength.
These are independent calibration values, not undocumented Apple style recipes.

The renderer resumes the ordinary GPUI pass to paint foreground content. No
SwiftUI, private framework filter, or Apple shader binary is loaded.

## Color and platform contract

- Initial support is the macOS Metal backend and an opaque SDR window background.
- Gaussian tint uses straight linear RGBA; saturation and brightness operate in linear RGB.
  Ancestor opacity blends in linear RGB before the result is encoded to the
  window's sRGB values. Geometric edge coverage blends those encoded values.
  The identity material returns the captured values directly.
- Transparent window backdrops, HDR, desktop capture, and other applications'
  windows are outside this contract. Windows DirectX and WGPU need separate
  implementations and acceptance checks; WGPU rejects the backdrop primitive.
- Each Gaussian effect builds its own full-window snapshot and filtered texture.
  Clear uses an aligned capture of its bounds; live color conversion also uses
  full-window working textures. Shared captures, cropping, and persistent
  texture caches require measured performance and dependency-aware invalidation.
- Shapes are independent. A fused shape group needs a joint distance field and
  a shared capture; overlapping `Glass` elements do not imply fusion.

## Accessibility and motion

Glass observes public AppKit accessibility settings and refreshes when they
change. Reduce Transparency produces an opaque light or dark surface; Increase
Contrast also adds a contrasting outline. Explicit `GlassAccessibility` settings
can enable these behaviors in addition to system preferences.

Both opaque modes use the same continuous field as glass without capturing the
background. `Window::paint_continuous_quad` supports equal corner radii and an
optional uniform solid border. Dashed or nonuniform borders are rejected. Its
separate Metal batch preserves paint order and the ordinary GPUI quad layout.

`motion::spring` translates validated duration/bounce settings to GPUI's existing
spring. Use `AnimationExt::with_spring` with a stable element ID to preserve
velocity when retargeting. System Reduce Motion updates GPUI's setting. Apps
using springs without glass should initialize
`platform::accessibility_preferences(cx)` during startup.

## Acceptance checks

`devenv test` runs material validation, spring conversion, scene barrier/replay
checks, and Metal readback tests. The GPU checks cover identity, changing and
resized backgrounds at 1×/2×, clipping, all three shapes, linear color, blur
normalization, symmetry, and foreground exclusion.
Optical checks exercise refraction, dispersion, neighboring color bleed, and
highlight scaling. A readback benchmark covers zero, one, and two effects:

```sh
cargo bench -p gpui-cupertino --bench backdrop --locked
cargo bench -p gpui-cupertino --bench backdrop --locked -- --clear
```

The benchmark measures the full synchronous render/readback path, including
host work. These headless timings exclude live color conversion. The separate
attached-window sample is recorded in [validation results](validation.md).

The workspace also compares 262,144 converted pixels against the public
ColorSync CPU implementation, using sRGB, Display P3, and Adobe RGB 1998.
Every channel must agree within one code, with at least 99.9% exact channels.

Continuous-corner checks retain 144 measured native edge samples and compare
glass with opaque surfaces at 1×/2×. They also check contrast borders, parent
clipping, paint order, cached replay, tiny radii, and linear ancestor opacity.
To export the fixed constant-gray contour matrix into a new directory, run:

```sh
cargo run -p gpui-cupertino --example contours --locked -- /tmp/cupertino-contours
```

Run `cargo run -p gpui-cupertino --example glass --locked` in `devenv shell` for
the interactive coordinate grid. Scroll and resize, reverse the toolbar spring
before it settles, and toggle the appearance and accessibility overrides.
Gaussian controls remain independent material parameters. The inactive Clear
recipe remains fixed when the window gains focus; active-style matching requires
a separate recipe and acceptance matrix.

See [validation results](validation.md) for the independent reference, repeated
measurements, actual retained-animation checks, and native comparison procedure.
