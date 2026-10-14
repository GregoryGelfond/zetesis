#!/bin/sh
# Build locked release binaries and install the tools an end user runs.
# Precondition: one install at a time per Cargo target directory and per binary
# directory. Both builds of zetesis write the same artifact, and Cargo
# serializes builds but not the copies this script makes after them.
set -eu

# The packages built and the tools installed from them. Every install line and
# the closing report come from these lists, and a portable-gate regression
# checks them against INSTALL.md and the packages' binary targets.
packages='zetesis-cli zetesis-bench'
tools='zetesis zetesis-bench'
# The directory beside the tools that holds the default install's GPU-capable
# zetesis, one subdirectory per install identity. The same regression checks it
# against the directory the CLI looks in and against INSTALL.md.
libexec='zetesis-libexec'

usage='Usage: scripts/install.sh [--cpu-only] [binary-directory]'
cpu_only=false
install_dir=
for argument in "$@"; do
    case "$argument" in
        --help|-h)
            printf '%s\n' "$usage" \
                "Installs $tools into binary-directory (default: \$HOME/.local/bin)." \
                'Builds locked release binaries. zetesis runs on the CPU without loading the GPU stack' \
                "and hands GPU requests to the GPU-capable zetesis it installs in $libexec beside it:" \
                'Metal on macOS, Vulkan elsewhere.' \
                "--cpu-only builds zetesis without GPU support and removes $libexec; it then runs on the CPU only."
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
repo_dir=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
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
selected=
for package in $packages; do
    selected="$selected -p $package"
done
built=$target_dir/$host_target/release
stage=$target_dir/install-stage

# This installer produces runnable native binaries, overriding cross-build
# defaults from CARGO_BUILD_TARGET or Cargo configuration deliberately. One
# invocation unifies features, and no selected package enables GPU support in
# another. The option and package lists are split into words deliberately.
build() {
    cargo build --locked --release --target "$host_target" --target-dir "$target_dir" "$@"
}

# Copy $1 to $2 under a temporary name beside $2, then rename it into place, so
# that $2 is replaced whole or not at all.
place() {
    temporary=$(dirname -- "$2")/.$(basename -- "$2").installing
    install -m 755 "$1" "$temporary"
    mv -f -- "$temporary" "$2"
}

# The SHA-256 of $1, in hexadecimal.
digest() {
    if command -v sha256sum > /dev/null 2>&1; then
        sum=$(sha256sum -- "$1")
    else
        sum=$(shasum -a 256 -- "$1")
    fi
    printf '%s\n' "${sum%% *}"
}

# Every build finishes, and its executables are set aside, before anything in
# the binary directory changes, so a failed build leaves the previous install
# as it was.
rm -rf -- "${stage:?}"
mkdir -p -- "$stage"
if [ "$cpu_only" = true ]; then
    build --no-default-features $selected
    for tool in $tools; do
        cp -- "$built/$tool" "$stage/$tool"
    done
    report='CPU-only build'
else
    # The GPU-capable zetesis, set aside before the front's build reuses its
    # artifact path; its content names the install.
    build $selected
    mkdir -p -- "$stage/sibling"
    cp -- "$built/zetesis" "$stage/sibling/zetesis"
    cp -- "$built/zetesis-bench" "$stage/zetesis-bench"
    version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
    hash=$(digest "$stage/sibling/zetesis")
    identity=$version-$(printf '%s' "$hash" | cut -c 1-16)
    # The front: zetesis without the GPU stack, naming the sibling it hands GPU
    # requests to. The identity is exported in a subshell: POSIX leaves it
    # unspecified whether an assignment before a function call reaches the
    # commands the function runs.
    (
        ZETESIS_SIBLING_IDENTITY=$identity
        export ZETESIS_SIBLING_IDENTITY
        build --no-default-features --features front -p zetesis-cli
    )
    cp -- "$built/zetesis" "$stage/zetesis"
    # The front exists so that CPU runs start without a graphics framework. On
    # macOS the load commands decide it: reading them is a value-dependent step
    # in this otherwise glue script, so the GPU build's are read first as its
    # control, since that build links Metal at launch.
    if [ "$(uname -s)" = Darwin ]; then
        if ! gpu_load_commands=$(/usr/bin/otool -L "$stage/sibling/zetesis"); then
            printf '%s\n' 'Could not read the GPU build load commands; not installing.' >&2
            exit 1
        fi
        if ! printf '%s\n' "$gpu_load_commands" | grep -q '\.framework/'; then
            printf '%s\n' 'Found no framework in the load commands of the GPU build: the GPU build no longer links a framework at launch and the CPU front may no longer be needed; not installing.' >&2
            exit 1
        fi
        if ! front_load_commands=$(/usr/bin/otool -L "$stage/zetesis"); then
            printf '%s\n' 'Could not read the CPU front load commands; not installing.' >&2
            exit 1
        fi
        if printf '%s\n' "$front_load_commands" | grep -q '\.framework/'; then
            printf '%s\n' 'The CPU front links a framework, so it would start like the GPU build; not installing.' >&2
            exit 1
        fi
    fi
    report="default build: zetesis hands GPU requests to $libexec/$identity/zetesis"
fi

# Each tool, with zetesis last: in a default install the installed front
# names an installed sibling at every point, since the new sibling is placed
# before the new front, and the previous one is removed only after it.
place_tools() {
    for tool in $tools; do
        if [ "$tool" != zetesis ]; then
            place "$stage/$tool" "$install_dir/$tool"
        fi
    done
    place "$stage/zetesis" "$install_dir/zetesis"
}

# Remove $1, which the completed install no longer uses: a directory that
# cannot be removed is reported, and is not an error.
retire() {
    if ! rm -rf -- "$1"; then
        printf 'Could not remove %s, which this install no longer uses; remove it by hand.\n' "$1" >&2
    fi
}

mkdir -p -- "$install_dir"
if [ "$cpu_only" = true ]; then
    place_tools
    # The CPU-only zetesis never looks in this directory.
    retire "${install_dir:?}/$libexec"
else
    mkdir -p -- "$install_dir/$libexec/$identity"
    place "$stage/sibling/zetesis" "$install_dir/$libexec/$identity/zetesis"
    place_tools
    # The identity directories this install supersedes are chosen by the shape
    # of their names, the script's second value-dependent step beside the load
    # command reading: a directory whose name begins with a digit and ends in a
    # hyphen and sixteen lowercase hexadecimal digits, as an identity's version
    # and digest do. Any other name is kept; a directory of that shape placed
    # here by hand is removed with them. The classes list their characters,
    # since what a range matches depends on the locale.
    hex='[0123456789abcdef]'
    superseded="[0123456789]*-$hex$hex$hex$hex$hex$hex$hex$hex$hex$hex$hex$hex$hex$hex$hex$hex"
    for previous in "$install_dir/$libexec"/*; do
        name=${previous##*/}
        if [ -d "$previous" ] && [ "$name" != "$identity" ]; then
            case "$name" in
                $superseded) retire "$previous" ;;
            esac
        fi
    done
fi
rm -rf -- "${stage:?}"
printf 'Installed %s into %s (%s)\n' "$tools" "$install_dir" "$report"
case ":$PATH:" in
    *":$install_dir:"*) ;;
    *) printf 'Add this directory to PATH: %s\n' "$install_dir" ;;
esac
