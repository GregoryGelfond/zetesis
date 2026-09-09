#!/bin/sh
# A thin source-tree launcher; an explicit binary supports frozen engineering runs.
set -eu
if [ "${ZETESIS_MAINTENANCE+x}" = x ]; then
    exec "$ZETESIS_MAINTENANCE" "$@"
fi
repo_dir=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
exec cargo run --quiet --locked --manifest-path "$repo_dir/Cargo.toml" \
    --package zetesis-maintenance --bin zetesis-maintenance -- "$@"
