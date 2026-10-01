# Cupertino

Apple-inspired components, materials, and motion for GPUI.

| Crate | Responsibility |
| --- | --- |
| `cupertino` | Framework-independent design values and motion primitives. |
| `gpui-cupertino` | GPUI components, rendering, and interaction; depends on `cupertino`. |

The implementation provides window-local glass on macOS Metal: inactive Clear,
configurable Gaussian materials, continuous outlines, and accessibility fallbacks.
Motion delegates to GPUI springs. Publishing remains disabled while the API and
license are being defined.

See [repository layout](docs/architecture.md) for directory responsibilities and
dependency boundaries, and [material rendering](docs/materials.md) for the rendering
contract and current limits.

```rust
use cupertino::materials::{ClearGlassMaterial, GlassShape};
use gpui::{div, prelude::*, px};
use gpui_cupertino::materials::Glass;

let material = ClearGlassMaterial::new(
    GlassShape::RoundedRectangle { corner_radius: 20.0 },
    10.0,
)?;
let panel = Glass::clear(material, div().w(px(240.0)).h(px(128.0)).child("Library"));
```

Clear requires an Apple GPU. Its host blur radius is distinct from Gaussian
sigma. Actual GPUI window captures pass all 24 images and 104 reference regions
on the measured SDR setup. See [validation results](docs/validation.md) for the
one-code error limit and supported scope.

## Development

Install [Nix](https://nixos.org/download/) and [devenv](https://devenv.sh/getting-started/).
devenv provides Rust stable, Cargo, rustfmt, Clippy, and rust-analyzer, plus
Python with NumPy, SciPy, Pillow (including ICC color management), and Matplotlib
for material calibration. `devenv.lock` pins the environment inputs and Python
packages; no separate pip installation is needed.
On macOS, select a full Xcode installation with its Metal Toolchain installed;
`xcrun -sdk macosx metal --version` must succeed. The environment uses Xcode's SDK
because building GPUI's shaders requires the Metal compiler.

```sh
devenv shell
cargo build --workspace --locked
cargo run -p gpui-cupertino --example glass --locked
check
```

`check` runs formatting checks, Clippy with warnings treated as errors, and tests
for the workspace. Run the same checks without entering a shell with:

```sh
devenv test
```

The checks include actual Metal readback tests. The example has a scrollable
coordinate grid, reversible spring motion, neighboring glass, and accessibility
overrides. GPUI is pinned and patched in [vendor/](vendor/README.md); use that
copy when integrating this workspace.
