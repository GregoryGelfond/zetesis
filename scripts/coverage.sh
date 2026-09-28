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
metal_groups='wgpu-lib|lib|14|aggregate::device::tests::metal_aggregate_readback_failure_retains_submitted_work lazy::transport_tests::metal_lazy_transport_reuse_preserves_round_truth lazy::transport_tests::metal_input_slack_preserves_exact_admission lazy::transport_tests::metal_lazy_transport_refusal_preserves_reuse lazy::transport_tests::metal_lazy_transport_cancelled_read_discards_capacity formula::device::tests::metal_busy_refusal_preserves_formula_state aggregate::device::tests::metal_readback_failure_invalidates_context_peers formula::device::tests::metal_profile_starts_fresh_formula_oracles formula::device::tests::metal_profiles_identify_exact_compilations formula::device::tests::metal_profile_reuse_checks_context_lifecycle relation::device::tests::metal_interrupted_preparation_preserves_context context::tests::control_tests::metal_controlled_calls_preserve_stop_identity formula::device::tests::metal_formula_submission_receipt_survives_interruption lazy::transport_tests::metal_lazy_uploads_reuse_only_current_batch_inputs
tight|integration|4|hardware_tight::metal_support_matches_exact_reduct_semantics hardware_tight::metal_support_preserves_batch_isolation hardware_tight::metal_support_refusals_preserve_reusable_residency hardware_tight::metal_support_residency_tracks_theory_identity
formula|integration|2|hardware_formula::metal_formula_limits_resize_identity_and_word_boundaries_remain_explicit hardware_formula::metal_formula_queries_preserve_exact_frozen_semantics_and_residency
aggregate|integration|3|hardware_aggregate::metal_aggregate_reductions_match_native_occurrences hardware_aggregate::metal_aggregate_guards_preserve_numeric_boundaries hardware_aggregate::metal_aggregate_exact_admission_preserves_cache_lifecycle
lazy|integration|4|hardware_lazy::metal_lazy_worlds_match_exact_frozen_cpu_closures hardware_lazy::metal_lazy_growth_preserves_previous_round_truth hardware_lazy::metal_lazy_catalog_fits_when_static_carrier_refuses hardware_lazy::metal_source_selections_preserve_each_frozen_closure
cli-lazy|lazy_gpu|5|physical::ordinary_lazy_metal_preserves_complete_cpu_models physical::metal_automatic_grounder_keeps_source_joins physical::requested_model_limit_retains_completed_lazy_candidates physical::lazy_source_stop_preserves_unfinished_candidate_counts physical::lazy_writer_failure_preserves_completed_device_work
cli-formula|formula_gpu|3|physical::ordinary_metal_formula_batches_match_complete_cpu_models_costs_and_displays physical::ordinary_metal_formula_limits_preserve_partial_coverage_and_writer_errors physical::ordinary_metal_table_joins_preserve_complete_answers
world-views|integration|4|world_views_gpu::metal_world_view_preserves_nonoptimal_answers world_views_gpu::metal_collection_limit_retains_checked_accounting world_views_gpu::metal_collection_refuses_a_foreign_context world_views_gpu::metal_resources_leave_a_cpu_collection_on_the_cpu
aggregate-measurement|aggregate_measurement|1|metal_aggregate_measurements_require_actual_submissions
relation|integration|2|hardware_relation::metal_relation_masks_match_typed_rows hardware_relation::metal_relation_refusals_preserve_prepared_view
relation-measurement|relation_measurement|1|metal_relation_measurement_keeps_complete_masks
context|integration|1|hardware_context::metal_formula_executes_while_relation_columns_remain_prepared
solve-context|lib|3|engine::resource_tests::metal_closure_retains_the_supplied_context formula_execution::tests::resource_tests::metal_formula_retains_the_supplied_context formula_execution::tests::resource_tests::metal_formula_sessions_reuse_the_supplied_profile
session-resources|integration|9|session_resources_gpu::metal_resources_preserve_independent_sessions session_resources_gpu::metal_resource_policy_refusal_preserves_reuse session_resources_gpu::metal_resources_preserve_cpu_policies session_resources_gpu::metal_observer_failure_preserves_resource_reuse session_resources_gpu::metal_formula_profiles_preserve_independent_sessions session_resources_gpu::metal_tight_sessions_preserve_complete_families session_resources_gpu::metal_general_formulas_keep_device_execution session_resources_gpu::metal_tight_refusal_preserves_pending_coverage session_resources_gpu::metal_terminal_sessions_preserve_complete_families
language-consumers|integration|2|language_consumers::physical::metal_families_retain_scored_observations language_consumers::physical::metal_optimum_ties_retain_full_answers
static|integration|2|hardware::metal_constructor_executes_resident_batches_without_fallback hardware::metal_static_oracle_matches_independent_closures'

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
    if cargo +1.97.1 llvm-cov report --locked --fail-under-lines "$floor"; then
        :
    else
        workspace_floor_exit=$?
    fi
    printf 'workspace\t%s\n' "$workspace_floor_exit" >> "$coverage_dir/floors.tsv"
    export CARGO_LLVM_COV_TARGET_DIR="$coverage_dir/build-cli-cpu"
    cpu_floor_exit=0
    if cargo +1.97.1 llvm-cov report --package zetesis-cli --package zetesis-solve --locked --fail-under-lines "$floor"; then
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
