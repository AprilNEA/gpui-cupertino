# Cupertino

Apple-inspired components, materials, and motion for GPUI.

| Crate | Responsibility |
| --- | --- |
| `cupertino` | Framework-independent design values and motion primitives. |
| `gpui-cupertino` | GPUI components, rendering, and interaction; depends on `cupertino`. |

The workspace currently contains the development setup and crate skeletons.
Components and the GPUI dependency will be added with the first implementation.
Publishing is disabled until the initial API and license are defined.

See [repository layout](docs/architecture.md) for directory responsibilities and
dependency boundaries.

## Development

Install [Nix](https://nixos.org/download/) and [devenv](https://devenv.sh/getting-started/).
devenv provides Rust stable, Cargo, rustfmt, Clippy, and rust-analyzer;
`devenv.lock` pins the environment inputs.

```sh
devenv shell
cargo build --workspace --locked
check
```

`check` runs formatting checks, Clippy with warnings treated as errors, and tests
for the workspace. Run the same checks without entering a shell with:

```sh
devenv test
```
