#!/bin/sh
# Physical qualification of one device backend: every reviewed group of exact
# hardware tests must pass on this host, checking complete CPU/device families
# and explicit failure boundaries. No coverage instrumentation; coverage has its own
# Metal population.
set -eu

backend=${1:-}
if [ "$#" -gt 1 ]; then
    printf '%s\n' 'Usage: scripts/hardware.sh [--metal|--vulkan]' >&2
    exit 2
fi
case "$backend" in
    --metal) backend=metal ;;
    --vulkan) backend=vulkan ;;
    '')
        # The host's own backend: Metal on macOS, Vulkan elsewhere.
        case "$(uname -s)" in
            Darwin) backend=metal ;;
            *) backend=vulkan ;;
        esac ;;
    *)
        printf '%s\n' 'Usage: scripts/hardware.sh [--metal|--vulkan]' >&2
        exit 2 ;;
esac
repo_dir=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
cd -- "$repo_dir"
# The reviewed selection of this backend, as zetesis-maintenance embeds it;
# the checker refuses any other table.
if [ "$backend" = vulkan ]; then
    table=$(cat crates/zetesis-maintenance/src/coverage/physical-selection-vulkan.txt)
else
    table=$(cat crates/zetesis-maintenance/src/coverage/physical-selection.txt)
fi
hardware_dir="$repo_dir/target/hardware"
mkdir -p -- "$hardware_dir"
printf '%s\n' incomplete > "$hardware_dir/$backend-status.txt"

run_group() {
    group=$1
    target=$2
    names=$3
    # Splitting is intentional: these are fixed libtest identifiers, not input.
    set -- $names
    if [ "$target" = lib ]; then
        set -- --lib -- --ignored --nocapture --test-threads=1 --exact "$@"
    else
        set -- --test "$target" -- --ignored --nocapture --test-threads=1 --exact "$@"
    fi
    log="$hardware_dir/$backend-$group.log"
    status="$hardware_dir/$backend-$group-status.txt"
    printf '%s\n' incomplete > "$status"
    # Workspace feature unification, as the coverage gate keeps it.
    if CARGO_TERM_COLOR=never cargo +1.97.1 test --locked --workspace --all-features \
        "$@" > "$log" 2>&1; then
        cat "$log"
    else
        group_exit=$?
        cat "$log" >&2 || :
        return "$group_exit"
    fi
    # Cargo permits a successful zero-match selection: check the named
    # records and the pinned libtest summaries.
    scripts/maintenance.sh coverage-physical --log "$log" --group "$group" --table "$table"
    printf '%s\n' passed > "$status"
}

while IFS='|' read -r group target expected names; do
    printf '%s\n' incomplete > "$hardware_dir/$backend-$group-status.txt"
    : > "$hardware_dir/$backend-$group.log"
done <<EOT
$table
EOT
while IFS='|' read -r group target expected names; do
    run_group "$group" "$target" "$names"
done <<EOT
$table
EOT
printf '%s\n' passed > "$hardware_dir/$backend-status.txt"
printf '%s\n' "hardware qualification passed: $backend, 16 groups, 59 exact tests"
