//! Numeric min/max source translation, generated sentinels and total observers.
use crate::support::source_cases;
use crate::support::source_oracle;
use crate::support::source_records;
use source_oracle::records as clingo;
use source_records::{admit, exhaustive};
use zetesis_core::Value;
use zetesis_themelios::FormulaLimits;

fn cases() -> Vec<source_cases::Case> {
    source_cases::cases(include_str!("../fixtures/extrema-source.jsonl"))
}
#[test]
fn numeric_extrema_preserve_complete_models_and_every_objective_cost() {
    let cases = cases();
    assert_eq!(cases.len(), 198);
    assert_eq!(
        cases.iter().map(|case| case.records.len()).sum::<usize>(),
        654
    );
    for case in cases {
        let input = admit(&case.source, &FormulaLimits::default())
            .unwrap_or_else(|error| panic!("{}: {error}", case.name));
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
fn empty_assignments_use_real_extrema_and_respect_assignment_cache_limits() {
    for (function, value) in [("min", Value::Supremum), ("max", Value::Infimum)] {
        let source = format!("m(M) :- M = #{function}{{}}.");
        let input = admit(&source, &FormulaLimits::default()).unwrap();
        assert_eq!(input.atoms().len(), 1);
        assert_eq!(
            input
                .atoms()
                .at(0)
                .unwrap()
                .values()
                .iter()
                .collect::<Vec<_>>(),
            &[value]
        );
        for limits in [
            FormulaLimits {
                max_assignment_values: 0,
                ..FormulaLimits::default()
            },
            FormulaLimits {
                max_aggregate_cache_rows: 0,
                ..FormulaLimits::default()
            },
            FormulaLimits {
                max_aggregate_cache_roots: 0,
                ..FormulaLimits::default()
            },
        ] {
            assert!(
                !admit(&source, &limits)
                    .unwrap_err()
                    .diagnostics()
                    .is_empty()
            );
        }
    }
}
#[test]
fn unresolved_numeric_endpoints_remain_located_refusals() {
    for source in [
        "p :- #min {2147483647,k:not p} != 2147483647.",
        "p :- #max {-2147483648,k:not p} != -2147483648.",
        "m(M) :- M = #max {2147483647,k}.",
    ] {
        assert!(
            !admit(source, &FormulaLimits::default())
                .unwrap_err()
                .diagnostics()
                .is_empty()
        );
    }
}

#[test]
#[ignore = "requires independent clingo; 198 bounded complete source comparisons"]
fn recorded_extrema_sources_match_fresh_clingo() {
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
