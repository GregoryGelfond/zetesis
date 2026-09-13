//! Scalar data evaluation retains complete original and frozen source semantics.
#[path = "support/finite_bindings.rs"]
mod reference;

use themelios_base::source::SourceId;
use themelios_program::term::EvalError;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionFailure, ExpansionLimits, FormulaFailure,
    FormulaLimits, admit_formula,
};

const CASES: &[(&str, &str)] = &[
    (
        "d(-3;2). p(X+2,X*3,X/2,X\\2,X**2,~X,|X|):-d(X).",
        "d(-3;2). p(-1,-9,-1,-1,9,2,3):-d(-3). p(4,6,1,0,4,-3,2):-d(2).",
    ),
    (
        "d(1..2). {p(X)}:-d(X). q:-p(X),((X+2)*3)/2=4.",
        "d(1..2). {p(1)}:-d(1). {p(2)}:-d(2). q:-p(1).",
    ),
    (
        "d(-7;7). p(X/(-3),X\\(-3)):-d(X).",
        "d(-7;7). p(2,-1):-d(-7). p(-2,1):-d(7).",
    ),
    (
        "d(0;1). p(X**0,(X+1)**31):-d(X),X=0.",
        "d(0;1). p(1,1):-d(0).",
    ),
    (
        "d(2). p(f(X+1,(X*X,)),~X):-d(X).",
        "d(2). p(f(3,(4,)),-3):-d(2).",
    ),
    (
        "d(#inf;\"x\"). p(f(X,()),(X,)):-d(X).",
        "d(#inf;\"x\"). p(f(#inf,()),(#inf,)):-d(#inf). p(f(\"x\",()),(\"x\",)):-d(\"x\").",
    ),
    (
        "d(-f(1)). p(g(X,X),(X,)):-d(X).",
        "d(-f(1)). p(g(-f(1),-f(1)),(-f(1),)):-d(-f(1)).",
    ),
];
fn input(source: &str) -> AdmittedFormula {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
}

#[test]
fn scalar_sources_keep_complete_models() {
    for &(source, expanded) in CASES {
        let admitted = input(source);
        assert_eq!(
            reference::native(&admitted),
            reference::native(&input(expanded)),
            "{source}"
        );
        assert_eq!(
            reference::native(&admitted),
            reference::exhaustive(&admitted),
            "{source}"
        );
    }
}

#[test]
fn scalar_sources_keep_every_frozen_pair() {
    for &(source, expanded) in CASES {
        let left = input(source);
        let right = input(expanded);
        assert_eq!(left.atoms().len(), right.atoms().len());
        let right_indices: Vec<_> = left
            .atoms()
            .iter()
            .map(|atom| {
                right
                    .atoms()
                    .iter()
                    .position(|other| atom == other)
                    .unwrap()
            })
            .collect();
        let right_mask = |mask: usize| {
            right_indices
                .iter()
                .enumerate()
                .fold(0, |result, (left, right)| {
                    result | (((mask >> left) & 1) << right)
                })
        };
        assert!(left.atoms().len() <= 8, "bounded complete interpretations");
        for outer in 0..1 << left.atoms().len() {
            let left_frozen = reference::values(left.theory(), outer, None);
            let right_frozen = reference::values(right.theory(), right_mask(outer), None);
            assert_eq!(
                reference::holds(left.theory(), &left_frozen),
                reference::holds(right.theory(), &right_frozen),
                "original M={outer}: {source}"
            );
            for inner in 0..1 << left.atoms().len() {
                assert_eq!(
                    reference::holds(
                        left.theory(),
                        &reference::values(left.theory(), inner, Some(&left_frozen))
                    ),
                    reference::holds(
                        right.theory(),
                        &reference::values(right.theory(), right_mask(inner), Some(&right_frozen))
                    ),
                    "frozen M={outer}, J={inner}: {source}"
                );
            }
        }
    }
}

#[test]
fn arithmetic_refusals_retain_the_source_rule() {
    for (source, expected) in [
        ("d(2147483647). p(X+1):-d(X).", EvalError::Overflow),
        ("d(-2147483647-1). p(X/(-1)):-d(X).", EvalError::Overflow),
        ("d(-2147483647-1). p(X\\(-1)):-d(X).", EvalError::Overflow),
        ("d(-2147483647-1). p(-X):-d(X).", EvalError::Overflow),
        ("d(-2147483647-1). p(|X|):-d(X).", EvalError::Overflow),
        ("d(0). p(1/X):-d(X).", EvalError::Undefined),
        ("d(-1). p(2**X):-d(X).", EvalError::Undefined),
        ("d(a). p(X+1):-d(X).", EvalError::Undefined),
        ("d(f(1)). p(X+1):-d(X).", EvalError::Undefined),
    ] {
        let source_id = SourceId::new(29);
        let failure = admit_formula(
            source.into(),
            AdmissionOptions {
                source_id,
                ..Default::default()
            },
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap_err();
        let FormulaFailure::Expansion(ExpansionFailure::Evaluation { error, location }) = failure
        else {
            panic!("expected evaluated arithmetic refusal: {failure}");
        };
        assert_eq!(error, expected, "{source}");
        assert_eq!(location.source, source_id);
        let start = usize::try_from(location.span.start().get()).unwrap();
        let end = usize::try_from(location.span.end().get()).unwrap();
        assert_eq!(
            &source[start..end],
            &source[source.find("p(").unwrap()..],
            "exact source rule location: {source}"
        );
    }
}

#[test]
#[ignore = "requires independently installed clingo"]
fn scalar_models_match_clingo() {
    for &(source, _) in CASES {
        let output = reference::external(source, true);
        assert_eq!(output["Models"]["More"], "no");
        let mut models = reference::Models::new();
        for witness in output["Call"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|call| call["Witnesses"].as_array().into_iter().flatten())
        {
            assert!(
                models.insert(
                    witness["Value"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|atom| atom.as_str().unwrap().to_owned())
                        .collect()
                )
            );
        }
        assert_eq!(
            output["Models"]["Number"].as_u64().unwrap(),
            models.len() as u64
        );
        assert_eq!(reference::native(&input(source)), models, "{source}");
    }
}
