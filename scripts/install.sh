#!/bin/sh
# Build locked release binaries and install the tools an end user runs.
set -eu

# The packages built and the tools installed from them. Every install line and
# the closing report come from these lists, and a portable-gate regression
# checks them against INSTALL.md and the packages' binary targets.
packages='zetesis-cli zetesis-bench'
tools='zetesis zetesis-bench'

usage='Usage: scripts/install.sh [--cpu-only] [binary-directory]'
cpu_only=false
install_dir=
for argument in "$@"; do
    case "$argument" in
        --help|-h)
            printf '%s\n' "$usage" \
                "Installs $tools into binary-directory (default: \$HOME/.local/bin)." \
                'Builds locked release binaries with GPU support: Metal on macOS, Vulkan elsewhere.' \
                '--cpu-only builds zetesis without GPU support; it then runs on the CPU only.'
            exit 0 ;;
        --cpu-only) cpu_only=true ;;
        -*)
            printf '%s\n' "Unknown option: $argument" "$usage" >&2
            exit 2 ;;
        *)
            if [ -n "$install_dir" ]; then
                printf '%s\n' 'Expected at most one binary directory; see --help.' >&2
                exit 2
            fi
            install_dir=$argument ;;
    esac
done
repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
install_dir=${install_dir:-"$HOME/.local/bin"}
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
if [ "$cpu_only" = true ]; then
    features=--no-default-features
    build='CPU-only build'
else
    features=
    build='default build, with GPU support'
fi
selected=
for package in $packages; do
    selected="$selected -p $package"
done
# This installer produces runnable native binaries, overriding cross-build
# defaults from CARGO_BUILD_TARGET or Cargo configuration deliberately. One
# invocation unifies features; no selected package enables GPU support in
# another, so --cpu-only is honoured. The option and package lists are split
# into words deliberately.
cargo build --locked --release --target "$host_target" \
    --target-dir "$target_dir" $features $selected
mkdir -p -- "$install_dir"
for tool in $tools; do
    install -m 755 "$target_dir/$host_target/release/$tool" "$install_dir/$tool"
done
printf 'Installed %s into %s (%s)\n' "$tools" "$install_dir" "$build"
case ":$PATH:" in
    *":$install_dir:"*) ;;
    *) printf 'Add this directory to PATH: %s\n' "$install_dir" ;;
esac
