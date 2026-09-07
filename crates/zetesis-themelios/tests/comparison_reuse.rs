//! A comparison certificate belongs to one completed nongenerative binding.

#[path = "support/source_records.rs"]
mod source_records;

use themelios_program::term::EvalError;
use zetesis_themelios::{ExpansionFailure, FormulaFailure, FormulaLimits, FormulaResource};

const FIXTURE: &str = include_str!("fixtures/comparison-reuse.jsonl");

#[test]
fn complete_models_preserve_backtracking_generators_and_residual_filters() {
    let cases = source_records::cases(FIXTURE);
    assert_eq!(cases.len(), 20);
    assert_eq!(
        cases.iter().map(|case| case.records.len()).sum::<usize>(),
        35
    );
    for case in cases {
        let input = source_records::admit(&case.source, FormulaLimits::default())
            .unwrap_or_else(|error| panic!("{}: {error:?}", case.name));
        assert_eq!(
            source_records::exhaustive(&input),
            case.records,
            "{}",
            case.name
        );
    }
}

#[test]
fn later_success_never_certifies_deferred_undefined_arithmetic() {
    for source in [
        "d(0).p:-d(X),1/X=0,X=0,X+0=0.",
        "d(0).p:-d(X),X=0,1/X=0,X+0=0.",
        "d(1;2).p(X):-d(X),1/(2-X)>=0,X>0.",
        "d(1).p(Y):-d(X),X=1,Y=0..1,1/Y=0.",
    ] {
        let error = source_records::admit(source, FormulaLimits::default())
            .expect_err("a complete row still evaluates every inconclusive comparison");
        assert!(
            matches!(
                error,
                FormulaFailure::Expansion(ExpansionFailure::Evaluation {
                    error: EvalError::Undefined,
                    ..
                })
            ),
            "{source}: {error:?}"
        );
    }
}

#[test]
fn reused_comparisons_keep_the_work_ceiling_inclusive() {
    let source = "d(1;2;3).p(X,Y):-d(X),d(Y),X+Y=4,X!=Y.";
    let admitted = |maximum| {
        source_records::admit(
            source,
            FormulaLimits {
                max_work: maximum,
                ..FormulaLimits::default()
            },
        )
    };
    let mut lower = 0;
    let mut upper = 1_024;
    assert!(admitted(upper).is_ok());
    while lower + 1 < upper {
        let middle = lower + (upper - lower) / 2;
        if admitted(middle).is_ok() {
            upper = middle;
        } else {
            lower = middle;
        }
    }
    assert!(admitted(upper).is_ok());
    for maximum in [0, lower] {
        assert!(matches!(
            admitted(maximum),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Work,
                limit,
                observed,
                ..
            }) if limit == u128::from(maximum) && observed == limit + 1
        ));
    }
}

#[test]
#[ignore = "requires the independent clingo executable on PATH"]
fn unchanged_comparison_sources_match_fresh_clingo() {
    for case in source_records::cases(FIXTURE) {
        assert_eq!(
            source_records::clingo(&case.source),
            case.records,
            "{}",
            case.name
        );
    }
}
