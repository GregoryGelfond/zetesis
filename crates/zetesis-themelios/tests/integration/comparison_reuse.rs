//! A comparison certificate belongs to one completed nongenerative binding.

use crate::support::source_cases;

use themelios_program::term::EvalError;
use zetesis_clingo_support as oracle;
use zetesis_reference_support as reference;
use zetesis_test_support::records::Records;
use zetesis_themelios::{ExpansionFailure, FormulaFailure, FormulaLimits, FormulaResource};

const FIXTURE: &str = include_str!("../fixtures/comparison-reuse.jsonl");
const FALSE_FILTER_SOURCE: &str = "d(0).p:-d(X),X!=0,1/X>0.";

#[test]
fn complete_models_preserve_backtracking_generators_and_residual_filters() {
    let cases = source_cases::cases(FIXTURE);
    assert_eq!(cases.len(), 20);
    assert_eq!(
        cases.iter().map(|case| case.records.len()).sum::<usize>(),
        35
    );
    for case in cases {
        let input = reference::admit(&case.source, &FormulaLimits::default())
            .unwrap_or_else(|error| panic!("{}: {error:?}", case.name));
        assert_eq!(reference::exhaustive(&input), case.records, "{}", case.name);
    }
}

#[test]
fn an_earlier_false_comparison_excludes_the_substitution() {
    let cases = source_cases::cases(FIXTURE);
    let excluded: Vec<_> = cases
        .iter()
        .filter(|case| case.name == "earlier_false_discards_invalid")
        .collect();
    assert_eq!(excluded.len(), 1);
    assert_eq!(excluded[0].source, FALSE_FILTER_SOURCE);
    // X != 0 excludes the substitution X = 0, so 1/X is never reached and the
    // native family is the reference's.
    let input = reference::admit(FALSE_FILTER_SOURCE, &FormulaLimits::default())
        .expect("the excluded substitution reaches no operation");
    assert_eq!(reference::exhaustive(&input), excluded[0].records);
}

#[test]
fn comparisons_cannot_admit_an_entirely_undefined_family() {
    for source in [
        "d(0).p:-d(X),1/X=0,X=0,X+0=0.",
        "d(0).p:-d(X),X=0,1/X=0,X+0=0.",
    ] {
        let error = reference::admit(source, &FormulaLimits::default())
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
fn mixed_comparison_families_preserve_defined_substitutions() {
    for (source, expected) in [
        ("d(1;2).p(X):-d(X),1/(2-X)>=0,X>0.", "d(1;2).p(1)."),
        ("d(1).p(Y):-d(X),X=1,Y=0..1,1/Y=0.", "d(1)."),
    ] {
        let input = reference::admit(source, &FormulaLimits::default()).unwrap();
        let expected = reference::admit(expected, &FormulaLimits::default()).unwrap();
        assert_eq!(input.warnings().len(), 1, "{source}");
        assert_eq!(
            reference::exhaustive(&input),
            reference::exhaustive(&expected)
        );
    }
}

#[test]
fn reused_comparisons_keep_the_work_ceiling_inclusive() {
    let source = "d(1;2;3).p(X,Y):-d(X),d(Y),X+Y=4,X!=Y.";
    let admitted = |maximum| {
        reference::admit(
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
    let expected = Records::from([(
        ["d(1)", "d(2)", "d(3)", "p(1,3)", "p(3,1)"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        None,
    )]);
    assert_eq!(reference::exhaustive(&complete), expected);
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
    assert_eq!(reference::exhaustive(&exact), expected);
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
        assert_eq!(oracle::records(&case.source), case.records, "{}", case.name);
    }
}
