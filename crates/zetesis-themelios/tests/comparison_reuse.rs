//! A comparison certificate belongs to one completed nongenerative binding.

#[path = "support/source_records.rs"]
mod source_records;
#[path = "support/source_cases.rs"]
mod source_cases;
#[path = "support/source_oracle.rs"]
mod source_oracle;

use themelios_program::term::EvalError;
use zetesis_themelios::{ExpansionFailure, FormulaFailure, FormulaLimits, FormulaResource};

const FIXTURE: &str = include_str!("fixtures/comparison-reuse.jsonl");
const FALSE_FILTER_SOURCE: &str = "d(0).p:-d(X),X!=0,1/X>0.";

#[test]
fn complete_models_preserve_backtracking_generators_and_residual_filters() {
    let cases = source_cases::cases(FIXTURE);
    assert_eq!(cases.len(), 20);
    assert_eq!(
        cases.iter().map(|case| case.records.len()).sum::<usize>(),
        35
    );
    let admitted: Vec<_> = cases
        .into_iter()
        .filter(|case| case.name != "earlier_false_discards_invalid")
        .collect();
    assert_eq!(admitted.len(), 19);
    assert_eq!(
        admitted
            .iter()
            .map(|case| case.records.len())
            .sum::<usize>(),
        34
    );
    for case in admitted {
        let input = source_records::admit(&case.source, &FormulaLimits::default())
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
fn a_false_filter_preserves_required_undefined_arithmetic() {
    let cases = source_cases::cases(FIXTURE);
    let refused: Vec<_> = cases
        .iter()
        .filter(|case| case.name == "earlier_false_discards_invalid")
        .collect();
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0].source, FALSE_FILTER_SOURCE);
    // X=0 is a complete positive binding. Clingo's successful record remains
    // in the unchanged fixture, but native admission validates the later 1/X.
    let error = source_records::admit(FALSE_FILTER_SOURCE, &FormulaLimits::default())
        .expect_err("the earlier false comparison cannot discard a required error");
    let FormulaFailure::Expansion(ExpansionFailure::Evaluation {
        error: EvalError::Undefined,
        location,
    }) = error
    else {
        panic!("expected located undefined arithmetic: {error:?}");
    };
    assert_eq!(location.source, themelios_base::source::SourceId::new(0));
    assert_eq!(location.span.start().get(), 5);
    assert_eq!(
        usize::try_from(location.span.end().get()).unwrap(),
        FALSE_FILTER_SOURCE.len()
    );
}

#[test]
fn later_success_never_certifies_deferred_undefined_arithmetic() {
    for source in [
        "d(0).p:-d(X),1/X=0,X=0,X+0=0.",
        "d(0).p:-d(X),X=0,1/X=0,X+0=0.",
        "d(1;2).p(X):-d(X),1/(2-X)>=0,X>0.",
        "d(1).p(Y):-d(X),X=1,Y=0..1,1/Y=0.",
    ] {
        let error = source_records::admit(source, &FormulaLimits::default())
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
            &FormulaLimits {
                max_work: maximum,
                ..FormulaLimits::default()
            },
        )
    };
    // This test locates an inclusive work boundary, not a fixed cost target.
    // Start from an actually admitted endpoint under the public finite policy;
    // checked lookup operations may change the amount of work this source needs.
    let mut upper = FormulaLimits::default().max_work;
    let complete = admitted(upper).expect("finite comparison fixture is admitted");
    let expected = source_records::Records::from([(
        ["d(1)", "d(2)", "d(3)", "p(1,3)", "p(3,1)"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        None,
    )]);
    assert_eq!(source_records::exhaustive(&complete), expected);
    let has_enough_work = |maximum| match admitted(maximum) {
        Ok(_) => true,
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Work,
            limit,
            observed,
            ..
        }) if limit == u128::from(maximum) && observed > limit => false,
        Err(error) => panic!("expected only a work refusal at {maximum}: {error:?}"),
    };
    let mut lower = 0;
    while lower + 1 < upper {
        let middle = lower + (upper - lower) / 2;
        if has_enough_work(middle) {
            upper = middle;
        } else {
            lower = middle;
        }
    }
    assert_eq!(upper, lower + 1);
    let exact = admitted(upper).expect("the exact work ceiling is inclusive");
    assert_eq!(source_records::exhaustive(&exact), expected);
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
fn original_comparison_sources_retain_fresh_clingo_records() {
    for case in source_cases::cases(FIXTURE) {
        assert_eq!(
            source_oracle::records(&case.source),
            case.records,
            "{}",
            case.name
        );
    }
}
