//! Finite inverse matching, structural alternatives and wildcard key identities.

use zetesis_core::Model;
use zetesis_cpu::Cancellation;
use zetesis_themelios::observation::{ErrorKind, EvaluationError, Limits, Resource};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

fn input(source: &str) -> zetesis_themelios::AdmittedFormula {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_or_else(|error| panic!("{source}: {error}"))
}
fn shown(source: &str) -> Vec<String> {
    let input = input(source);
    let model = Model::new(input.atoms().iter().cloned());
    input
        .metadata()
        .observations()
        .render(
            &model,
            input.metadata().output(),
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap()
        .text()
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

#[test]
fn one_invertible_occurrence_supplies_one_checked_scalar() {
    for (source, expected) in [
        ("p(2).#show.#show X:p(X+1).", vec!["1"]),
        ("p(f(7)).#show.#show X:p(f(2*X+1)).", vec!["3"]),
        ("#show.#show (X,Y):X=Y+1=2.", vec!["(2,1)"]),
        ("#show.#show X:7=1+2*X.", vec!["3"]),
        ("#show.#show X:2*X=7.", vec![]),
        ("#show.#show X:5-X=2.", vec!["3"]),
        ("#show.#show X: -(X+1)=2.", vec!["-3"]),
        ("#show.#show X:X+1=(-2147483647-1).", vec![]),
        ("p(2,3).#show.#show X:p(X+1,X).", vec![]),
    ] {
        assert_eq!(shown(source), expected, "{source}");
    }
}
#[test]
fn authored_arithmetic_errors_remain_errors_after_inverse_capture() {
    let input = input("#show ok.#show X:X+(1/0)=2.");
    let error = input
        .metadata()
        .observations()
        .evaluate(
            &Model::default(),
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap_err();
    assert_eq!(
        error.kind(),
        &ErrorKind::Evaluation(EvaluationError::Undefined)
    );
    assert!(error.location().is_some());
}
#[test]
fn structural_pool_captures_share_one_complete_guard() {
    for (source, expected) in [
        ("#show.#show X:(f(X);g(X))=f(1).", vec!["1"]),
        ("#show.#show 1:f((X;2))=f(1).", vec!["1"]),
        ("#show.#show (X,Y):(f(X);g(X))=Y=f(1).", vec!["(1,f(1))"]),
        ("#show.#show X:(f(X,X);g(X,X))=f(1,2).", vec![]),
        ("n(1).#show.#show X:n(X),f((X;2))=f(1).", vec!["1"]),
    ] {
        assert_eq!(shown(source), expected, "{source}");
    }
}
#[test]
fn constructor_capture_normalizes_repeated_signs() {
    for source in [
        "#show.#show X: -(-f(X))=f(1).",
        "#show.#show X: -(-(-f(X)))=-f(1).",
    ] {
        assert_eq!(shown(source), ["1"], "{source}");
    }
}

#[test]
fn negative_cardinality_uses_existential_witnesses() {
    for (source, expected) in [
        ("#show.#show N:N={not p(_)}.", "1"),
        ("p(1).p(2).#show.#show N:N={not not p(_)}.", "1"),
        ("p(1).#show.#show N:N={not p(_)}.", "0"),
        ("p(f(1)).p(f(2)).#show.#show N:N={not not p(f(_))}.", "1"),
        ("-p(1).#show.#show N:N={not p(_);not not -p(_)}.", "2"),
    ] {
        assert_eq!(shown(source), [expected], "{source}");
    }
}

#[test]
fn negative_cardinality_deduplicates_equal_concrete_alternatives() {
    for source in [
        "#show.#show N:N={not p(_);not p(1);not p((_;1))}.",
        "#show.#show N:N={not p(f((_;1)));not p(f(1))}.",
        "d(f(1)).#show.#show N:N={not p(f((_;1)));not p(X):d(X)}.",
    ] {
        assert_eq!(shown(source), ["2"], "{source}");
    }
}
#[test]
fn numeric_aggregate_mismatch_keeps_wide_measures() {
    assert!(shown("#show.#show X:f(X)=#sum{2147483647,a;1,b}.").is_empty());
}
#[test]
fn numeric_aggregate_mismatch_preserves_authored_errors() {
    let input = input("#show ok.#show X:f(X)=#sum{1/0,a}.");
    let error = input
        .metadata()
        .observations()
        .evaluate(
            &Model::default(),
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap_err();
    assert_eq!(
        error.kind(),
        &ErrorKind::Evaluation(EvaluationError::Undefined)
    );
}
#[test]
fn inverse_work_admission_is_inclusive() {
    let input = input("#show ok.#show X:(f(X+1);g(X+1))=f(2).");
    let evaluate = |max_work| {
        input.metadata().observations().evaluate(
            &Model::default(),
            Limits {
                max_work,
                ..Limits::default()
            },
            &Cancellation::default(),
        )
    };
    let complete = evaluate(u64::MAX).unwrap();
    for max_work in 0..complete.statistics().work {
        let error = evaluate(max_work).unwrap_err();
        assert!(matches!(
            error.kind(),
            ErrorKind::Limit {
                resource: Resource::Work,
                ..
            }
        ));
        assert!(error.statistics().work <= max_work);
    }
    assert_eq!(
        evaluate(complete.statistics().work).unwrap().symbols(),
        complete.symbols()
    );
}
