//! Positive sums and ignored tuple contributions retain original reduct semantics.
#[path = "support/source_records.rs"]
mod source_records;

use source_records::{admit, clingo, exhaustive};
use zetesis_themelios::FormulaLimits;

fn cases() -> Vec<source_records::Case> {
    source_records::cases(include_str!("fixtures/sum-profiles.jsonl"))
}

#[test]
fn sum_profiles_preserve_complete_models_and_every_objective_cost() {
    let cases = cases();
    assert_eq!(cases.len(), 274);
    assert_eq!(
        cases.iter().map(|case| case.records.len()).sum::<usize>(),
        1_018
    );
    for case in cases {
        let input = admit(&case.source, FormulaLimits::default())
            .unwrap_or_else(|error| panic!("{}: {error}\n{}", case.name, case.source));
        assert_eq!(
            exhaustive(&input),
            case.records,
            "{}: {}",
            case.name,
            case.source
        );
    }
}

#[test]
fn ignored_weights_do_not_relax_scope_or_resource_checks() {
    for source in [
        "n(N) :- N = #sum+ { -2,X }.",
        "n(N) :- N = #sum { foo,X }.",
        "n(N) :- N = #sum+ { 2,X : p(Y) }.",
    ] {
        assert!(
            !admit(source, FormulaLimits::default())
                .unwrap_err()
                .diagnostics()
                .is_empty()
        );
    }
    for function in ["sum", "sum+"] {
        let source = format!("n(N) :- N = #{function} {{ foo,k; -2,j; 3,l }}.");
        for limits in [
            FormulaLimits {
                max_assignment_values: 0,
                ..Default::default()
            },
            FormulaLimits {
                max_aggregate_cache_rows: 0,
                ..Default::default()
            },
            FormulaLimits {
                max_aggregate_cache_roots: 0,
                ..Default::default()
            },
            FormulaLimits {
                max_work: 0,
                ..Default::default()
            },
        ] {
            assert!(!admit(&source, limits).unwrap_err().diagnostics().is_empty());
        }
    }
}

#[test]
#[ignore = "requires independent clingo; 274 bounded complete source comparisons"]
fn recorded_sum_profiles_match_fresh_clingo() {
    for case in cases() {
        assert_eq!(
            clingo(&case.source),
            case.records,
            "{}: {}",
            case.name,
            case.source
        );
    }
}
