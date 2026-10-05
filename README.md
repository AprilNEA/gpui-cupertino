# Cupertino

Apple-inspired components, materials, and motion for GPUI.

| Crate | Responsibility |
| --- | --- |
| `cupertino` | Framework-independent design values and motion primitives. |
| `gpui-cupertino` | GPUI components, rendering, and interaction; depends on `cupertino`. |

The implementation provides a light/dark theme, form controls, single-line text
input, tabs, sidebars, split views, virtual lists and tables, scrolling, menus,
dialogs, sheets, and tooltips.
Window-local glass on macOS Metal includes inactive Clear,
configurable Gaussian materials, continuous outlines, and accessibility fallbacks.
Motion delegates to GPUI springs. The full component library remains in development;
publishing is disabled while the API and license are being defined.

See [component contracts](docs/components.md), [material rendering](docs/materials.md),
and the [repository goal and milestones](docs/roadmap.md). The
[repository layout](docs/architecture.md) defines dependency boundaries.

```rust
use gpui_cupertino::components::Button;

let save = Button::new("save", "Save")
    .primary()
    .on_click(|_, _, _| println!("Save"));
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
cargo run -p gpui-cupertino --example components --locked
cargo run -p gpui-cupertino --example collections --locked
cargo run -p gpui-cupertino --example detail --locked
cargo run -p gpui-cupertino --example navigation --locked
cargo run -p gpui-cupertino --example glass --locked
check
```

`check` runs formatting checks, Clippy with warnings treated as errors, and tests
for the workspace. Run the same checks without entering a shell with:

```sh
devenv test
```

The checks include interaction, accessibility, and actual Metal readback tests.
The examples cover a settings form, searchable multiple-selection collections,
detail editing with nested modal panels, and workspace navigation with resizable
panes, retained tabs, empty states, and loading progress. Call
`gpui_cupertino::init(cx)` at startup to enable component key bindings and macOS
accessibility preference updates. The glass
example has a scrollable
coordinate grid, reversible spring motion, neighboring glass, and accessibility
overrides. GPUI uses the exact `=0.1.1` GPUI Alloy registry packages, with the existing Rust aliases. The [vendor snapshot](vendor/README.md) remains as source history.
macOS applications must enable the `font-kit` feature on `gpui_platform` to render text; the examples enable this feature.
Native text-field and Popover focus checks pass with GPUI Alloy `0.1.1`; Chinese IME and VoiceOver acceptance remain incomplete. See [component verification](docs/components.md#verification).
The [native comparison procedure](docs/validation.md#reproduction-and-native-comparison)
supports separate active and inactive material probes.
