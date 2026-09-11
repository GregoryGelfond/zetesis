#!/bin/sh
# Separate, freshly instrumented profiles; each must pass the committed floor.
set -eu

mode=${1:-gate}
metal=${2:-}
if [ "$#" -gt 2 ] || { [ "$#" -eq 2 ] && [ "$metal" != --metal ]; }; then
    printf '%s\n' 'Usage: scripts/coverage.sh [gate|baseline] [--metal]' >&2
    exit 2
fi
case "$mode" in gate|baseline) ;; *)
    printf '%s\n' 'Usage: scripts/coverage.sh [gate|baseline] [--metal]' >&2
    exit 2 ;;
esac
# Each row names a report group, Cargo target, required count and exact tests.
# All invocations keep workspace feature unification, including the library
# selection: narrowing to one package can change instrumented dependency builds.
metal_groups='wgpu-lib|lib|5|aggregate::device::tests::metal_aggregate_readback_failure_retains_submitted_work lazy::transport_tests::metal_lazy_transport_reuse_preserves_round_truth lazy::transport_tests::metal_input_slack_preserves_exact_admission lazy::transport_tests::metal_lazy_transport_refusal_preserves_reuse lazy::transport_tests::metal_lazy_transport_cancelled_read_discards_capacity
tight|hardware_tight|4|metal_support_matches_exact_reduct_semantics metal_support_preserves_batch_isolation metal_support_refusals_preserve_reusable_residency metal_support_residency_tracks_theory_identity
formula|hardware_formula|2|metal_formula_limits_resize_identity_and_word_boundaries_remain_explicit metal_formula_queries_preserve_exact_frozen_semantics_and_residency
aggregate|hardware_aggregate|3|metal_aggregate_reductions_match_native_occurrences metal_aggregate_guards_preserve_numeric_boundaries metal_aggregate_exact_admission_preserves_cache_lifecycle
lazy|hardware_lazy|4|metal_lazy_worlds_match_exact_frozen_cpu_closures metal_lazy_growth_preserves_previous_round_truth metal_lazy_catalog_fits_when_static_carrier_refuses metal_source_selections_preserve_each_frozen_closure
cli-lazy|lazy_gpu|5|physical::ordinary_lazy_metal_preserves_complete_cpu_models physical::automatic_metal_keeps_lazy_grounding physical::requested_model_limit_retains_completed_lazy_candidates physical::lazy_source_stop_preserves_unfinished_candidate_counts physical::lazy_writer_failure_preserves_completed_device_work
cli-formula|formula_gpu|2|physical::ordinary_metal_formula_batches_match_complete_cpu_models_costs_and_displays physical::ordinary_metal_formula_limits_preserve_partial_coverage_and_writer_errors
world-views|world_views_gpu|2|metal_world_view_preserves_nonoptimal_answers metal_collection_limit_retains_checked_accounting
aggregate-measurement|aggregate_measurement|1|metal_aggregate_measurements_require_actual_submissions
relation|hardware_relation|2|metal_relation_masks_match_typed_rows metal_relation_refusals_preserve_prepared_view
relation-measurement|relation_measurement|1|metal_relation_measurement_keeps_complete_masks'

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
floor=$(scripts/maintenance.sh coverage-floor --mode "$mode" --path scripts/coverage-floor.txt)

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
    --rustc-version "$rust_version" --llvm-cov "$LLVM_COV" \
    --llvm-cov-version "$cov_version" --llvm-profdata "$LLVM_PROFDATA" \
    --llvm-profdata-version "$profdata_version"
if [ "$metal" = --metal ]; then
    set -- "$@" --metal-groups "$metal_groups"
fi
scripts/maintenance.sh "$@" > "$coverage_dir/toolchain.json"

write_report() {
    destination=$1
    shift
    mkdir -p -- "$destination"
    # In 0.8.7, report rejects build-feature flags despite listing them in help.
    # The separate instrumented directories retain each feature configuration.
    cargo +1.97.1 llvm-cov report "$@" --locked --json \
        --output-path "$destination/coverage.json"
    cargo +1.97.1 llvm-cov report "$@" --locked --html --output-dir "$destination"
}

run_metal_group() {
    group=$1
    target=$2
    expected=$3
    names=$4
    # Splitting is intentional: these are fixed libtest identifiers, not input.
    set -- $names
    if [ "$target" = lib ]; then
        set -- --lib --locked --no-report --no-clean -- --ignored --nocapture \
            --test-threads=1 --exact "$@"
    else
        set -- --test "$target" --locked --no-report --no-clean -- --ignored \
            --nocapture --test-threads=1 --exact "$@"
    fi
    physical_log="$coverage_dir/workspace/metal-$group.log"
    physical_status="$coverage_dir/workspace/metal-$group-status.txt"
    # Keep workspace feature unification and the existing instrumented target.
    # --no-clean retains the portable profile; no CLI-CPU data enters this stage.
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
        --group "$group" --table "$metal_groups"
    then
        return 1
    fi
    printf '%s\n' passed > "$physical_status"
}

run_metal() {
    printf '%s\n' incomplete > "$coverage_dir/workspace/metal-status.txt"
    # Reset every group before any execution, including groups an early failure
    # would leave unvisited. Old logs and status markers cannot certify this run.
    while IFS='|' read -r group target expected names; do
        printf '%s\n' incomplete > "$coverage_dir/workspace/metal-$group-status.txt"
        : > "$coverage_dir/workspace/metal-$group.log"
    done <<EOF
$metal_groups
EOF
    while IFS='|' read -r group target expected names; do
        run_metal_group "$group" "$target" "$expected" "$names"
    done <<EOF
$metal_groups
EOF
    printf '%s\n' passed > "$coverage_dir/workspace/metal-status.txt"
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
        if [ "$metal" = --metal ]; then
            write_report "$report_dir/portable"
            run_metal
        fi
        set --
    else
        cargo +1.97.1 llvm-cov "$@" --locked --no-report
        set -- --package zetesis-cli
    fi
    write_report "$report_dir" "$@"
}

run_profile workspace --all-features
run_profile cli-cpu --package zetesis-cli --no-default-features

export CARGO_LLVM_COV_TARGET_DIR="$coverage_dir/build-workspace"
if [ "$mode" = gate ]; then
    cargo +1.97.1 llvm-cov report --locked --fail-under-lines "$floor"
    export CARGO_LLVM_COV_TARGET_DIR="$coverage_dir/build-cli-cpu"
    cargo +1.97.1 llvm-cov report --package zetesis-cli --locked --fail-under-lines "$floor"
    printf '%s\n' gate-passed > "$coverage_dir/status.txt"
else
    printf '%s\n' 'baseline-complete (nongating)' > "$coverage_dir/status.txt"
fi
printf 'Coverage %s completed; reports: %s\n' "$mode" "$coverage_dir"
