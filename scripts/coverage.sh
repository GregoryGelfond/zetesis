#!/bin/sh
# Separate, freshly instrumented profiles; each must pass the committed floor.
set -eu

mode=${1:-gate}
usage='Usage: scripts/coverage.sh [gate|baseline] [--metal|--vulkan]'
backend=
if [ "$#" -gt 2 ]; then
    printf '%s\n' "$usage" >&2
    exit 2
fi
if [ "$#" -eq 2 ]; then
    case "$2" in
        --metal) backend=metal ;;
        --vulkan) backend=vulkan ;;
        *)
            printf '%s\n' "$usage" >&2
            exit 2 ;;
    esac
fi
case "$mode" in gate|baseline) ;; *)
    printf '%s\n' "$usage" >&2
    exit 2 ;;
esac

repo_dir=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
cd -- "$repo_dir"
coverage_dir="$repo_dir/target/coverage"
mkdir -p -- "$coverage_dir"
if ! mkdir -- "$coverage_dir/.lock" 2>/dev/null; then
    printf '%s\n' 'Another coverage run owns target/coverage/.lock; reports cannot be shared concurrently.' >&2
    exit 2
fi
trap 'rmdir -- "$coverage_dir/.lock"' 0
trap 'exit 130' INT
trap 'exit 143' TERM
# A previous complete report never certifies an interrupted new run.
printf '%s\n' incomplete > "$coverage_dir/status.txt"
: > "$coverage_dir/floors.tsv"
floor=$(scripts/maintenance.sh coverage-floor --mode "$mode" --path scripts/coverage-floor.txt)
# The physical stage runs its backend's reviewed selection, read from the file
# the hardware gate reads; the checker refuses any other table. Each row names a
# report group, Cargo target, required count and exact tests. All invocations
# keep workspace feature unification, including the library selection:
# narrowing to one package can change instrumented dependency builds.
case "$backend" in
    metal) table=$(cat crates/zetesis-maintenance/src/coverage/physical-selection.txt) ;;
    vulkan) table=$(cat crates/zetesis-maintenance/src/coverage/physical-selection-vulkan.txt) ;;
    *) table= ;;
esac
# The test-support crates hold test code. Every report skips their sources, as
# cargo-llvm-cov already skips tests/ directories, so the floors measure product
# code.
support_sources='/crates/zetesis-(test|theory|clingo|reference)-support/'

tool_version=$(cargo +1.97.1 llvm-cov --version)
if [ "$tool_version" != 'cargo-llvm-cov 0.8.7' ]; then
    printf 'Expected cargo-llvm-cov 0.8.7; found %s\n' "$tool_version" >&2
    exit 2
fi
if { [ -n "${LLVM_COV:-}" ] && [ -z "${LLVM_PROFDATA:-}" ]; } ||
   { [ -z "${LLVM_COV:-}" ] && [ -n "${LLVM_PROFDATA:-}" ]; }; then
    printf '%s\n' 'Set both LLVM_COV and LLVM_PROFDATA, or neither.' >&2
    exit 2
fi
if [ -z "${LLVM_COV:-}" ]; then
    sysroot=$(rustc +1.97.1 --print sysroot)
    host=$(rustc +1.97.1 -vV | sed -n 's/^host: //p')
    LLVM_COV="$sysroot/lib/rustlib/$host/bin/llvm-cov"
    LLVM_PROFDATA="$sysroot/lib/rustlib/$host/bin/llvm-profdata"
fi
export LLVM_COV LLVM_PROFDATA
rust_version=$(rustc +1.97.1 -vV)
cov_version=$("$LLVM_COV" --version)
profdata_version=$("$LLVM_PROFDATA" --version)
set -- coverage-metadata --mode "$mode" --floor "$floor" \
    --rustc-version "$rust_version" --cargo-llvm-cov-version "$tool_version" \
    --llvm-cov "$LLVM_COV" \
    --llvm-cov-version "$cov_version" --llvm-profdata "$LLVM_PROFDATA" \
    --llvm-profdata-version "$profdata_version"
if [ -n "$backend" ]; then
    set -- "$@" --physical-backend "$backend" --physical-table "$table"
fi
scripts/maintenance.sh "$@" > "$coverage_dir/toolchain.json"

write_report() {
    destination=$1
    shift
    mkdir -p -- "$destination"
    # In 0.8.7, report rejects build-feature flags despite listing them in help.
    # The separate instrumented directories retain each feature configuration.
    cargo +1.97.1 llvm-cov report "$@" --locked \
        --ignore-filename-regex "$support_sources" --json \
        --output-path "$destination/coverage.json"
    cargo +1.97.1 llvm-cov report "$@" --locked \
        --ignore-filename-regex "$support_sources" --html --output-dir "$destination"
}

run_physical_group() {
    group=$1
    target=$2
    expected=$3
    names=$4
    # Splitting is intentional: these are fixed libtest identifiers, not input.
    set -- $names
    if [ "$target" = lib ]; then
        set -- --lib --locked --no-report -- --ignored --nocapture \
            --test-threads=1 --exact "$@"
    else
        set -- --test "$target" --locked --no-report -- --ignored \
            --nocapture --test-threads=1 --exact "$@"
    fi
    physical_log="$coverage_dir/workspace/$backend-$group.log"
    physical_status="$coverage_dir/workspace/$backend-$group-status.txt"
    # Keep workspace feature unification and the existing instrumented target.
    # A run that defers its report never cleans, so the portable profile is
    # retained; no CLI-CPU data enters this stage.
    if CARGO_TERM_COLOR=never cargo +1.97.1 llvm-cov --workspace --all-features \
        "$@" > "$physical_log" 2>&1; then
        cat "$physical_log"
    else
        physical_exit=$?
        cat "$physical_log" >&2 || :
        return "$physical_exit"
    fi
    # Cargo permits a successful zero-match selection. Check both named test
    # records and pinned libtest summaries; output within a test may share its
    # first line under --nocapture. Other workspace libraries may report zero.
    if ! scripts/maintenance.sh coverage-physical --log "$physical_log" \
        --group "$group" --table "$table"
    then
        return 1
    fi
    printf '%s\n' passed > "$physical_status"
}

run_physical() {
    printf '%s\n' incomplete > "$coverage_dir/workspace/$backend-status.txt"
    # Reset every group before any execution, including groups an early failure
    # would leave unvisited. Old logs and status markers cannot certify this run.
    while IFS='|' read -r group target expected names; do
        printf '%s\n' incomplete > "$coverage_dir/workspace/$backend-$group-status.txt"
        : > "$coverage_dir/workspace/$backend-$group.log"
    done <<EOF
$table
EOF
    while IFS='|' read -r group target expected names; do
        run_physical_group "$group" "$target" "$expected" "$names"
    done <<EOF
$table
EOF
    printf '%s\n' passed > "$coverage_dir/workspace/$backend-status.txt"
}

run_profile() {
    profile=$1
    shift
    report_dir="$coverage_dir/$profile"
    export CARGO_LLVM_COV_TARGET_DIR="$coverage_dir/build-$profile"
    mkdir -p -- "$report_dir"
    cargo +1.97.1 llvm-cov clean --workspace --locked
    if [ "$profile" = workspace ]; then
        cargo +1.97.1 llvm-cov --workspace "$@" --locked --no-report
        if [ -n "$backend" ]; then
            write_report "$report_dir/portable"
            run_physical
        fi
        set --
    else
        cargo +1.97.1 llvm-cov "$@" --locked --no-report
        set -- --package zetesis-cli --package zetesis-solve
    fi
    write_report "$report_dir" "$@"
}

run_profile workspace --all-features
# Keep this artifact name; the CPU population includes the extracted solver.
run_profile cli-cpu --package zetesis-cli --package zetesis-solve --no-default-features

export CARGO_LLVM_COV_TARGET_DIR="$coverage_dir/build-workspace"
if [ "$mode" = gate ]; then
    # Both reports already exist. Retain both independent floor verdicts even
    # when the workspace population has not met its unchanged floor.
    workspace_floor_exit=0
    if cargo +1.97.1 llvm-cov report --locked \
        --ignore-filename-regex "$support_sources" --fail-under-lines "$floor"; then
        :
    else
        workspace_floor_exit=$?
    fi
    printf 'workspace\t%s\n' "$workspace_floor_exit" >> "$coverage_dir/floors.tsv"
    export CARGO_LLVM_COV_TARGET_DIR="$coverage_dir/build-cli-cpu"
    cpu_floor_exit=0
    if cargo +1.97.1 llvm-cov report --package zetesis-cli --package zetesis-solve --locked \
        --ignore-filename-regex "$support_sources" --fail-under-lines "$floor"; then
        :
    else
        cpu_floor_exit=$?
    fi
    printf 'cli-cpu\t%s\n' "$cpu_floor_exit" >> "$coverage_dir/floors.tsv"
    if [ "$workspace_floor_exit" -ne 0 ]; then
        exit "$workspace_floor_exit"
    fi
    if [ "$cpu_floor_exit" -ne 0 ]; then
        exit "$cpu_floor_exit"
    fi
    completion_status=gate-passed
else
    completion_status='baseline-complete (nongating)'
fi
printf 'Coverage %s completed; reports: %s\n' "$mode" "$coverage_dir"
printf '%s\n' "$completion_status" > "$coverage_dir/status.txt"
