#!/bin/sh
# Run from an ordinary Metal-enabled macOS environment after installation.
set -eu
if [ "$#" -gt 1 ]; then
    printf '%s\n' 'Usage: scripts/qualify-metal.sh [new-result-directory]' >&2
    exit 2
fi
repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
result_dir=${1:-"$repo_dir/target/metal-$(date -u +%Y%m%dT%H%M%SZ)"}
# Refuse to overwrite an earlier qualification record.
mkdir -p -- "$(dirname -- "$result_dir")"
mkdir -- "$result_dir"
zetesis --version > "$result_dir/version.txt"
command -v zetesis > "$result_dir/binary-paths.txt"
command -v zetesis-bench >> "$result_dir/binary-paths.txt"
shasum -a 256 "$(command -v zetesis)" "$(command -v zetesis-bench)" \
    > "$result_dir/binary-sha256.txt"
shasum -a 256 "$repo_dir/examples/network-repair.lp" "$repo_dir/Cargo.lock" \
    "$repo_dir/rust-toolchain.toml" "$repo_dir/crates/zetesis-wgpu/src/oracle.wgsl" \
    > "$result_dir/source-sha256.txt"
cp "$repo_dir/rust-toolchain.toml" "$result_dir/rust-toolchain.toml"
if ! git -C "$repo_dir" rev-parse --verify HEAD > "$result_dir/checkout-revision.txt" 2>/dev/null; then
    printf '%s\n' 'No committed HEAD; binary and source hashes identify this run.' > "$result_dir/checkout-revision.txt"
fi
uname -a > "$result_dir/system.txt"
system_profiler SPDisplaysDataType -json > "$result_dir/graphics.json"
zetesis devices > "$result_dir/devices.txt" 2> "$result_dir/device-diagnostics.txt"
zetesis "$repo_dir/examples/network-repair.lp" --backend metal --grounder eager --models 0 \
    > "$result_dir/network-repair.txt" 2> "$result_dir/solver-diagnostics.txt"
zetesis "$repo_dir/examples/network-repair.lp" --backend cpu --grounder lazy --models 0 \
    > "$result_dir/network-repair-cpu.txt" 2> "$result_dir/cpu-diagnostics.txt"
cmp "$result_dir/network-repair.txt" "$result_dir/network-repair-cpu.txt"
zetesis-bench --backend metal --atoms 64,256 --batches 1,64,256 --repetitions 5 \
    > "$result_dir/oracle.tsv" 2> "$result_dir/benchmark-diagnostics.txt"
printf '%s\n' 'PASS: Metal solve matched lazy CPU output; every microbenchmark result matched the dense CPU oracle.' \
    > "$result_dir/status.txt"
printf 'Metal qualification complete: %s\n' "$result_dir"
