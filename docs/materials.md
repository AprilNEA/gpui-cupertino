# Material rendering

`Glass` wraps an existing GPUI element. Its child determines layout; glass is
painted first, and the child is painted afterward. Keep the child's background
transparent. Put clipping and group opacity on an enclosing element. Child text
and controls remain sharp and do not enter their own backdrop.

## Capture and filtering

`Window::paint_backdrop` inserts a scene ordering boundary. Earlier primitives
must finish before capture; later primitives cannot batch across that boundary.
Replaying cached paint commands inserts a new boundary and takes a new snapshot,
so scrolling and changing backgrounds do not reuse stale pixels. Consecutive
glass elements compose in paint order: a later glass can see an earlier one.

The Metal renderer ends its render pass, copies the window texture, and builds
a linear RGB, half-float Gaussian scale pyramid. Each level applies a normalized,
separable, seven-tap filter with doubled spacing. Levels keep their full spatial
resolution: decimation caused asymmetric impulse responses and phase-dependent
blur during scrolling. Only the two levels bracketing the requested `blur_sigma`
are retained, and they are blended by variance. This is an explicit filter chain,
not automatically generated mipmaps; its kernel approximates a Gaussian.

An analytic signed distance field defines circle, capsule, or rounded rectangle
coverage. Refraction strength follows inward edge distance; optical direction
can interpolate between the contour normal and a radial direction. Blur,
refraction, dispersion, edge bleed, tint, and directional highlights have separate controls.
Edge bleed mixes filtered color from just outside the contour into its inner
edge band, using the refraction band's width and an independent strength.
These are independent calibration values, not undocumented Apple style recipes.

The renderer resumes the ordinary GPUI pass to paint foreground content. No
SwiftUI, private framework filter, or Apple shader binary is loaded.

## Color and platform contract

- Initial support is the macOS Metal backend and an opaque SDR window background.
- Tint uses straight linear RGBA; saturation and brightness operate in linear RGB.
  The final result is encoded back to the window's sRGB values. The identity
  material returns the captured values directly.
- Transparent window backdrops, HDR, desktop capture, and other applications'
  windows are outside this contract. Windows DirectX and WGPU need separate
  implementations and acceptance checks; WGPU rejects the backdrop primitive.
- Each effect currently builds its own full-window snapshot and pyramid. Shared
  captures, cropping, and persistent caches require measured performance and
  dependency-aware invalidation before introduction.
- Shapes are independent. A fused shape group needs a joint distance field and
  a shared capture; overlapping `Glass` elements do not imply fusion.

## Accessibility and motion

Glass observes public AppKit accessibility settings and refreshes when they
change. Reduce Transparency produces an opaque light or dark surface; Increase
Contrast also adds a contrasting outline. Explicit `GlassAccessibility` settings
can enable these behaviors in addition to system preferences.

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
cargo bench -p gpui-cupertino --bench backdrop --profile dev --locked
```

This measures the full synchronous render/readback path, including host work;
it is not a pure GPU timestamp or an interactive frame-rate claim.

Run `cargo run -p gpui-cupertino --example glass --locked` in `devenv shell` for
the interactive coordinate grid. Scroll and resize, reverse the toolbar spring
before it settles, and toggle the appearance and accessibility overrides. Native
Apple pixel equivalence is not claimed; collecting native reference windows and
runtime parameter traces remains separate research.
