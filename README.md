# Cupertino

Apple-inspired components, materials, and motion for GPUI.

| Crate | Responsibility |
| --- | --- |
| `cupertino` | Framework-independent design values and motion primitives. |
| `gpui-cupertino` | GPUI components, rendering, and interaction; depends on `cupertino`. |

The first implementation provides window-local glass on macOS Metal: analytic
shapes, blur, tint, refraction, dispersion, highlights, and accessibility fallbacks.
Motion delegates to GPUI springs. Publishing remains disabled while the API and
license are being defined.

See [repository layout](docs/architecture.md) for directory responsibilities and
dependency boundaries, and [material rendering](docs/materials.md) for the rendering
contract and current limits.

```rust
use cupertino::materials::{GlassMaterial, GlassMaterialOptions, GlassShape};
use gpui::{div, prelude::*};
use gpui_cupertino::materials::Glass;

let material = GlassMaterial::try_from(GlassMaterialOptions {
    shape: GlassShape::Capsule,
    blur_sigma: 8.0,
    tint: [1.0, 1.0, 1.0, 0.12], // Linear RGBA.
    ..Default::default()
})?;
let toolbar = Glass::new(material, div().px_4().py_2().child("Library"));
```

## Development

Install [Nix](https://nixos.org/download/) and [devenv](https://devenv.sh/getting-started/).
devenv provides Rust stable, Cargo, rustfmt, Clippy, and rust-analyzer;
`devenv.lock` pins the environment inputs.
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
