#!/bin/sh
# Compare the current checkout's native binary with external clingo.
set -eu
repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
target_dir=${CARGO_TARGET_DIR:-"$repo_dir/target"}
case "$target_dir" in /*) ;; *) target_dir="$PWD/$target_dir" ;; esac
cd -- "$repo_dir"
host_target=$(rustc -vV | sed -n 's/^host: //p')
test -n "$host_target"
cargo build --locked --release --target "$host_target" --target-dir "$target_dir" \
    -p zetesis-cli -p zetesis-validation
exec "$target_dir/$host_target/release/zetesis-validate" --repo "$repo_dir" \
    --zetesis "$target_dir/$host_target/release/zetesis" "$@"
