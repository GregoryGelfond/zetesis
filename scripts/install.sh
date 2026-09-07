#!/bin/sh
# Build once, then run zetesis, zetesis-bench and zetesis-validate without Cargo.
set -eu

case "${1-}" in
    --help|-h)
        printf '%s\n' 'Usage: scripts/install.sh [binary-directory]' \
            'Default: $HOME/.local/bin. Builds locked release binaries with GPU support.'
        exit 0 ;;
esac
if [ "$#" -gt 1 ]; then
    printf '%s\n' 'Expected at most one binary directory; see --help.' >&2
    exit 2
fi
repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
install_dir=${1:-"$HOME/.local/bin"}
target_dir=${CARGO_TARGET_DIR:-"$repo_dir/target"}
case "$install_dir" in /*) ;; *) install_dir="$PWD/$install_dir" ;; esac
case "$target_dir" in /*) ;; *) target_dir="$PWD/$target_dir" ;; esac
# Select the repository's pinned Rust toolchain, even when invoked elsewhere.
cd -- "$repo_dir"
host_target=$(rustc -vV | sed -n 's/^host: //p')
if [ -z "$host_target" ]; then
    printf '%s\n' 'Could not determine the native Rust host target.' >&2
    exit 2
fi
# This installer produces runnable native binaries, overriding cross-build
# defaults from CARGO_BUILD_TARGET or Cargo configuration deliberately.
cargo build --locked --release --target "$host_target" \
    --target-dir "$target_dir" -p zetesis-cli -p zetesis-experiments -p zetesis-validation
mkdir -p -- "$install_dir"
install -m 755 "$target_dir/$host_target/release/zetesis" "$install_dir/zetesis"
install -m 755 "$target_dir/$host_target/release/zetesis-bench" "$install_dir/zetesis-bench"
install -m 755 "$target_dir/$host_target/release/zetesis-validate" "$install_dir/zetesis-validate"
printf 'Installed %s, %s and %s\n' "$install_dir/zetesis" "$install_dir/zetesis-bench" "$install_dir/zetesis-validate"
case ":$PATH:" in
    *":$install_dir:"*) ;;
    *) printf 'Add this directory to PATH: %s\n' "$install_dir" ;;
esac
