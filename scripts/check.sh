#!/bin/sh
# Independent check layers; full target is allowed to fail while unsupported.
set -eu
mode=${1:-portable}
if [ "$#" -gt 1 ]; then
    printf '%s\n' 'Usage: scripts/check.sh [portable|coverage|oracle|proofs|full]' >&2
    exit 2
fi
case "$mode" in portable|coverage|oracle|proofs|full) ;; *)
    printf '%s\n' 'Usage: scripts/check.sh [portable|coverage|oracle|proofs|full]' >&2
    exit 2 ;;
esac
repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd -- "$repo_dir"
if [ "$mode" = portable ] || [ "$mode" = full ]; then
    python3 -m unittest discover -s scripts/tests -v
    cargo fmt --all -- --check
    cargo test --locked --workspace --all-features
    cargo test --locked -p zetesis-cli --no-default-features
    cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
    cargo clippy --locked -p zetesis-cli --no-default-features --all-targets -- -D warnings
    RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --all-features --no-deps
    cargo bench --locked -p zetesis-experiments --bench oracles -- --test
    cargo bench --locked -p zetesis-cpu --bench lazy_joins -- --test
fi
if [ "$mode" = oracle ] || [ "$mode" = full ]; then
    cargo test --locked -p zetesis-themelios --test aggregate_consumers --test structured_witnesses -- --ignored --nocapture
    cargo test --locked -p zetesis-ferraris --test aggregate_clingo -- --ignored --nocapture
    cargo test --locked -p zetesis-ferraris --test extrema_clingo --test value_extrema -- --ignored --nocapture
    cargo test --locked -p zetesis-cli --test clingo --test extended_clingo --test multiple_inputs --test maximize -- --ignored --nocapture
    cargo test --locked -p zetesis-themelios --test bundle_admission --test metadata --test formula --test formula_clingo --test aggregate_clingo --test aggregate_assignments_multiple --test aggregate_objective_observers --test extrema_source --test scalar_bindings_clingo --test objective_bounds_adversarial --test factorization_clingo --test comparison_reuse --test disjunction --test sum_profiles --test weak_objectives --test maximize_clingo --test observations --test observations_adversarial --test choice_intervals --test ground_guards --test conditional_body --test comparison_generators --test finite_bindings --test evaluated_heads --test negative_heads --test structural_values --test finite_pools --test true_heads --test count_heads --test value_extrema --test structural_bindings --test finite_values --test consequent_alternatives --test function_patterns --test positive_arguments --test scalar_evaluation -- --ignored --nocapture
fi
if [ "$mode" = proofs ] || [ "$mode" = full ]; then
    (cd proofs && lake build && lake env lean -DautoImplicit=false -DwarningAsError=true Audit.lean)
    python3 scripts/proof_record.py
fi
if [ "$mode" = coverage ] || [ "$mode" = full ]; then
    ./scripts/coverage.sh
fi
if [ "$mode" = full ]; then
    mkdir -p -- "$repo_dir/target"
    ./scripts/validate.sh --report "$repo_dir/target/full-compatibility.json"
fi
