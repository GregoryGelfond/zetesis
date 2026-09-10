#!/bin/sh
# Independent check layers; full target is allowed to fail while unsupported.
set -eu
mode=${1:-portable}
coverage_option=${2:-}
if [ "$#" -gt 2 ] || { [ "$#" -eq 2 ] && { [ "$mode" != coverage ] || [ "$coverage_option" != --metal ]; }; }; then
    printf '%s\n' 'Usage: scripts/check.sh [portable|coverage|oracle|proofs|book|full]; scripts/check.sh coverage --metal' >&2
    exit 2
fi
case "$mode" in portable|coverage|oracle|proofs|book|full) ;; *)
    printf '%s\n' 'Usage: scripts/check.sh [portable|coverage|oracle|proofs|book|full]; scripts/check.sh coverage --metal' >&2
    exit 2 ;;
esac
repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd -- "$repo_dir"
if [ "$mode" = portable ] || [ "$mode" = full ]; then
    cargo fmt --all -- --check
    cargo test --locked --workspace --all-features
    cargo test --locked -p zetesis-cli --no-default-features
    cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
    cargo clippy --locked -p zetesis-cli --no-default-features --all-targets -- -D warnings
    RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --all-features --no-deps
    # These maintained semantic experiments are independent Cargo workspaces.
    # Main-workspace checks cannot select them implicitly.
    for standalone_manifest in validation/reference/Cargo.toml experiments/gate-transfer/Cargo.toml refinement/membership/rust/Cargo.toml; do
        cargo fmt --manifest-path "$standalone_manifest" --all -- --check
        cargo test --manifest-path "$standalone_manifest" --locked --all-targets --all-features
        cargo test --manifest-path "$standalone_manifest" --locked --doc --all-features
        cargo clippy --manifest-path "$standalone_manifest" --locked --all-targets --all-features -- -D warnings
        RUSTDOCFLAGS="-D warnings" cargo doc --manifest-path "$standalone_manifest" --locked --all-features --no-deps
    done
    cargo bench --locked -p zetesis-experiments --bench oracles -- --test
    cargo bench --locked -p zetesis-cpu --bench lazy_joins -- --test
    cargo bench --locked -p zetesis-ferraris --bench native_aggregates -- --test
    cargo bench --locked -p zetesis-ferraris --bench frozen_reduct -- --test
fi
if [ "$mode" = book ] || [ "$mode" = full ]; then
    if [ "$(mdbook --version)" != 'mdbook v0.5.4' ]; then
        printf '%s\n' 'Documentation checks require mdBook 0.5.4.' >&2
        exit 2
    fi
    mdbook build
    # Keep one dependency configuration here so example crate lookup is unique.
    cargo build --locked -p zetesis-cli --lib --no-default-features --target-dir target/book-tests
    mdbook test --library-path target/book-tests/debug/deps
fi
if [ "$mode" = oracle ] || [ "$mode" = full ]; then
    cargo test --locked -p zetesis-themelios --test logical_bounds --test objective_priorities --test objective_priority_certificates --test objective_measure_carriers --test finite_chains --test affine_normalization -- --ignored --nocapture
    cargo test --locked -p zetesis-themelios --test extrema_alias_contracts --test boolean_element_contracts --test signed_element_contracts --test signed_choices -- --ignored --nocapture
    cargo test --locked -p zetesis-themelios --test program_parts --test conditional_consumers --test weighted_heads --test nonbinding_guards --test extrema_heads --test count_plans --test count_head_activity --test objective_forwarding --test boolean_heads --test evaluated_witnesses --test objective_literal_weights --test objective_extrema_presence -- --ignored --nocapture
    cargo test --locked -p zetesis-themelios --test aggregate_dependencies --test aggregate_consumers --test choice_consumers --test outer_negative_consumers --test outer_ranges --test negative_count_eligibility --test structured_witnesses -- --ignored --nocapture
    cargo test --locked -p zetesis-ferraris --test aggregate_clingo -- --ignored --nocapture
    cargo test --locked -p zetesis-ferraris --test extrema_clingo --test value_extrema -- --ignored --nocapture
    cargo test --locked -p zetesis-cli --test clingo --test extended_clingo --test multiple_inputs --test maximize --test language_value_sessions --test contribution_sessions --test bound_priority_sessions --test finite_carrier_sessions -- --ignored --nocapture
    cargo test --locked -p zetesis-themelios --test bundle_admission --test metadata --test formula --test formula_clingo --test aggregate_clingo --test aggregate_assignments_multiple --test aggregate_objective_observers --test extrema_source --test scalar_bindings_clingo --test objective_bounds_adversarial --test factorization_clingo --test comparison_reuse --test disjunction --test sum_profiles --test weak_objectives --test maximize_clingo --test observations --test observations_adversarial --test choice_intervals --test ground_guards --test conditional_body --test comparison_generators --test finite_bindings --test evaluated_heads --test negative_heads --test structural_values --test finite_pools --test true_heads --test count_heads --test value_extrema --test structural_bindings --test finite_values --test consequent_alternatives --test function_patterns --test positive_arguments --test scalar_evaluation -- --ignored --nocapture
fi
if [ "$mode" = proofs ] || [ "$mode" = full ]; then
    (cd proofs && lake build && lake env lean -DautoImplicit=false -DwarningAsError=true Audit.lean)
    scripts/maintenance.sh proof-record
fi
if [ "$mode" = coverage ] || [ "$mode" = full ]; then
    if [ "$coverage_option" = --metal ]; then
        ./scripts/coverage.sh gate --metal
    else
        ./scripts/coverage.sh
    fi
fi
if [ "$mode" = full ]; then
    mkdir -p -- "$repo_dir/target"
    ./scripts/validate.sh --report "$repo_dir/target/full-compatibility.json"
fi
