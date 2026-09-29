{ pkgs, ... }:
{
  # GPUI compiles Metal with the selected Xcode, which the Nix SDK does not ship.
  apple.sdk = null;

  languages.rust = {
    enable = true;
    channel = "stable";
  };

  languages.python = {
    enable = true;
    package = pkgs.python3.withPackages (ps: [
      ps.numpy
      ps.scipy
      ps.pillow
      ps.matplotlib
    ]);
  };

  scripts.check.exec = ''
    set -euo pipefail
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
    cargo test --workspace --all-features --locked
  '';

  enterTest = ''
    check
  '';
}
