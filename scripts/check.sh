#!/bin/sh
# Independent validation layers; full runs every layer in sequence.
set -eu
mode=${1:-portable}
coverage_option=${2:-}
usage='Usage: scripts/check.sh [portable|coverage|oracle|proofs|book|hardware|full]; scripts/check.sh coverage [--metal|--vulkan]; scripts/check.sh hardware [--metal|--vulkan]'
if [ "$#" -gt 2 ]; then
    printf '%s\n' "$usage" >&2
    exit 2
fi
if [ "$#" -eq 2 ]; then
    case "$mode $coverage_option" in
        'coverage --metal'|'coverage --vulkan'|'hardware --metal'|'hardware --vulkan') ;;
        *)
            printf '%s\n' "$usage" >&2
            exit 2 ;;
    esac
fi
case "$mode" in portable|coverage|oracle|proofs|book|hardware|full) ;; *)
    printf '%s\n' "$usage" >&2
    exit 2 ;;
esac
repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd -- "$repo_dir"
if [ "$mode" = portable ] || [ "$mode" = full ]; then
    # Reject known temporary profiling markers in maintained crate Rust sources.
    # This literal check complements review; it does not recognize every probe.
    marker_status=0
    grep -rniE 'ZETESIS_PROBE|quickxplain_probe|PROBE_LEARN' crates --include='*.rs' || marker_status=$?
    case "$marker_status" in
        0)
            printf 'measurement scaffolding must not ship\n' >&2
            exit 1 ;;
        1) ;;
        *)
            printf 'could not complete the profiling-marker check\n' >&2
            exit "$marker_status" ;;
    esac
    cargo fmt --all -- --check
    cargo test --locked --workspace --all-features --no-fail-fast
    cargo test --locked -p zetesis-cli -p zetesis-solve -p zetesis-engine -p zetesis --no-default-features --no-fail-fast
    cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
    cargo clippy --locked -p zetesis-cli -p zetesis-solve -p zetesis-engine -p zetesis --no-default-features --all-targets -- -D warnings
    RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --all-features --no-deps
    # The maintained standalone packages are independent Cargo workspaces.
    # Main-workspace checks cannot select them implicitly.
    for standalone_manifest in validation/reference/Cargo.toml refinement/membership/rust/Cargo.toml; do
        cargo fmt --manifest-path "$standalone_manifest" --all -- --check
        cargo test --manifest-path "$standalone_manifest" --locked --all-targets --all-features --no-fail-fast
        cargo test --manifest-path "$standalone_manifest" --locked --doc --all-features --no-fail-fast
        cargo clippy --manifest-path "$standalone_manifest" --locked --all-targets --all-features -- -D warnings
        RUSTDOCFLAGS="-D warnings" cargo doc --manifest-path "$standalone_manifest" --locked --all-features --no-deps
    done
    cargo bench --locked -p zetesis-cpu --bench oracles -- --test
    cargo bench --locked -p zetesis-cpu --bench lazy_joins -- --test
    cargo bench --locked -p zetesis-ferraris --bench native_aggregates -- --test
    cargo bench --locked -p zetesis-ferraris --bench frozen_reduct -- --test
    cargo bench --locked -p zetesis-ferraris --bench membership -- --test
    cargo bench --locked -p zetesis-ferraris --bench admission -- --test
    cargo bench --locked -p zetesis-ferraris --bench interpretation -- --test
    cargo bench --locked -p zetesis-ferraris --bench tight_admission -- --test
fi
if [ "$mode" = book ] || [ "$mode" = full ]; then
    # mdBook invokes rustdoc from a temporary directory outside this checkout.
    # Keep the same toolchain that Cargo selects here for its dependencies.
    book_toolchain=$(rustup show active-toolchain)
    RUSTUP_TOOLCHAIN=${book_toolchain%% *}
    export RUSTUP_TOOLCHAIN
    if [ "$(mdbook --version)" != 'mdbook v0.5.4' ]; then
        printf '%s\n' 'Documentation checks require mdBook 0.5.4.' >&2
        exit 2
    fi
    mdbook build
    # Reuse this checkout's ordinary Cargo target; expose only current libraries.
    mkdir -p -- target/book-views
    book_view=$(mktemp -d "$repo_dir/target/book-views/run.XXXXXXXX")
    (
        # Only this freshly created view is removed; Cargo artifacts are retained.
        trap 'rm -rf -- "$book_view/libraries"' 0
        cargo build --locked -p zetesis -p zetesis-cli -p zetesis-solve -p zetesis-validation --lib --all-features --target-dir target --message-format=json-render-diagnostics > "$book_view/artifacts.jsonl"
        scripts/maintenance.sh book-libraries --messages "$book_view/artifacts.jsonl" \
            --build-directory target --destination "$book_view/libraries" \
            --crate zetesis --crate zetesis_cli --crate zetesis_solve --crate zetesis_validation
        mdbook test --library-path "$book_view/libraries"
    )
fi
if [ "$mode" = oracle ] || [ "$mode" = full ]; then
    case "${CLINGO:-}" in /*) ;; *)
        printf '%s\n' 'Oracle checks require CLINGO to name an absolute executable path.' >&2
        exit 2 ;;
    esac
    oracle_path=$(command -v clingo) || {
        printf '%s\n' 'Oracle checks require clingo on PATH.' >&2
        exit 2
    }
    if [ ! -f "$CLINGO" ] || [ ! -x "$CLINGO" ] || [ ! "$CLINGO" -ef "$oracle_path" ]; then
        printf '%s\n' 'CLINGO and PATH clingo must select the same executable file.' >&2
        exit 2
    fi
    oracle_version=$("$CLINGO" --version) || {
        printf '%s\n' 'Cannot observe the clingo version; no oracle campaign started.' >&2
        exit 2
    }
    if [ "$(printf '%s\n' "$oracle_version" | sed -n '1p')" != 'clingo version 5.8.2' ]; then
        printf 'Oracle checks require clingo 5.8.2; found %s\n' "$oracle_version" >&2
        exit 2
    fi
    mkdir -p -- target/oracle-checks
    oracle_records=$(mktemp -d "$repo_dir/target/oracle-checks/run.XXXXXXXX")
    printf '%s\n' "$CLINGO" > "$oracle_records/executable.txt"
    printf '%s\n' "$oracle_version" > "$oracle_records/version.txt"
    printf '%s\n' incomplete > "$oracle_records/status.txt"
    oracle_index=0
    oracle_first_failure=0
    oracle_test() {
        oracle_index=$((oracle_index + 1))
        printf '%s\n' cargo test "$@" > "$oracle_records/$oracle_index.argv"
        # Capture each test's prints until its harness result is complete,
        # then show all output. This retains raw reference evidence without
        # interleaving it into the exact test-name records checked below.
        {
            if cargo test "$@"; then
                printf '%s\n' 0 > "$oracle_records/$oracle_index.exit"
            else
                printf '%s\n' "$?" > "$oracle_records/$oracle_index.exit"
            fi
        } | tee "$oracle_records/$oracle_index.log"
        oracle_exit=$(cat "$oracle_records/$oracle_index.exit")
        printf 'Oracle campaign %s exited %s\n' "$oracle_index" "$oracle_exit"
        if [ "$oracle_first_failure" -eq 0 ] && [ "$oracle_exit" -ne 0 ]; then
            oracle_first_failure=$oracle_exit
        fi
    }
    oracle_test --locked --no-fail-fast -p zetesis-themelios --test integration -- --ignored --show-output arithmetic_validation:: support_delta:: extremal_terms:: observation_bindings:: objective_rich_cycles:: objective_pools:: conditional_heads:: keyed_constraints:: fixed_constraints:: strong_negation::
    oracle_test --locked --no-fail-fast -p zetesis-solve --no-default-features --test integration language_consumers::original_sources_retain_declared_reference_results -- --ignored --show-output
    oracle_test --locked --no-fail-fast -p zetesis-solve --no-default-features --test integration -- --ignored --show-output projected_reference::
    oracle_test --locked --no-fail-fast -p zetesis-validation --test integration -- --ignored --show-output example_parity:: authored_examples::
    oracle_test --locked --no-fail-fast -p zetesis-themelios --test integration -- --ignored --show-output objective_boundaries:: objective_dependency_contracts::
    oracle_test --locked --no-fail-fast -p zetesis-themelios --test integration -- --ignored --show-output objective_scopes:: objective_carrier_composition:: objective_language_boundaries::
    oracle_test --locked --no-fail-fast -p zetesis-themelios --test integration -- --ignored --show-output head_contributions:: objective_source_completion:: objective_field_expressions:: objective_priority_reporting:: objective_cyclic_producers:: objective_rich_producers:: observation_expressions:: observation_scopes:: observation_families::
    oracle_test --locked --no-fail-fast -p zetesis-themelios --test integration -- --ignored --show-output logical_bounds:: objective_priorities:: objective_priority_certificates:: objective_measure_carriers:: finite_chains:: affine_normalization::
    oracle_test --locked --no-fail-fast -p zetesis-themelios --test integration -- --ignored --show-output extrema_alias_contracts:: boolean_element_contracts:: signed_element_contracts:: signed_choices::
    oracle_test --locked --no-fail-fast -p zetesis-themelios --test integration -- --ignored --show-output program_parts:: conditional_consumers:: weighted_heads:: nonbinding_guards:: extrema_heads:: count_plans:: count_head_activity:: objective_forwarding:: boolean_heads:: evaluated_witnesses:: objective_literal_weights:: objective_extrema_presence::
    oracle_test --locked --no-fail-fast -p zetesis-themelios --test integration -- --ignored --show-output aggregate_dependencies:: aggregate_consumers:: choice_consumers:: outer_negative_consumers:: outer_ranges:: negative_count_eligibility:: structured_witnesses::
    oracle_test --locked --no-fail-fast -p zetesis-ferraris --test integration -- --ignored --show-output aggregate_clingo::
    oracle_test --locked --no-fail-fast -p zetesis-ferraris --test integration -- --ignored --show-output extrema_clingo::
    oracle_test --locked --no-fail-fast -p zetesis-cli --test integration -- --ignored --show-output clingo:: extended_clingo:: multiple_inputs:: maximize:: language_value_sessions:: contribution_sessions:: bound_priority_sessions:: finite_carrier_sessions:: count_objective_sessions:: strong_negation::
    oracle_test --locked --no-fail-fast -p zetesis-themelios --test integration -- --ignored --show-output bundle_admission:: metadata:: formula:: formula_clingo:: aggregate_clingo:: aggregate_assignments_multiple:: aggregate_objective_observers:: extrema_source:: scalar_bindings_clingo:: objective_bounds_adversarial:: factorization_clingo:: comparison_reuse:: disjunction:: sum_profiles:: weak_objectives:: maximize_clingo:: observations:: observations_adversarial:: choice_intervals:: ground_guards:: conditional_body:: comparison_generators:: finite_bindings:: evaluated_heads:: negative_heads:: structural_values:: finite_pools:: true_heads:: count_heads:: value_extrema:: structural_bindings:: finite_values:: consequent_alternatives:: function_patterns:: positive_arguments:: scalar_evaluation::
    # Each campaign ran at least one test, and exactly the ignored tests its
    # filters select among the sources: a filter matching nothing, or a
    # selected test the campaign's features compile out, fails the gate.
    if scripts/maintenance.sh oracle-runs --root . --records "$oracle_records"; then
        oracle_runs_exit=0
    else
        oracle_runs_exit=$?
    fi
    printf '%s\n' "$oracle_runs_exit" > "$oracle_records/runs.exit"
    if [ "$oracle_first_failure" -eq 0 ] && [ "$oracle_runs_exit" -ne 0 ]; then
        oracle_first_failure=$oracle_runs_exit
    fi
    printf 'Oracle campaign records: %s\n' "$oracle_records"
    if [ "$oracle_first_failure" -ne 0 ]; then
        printf '%s\n' failed > "$oracle_records/status.txt"
        exit "$oracle_first_failure"
    fi
    printf '%s\n' passed > "$oracle_records/status.txt"
fi
if [ "$mode" = proofs ] || [ "$mode" = full ]; then
    mkdir -p -- target/proof-checks
    proof_check=$(mktemp -d "$repo_dir/target/proof-checks/run.XXXXXXXX")
    : > "$proof_check/audit.stdout"
    : > "$proof_check/audit.stderr"
    proof_exit=0
    if (cd proofs && lake build && lake env lean -DautoImplicit=false -DwarningAsError=true Audit.lean > "$proof_check/audit.stdout" 2> "$proof_check/audit.stderr"); then
        :
    else
        proof_exit=$?
    fi
    printf '%s\n' "$proof_exit" > "$proof_check/exit.txt"
    cat "$proof_check/audit.stdout" "$proof_check/audit.stderr"
    if [ "$proof_exit" -ne 0 ]; then
        exit "$proof_exit"
    fi
    if [ -s "$proof_check/audit.stderr" ]; then
        printf '%s\n' 'Strict Audit emitted stderr; its output cannot qualify.' >&2
        exit 2
    fi
    scripts/maintenance.sh proof-record --live-audit "$proof_check/audit.stdout"
fi
if [ "$mode" = coverage ] || [ "$mode" = full ]; then
    # A physical stage only when a backend is named.
    if [ -n "$coverage_option" ]; then
        ./scripts/coverage.sh gate "$coverage_option"
    else
        ./scripts/coverage.sh
    fi
fi
if [ "$mode" = hardware ]; then
    # The host's physical backend by default: Metal on macOS, Vulkan elsewhere.
    if [ -n "$coverage_option" ]; then
        ./scripts/hardware.sh "$coverage_option"
    else
        ./scripts/hardware.sh
    fi
fi
if [ "$mode" = full ]; then
    mkdir -p -- "$repo_dir/target"
    ./scripts/validate.sh --report "$repo_dir/target/full-compatibility.json"
fi
