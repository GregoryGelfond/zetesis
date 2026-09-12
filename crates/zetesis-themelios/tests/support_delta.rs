//! Delta support coverage agrees with complete finite source substitutions.

#[path = "support/source_records.rs"]
mod source_records;
#[path = "support/source_oracle.rs"]
mod source_oracle;

use zetesis_themelios::{ExpansionFailure, FormulaFailure, FormulaLimits, FormulaResource};

const CASES: &[(&str, &str)] = &[
    (
        "p(0). q(0). p(1):-q(0). q(1):-p(0). r(X,Y):-p(X),q(Y).",
        "p(0..1). q(0..1). r(0,0). r(0,1). r(1,0). r(1,1).",
    ),
    (
        "p(0). p(1):-p(0). r(X,Y):-p(X),p(Y).",
        "p(0..1). r(0,0). r(0,1). r(1,0). r(1,1).",
    ),
    (
        "p(0..2). q(0). q(1):-q(0). r(X,Y):-p(X),q(Y).",
        "p(0..2). q(0..1). r(0,0). r(0,1). r(1,0). r(1,1). r(2,0). r(2,1).",
    ),
    ("d(0). d(1):-d(0). {p(X):d(X)}.", "d(0..1). {p(0);p(1)}."),
    (
        "p(0). q(0):-not nope. p(1):-q(0). r(X):-p(X),not z(X).",
        "p(0..1). q(0). r(0..1).",
    ),
    (
        "d(0). d(1):-d(0). p(N):-N=#count{X:d(X)}.",
        "d(0..1). p(2).",
    ),
    (
        "-p(0). -p(1):--p(0). q(X,Y):--p(X),-p(Y).",
        "-p(0..1). q(0,0). q(0,1). q(1,0). q(1,1).",
    ),
];

#[test]
fn delta_support_matches_complete_finite_substitutions() {
    for &(source, expanded) in CASES {
        let admitted = source_records::admit(source, &FormulaLimits::default()).unwrap();
        let expanded = source_records::admit(expanded, &FormulaLimits::default()).unwrap();
        assert_eq!(
            source_records::exhaustive(&admitted),
            source_records::exhaustive(&expanded),
            "{source}"
        );
    }
}

#[test]
fn delta_completion_retains_authored_body_errors() {
    // p already has support before the later invalid positive row appears.
    // Membership pruning is not a certificate of authored scalar validation.
    for source in [
        "p. d(1). d(0):-d(1). p:-d(X),1=2,1/X=1.",
        "p. d(1). d(0):-d(1). p:-d(X),1/X=1,1=2.",
    ] {
        assert!(
            matches!(
                source_records::admit(source, &FormulaLimits::default()),
                Err(FormulaFailure::Expansion(
                    ExpansionFailure::Evaluation { .. }
                ))
            ),
            "{source}"
        );
    }
}

#[test]
fn support_completion_requires_the_final_empty_round() {
    let source = "p(0). p(X+1):-p(X),X<3.";
    let exact = FormulaLimits {
        max_support_rounds: 5,
        ..Default::default()
    };
    assert!(source_records::admit(source, &exact).is_ok());
    let short = FormulaLimits {
        max_support_rounds: 4,
        ..exact
    };
    assert!(matches!(
        source_records::admit(source, &short),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::SupportRounds,
            observed: 5,
            limit: 4,
            ..
        })
    ));
}

#[test]
#[ignore = "requires independently installed clingo"]
fn delta_sources_match_clingo_complete_models() {
    for &(source, _) in CASES {
        let admitted = source_records::admit(source, &FormulaLimits::default()).unwrap();
        assert_eq!(
            source_records::exhaustive(&admitted),
            source_oracle::records(source),
            "{source}"
        );
    }
}
