//! Extremal symbols remain typed closed values across source consumers.

use zetesis_clingo_support as oracle;
use zetesis_core::{Term, Value};
use zetesis_cpu::{Cancellation, CandidateLimits, CandidateTermination, Candidates, Limits, check};
use zetesis_reference_support as reference;
use zetesis_reference_support::{canonical, exhaustive};
use zetesis_test_support::records::Records;
use zetesis_themelios::{AdmissionOptions, FormulaLimits, admit};

#[test]
fn extremal_atom_arguments_retain_typed_values() {
    let input = admit(
        "p(#inf,#sup,\"#inf\",\"#sup\").".into(),
        AdmissionOptions::default(),
    )
    .unwrap();
    assert_eq!(
        input
            .program()
            .templates()
            .at(0)
            .unwrap()
            .head()
            .unwrap()
            .terms()
            .iter()
            .collect::<Vec<_>>(),
        [
            Term::Constant(Value::Infimum),
            Term::Constant(Value::Supremum),
            Term::Constant(Value::String("#inf".into())),
            Term::Constant(Value::String("#sup".into()))
        ]
        .iter()
        .map(zetesis_core::TemplateTerm::from)
        .collect::<Vec<_>>()
    );
}

fn expected(models: &[&[&str]]) -> Records {
    models
        .iter()
        .map(|model| (model.iter().map(|atom| (*atom).to_owned()).collect(), None))
        .collect()
}

const ORDINARY: &[(&str, &[&[&str]])] = &[
    ("t(#inf).", &[&["t(#inf)"]]),
    ("u(#sup).", &[&["u(#sup)"]]),
    ("{p(#sup)}.", &[&[], &["p(#sup)"]]),
    ("p:-not q(#inf).", &[&["p"]]),
    ("p(1).q(#inf,X):-p(X).", &[&["p(1)", "q(#inf,1)"]]),
    (
        "d(#inf).d(#sup).p(X):-d(X),X!=#sup.",
        &[&["d(#inf)", "d(#sup)", "p(#inf)"]],
    ),
    ("p(#inf).q(X):-p(X),X=#inf.", &[&["p(#inf)", "q(#inf)"]]),
    ("p(f(#inf,#sup)).", &[&["p(f(#inf,#sup))"]]),
];

const FORMULA: &[(&str, &[&[&str]])] = &[
    ("p(X):-X=#inf.", &[&["p(#inf)"]]),
    ("a|q(#sup).", &[&["a"], &["q(#sup)"]]),
    (
        "d(#inf;2).p(X):-d(Y),X=0..Y.",
        &[&["d(#inf)", "d(2)", "p(0)", "p(1)", "p(2)"]],
    ),
    (
        "d(#sup;2).p(X):-d(Y),X=Y..3.",
        &[&["d(#sup)", "d(2)", "p(2)", "p(3)"]],
    ),
    (
        "{a}.n(N):-N=#min{#inf:a}.p:-n(#sup).",
        &[&["n(#sup)", "p"], &["a", "n(#inf)"]],
    ),
    (
        "{a}.n(N):-N=#max{#sup:a}.p:-n(#inf).",
        &[&["n(#inf)", "p"], &["a", "n(#sup)"]],
    ),
    (
        "p(#inf).p(#sup).q(X):-p(X),X<0.r(X):-p(X),X>f(1).",
        &[&["p(#inf)", "p(#sup)", "q(#inf)", "r(#sup)"]],
    ),
];

#[test]
fn ordinary_extremal_sources_preserve_complete_models() {
    for &(source, wanted) in ORDINARY {
        let input = admit(source.into(), AdmissionOptions::default()).unwrap();
        let mut candidates = Candidates::new(
            input.program(),
            CandidateLimits::default(),
            Cancellation::default(),
        );
        let mut actual = Records::new();
        for seed in candidates.by_ref() {
            let result = check(
                input.program(),
                &seed.unwrap(),
                Limits::default(),
                &Cancellation::default(),
            )
            .unwrap();
            if result.accepted() {
                assert!(actual.insert((
                    result.closure().atoms().iter().map(canonical).collect(),
                    None
                )));
            }
        }
        assert_eq!(
            candidates.termination(),
            Some(CandidateTermination::Exhausted)
        );
        assert_eq!(actual, expected(wanted), "{source}");
    }
}

#[test]
fn formula_extremal_sources_preserve_complete_models() {
    for &(source, wanted) in ORDINARY.iter().chain(FORMULA) {
        let input = reference::admit(source, &FormulaLimits::default()).unwrap();
        assert_eq!(exhaustive(&input), expected(wanted), "{source}");
        assert_eq!(input.source().expect("source input").text(), source);
    }
}

const OBJECTIVE_BASE: &str = "{p(#inf)}.{p(#sup)}.";
const OBJECTIVES: &[&str] = &[
    "#minimize{X@7:p(X)}.",
    "#maximize{X@7:p(X)}.",
    "#minimize{1@X:p(X)}.",
    ":~p(X).[X@7]",
    "#minimize{#inf@7:p(#inf);#sup@7:p(#sup)}.",
    "#minimize{foo@7:p(X),X=#inf}.",
];

#[test]
fn extremal_objective_fields_supply_no_numeric_contribution() {
    let ordinary = reference::admit(OBJECTIVE_BASE, &FormulaLimits::default()).unwrap();
    let wanted = expected(&[&[], &["p(#inf)"], &["p(#sup)"], &["p(#inf)", "p(#sup)"]]);
    assert_eq!(exhaustive(&ordinary), wanted);
    for objective in OBJECTIVES {
        let source = format!("{OBJECTIVE_BASE}{objective}");
        let input = reference::admit(&source, &FormulaLimits::default()).unwrap();
        assert!(!input.objectives().is_present(), "{source}");
        assert_eq!(input.atoms(), ordinary.atoms(), "{source}");
        assert_eq!(
            (input.theory().nodes(), input.theory().operands()),
            (ordinary.theory().nodes(), ordinary.theory().operands()),
            "{source}"
        );
        assert_eq!(
            input.theory().roots(),
            ordinary.theory().roots(),
            "{source}"
        );
        assert_eq!(exhaustive(&input), wanted, "{source}");
    }
}

#[test]
fn extremal_values_keep_their_term_order() {
    assert!(
        Value::Infimum
            .compare_terms(&Value::Number(i32::MIN))
            .is_lt()
    );
    assert!(
        Value::Supremum
            .compare_terms(&Value::String("#sup".into()))
            .is_gt()
    );
    assert_ne!(Value::Infimum, Value::String("#inf".into()));
    assert_ne!(Value::Supremum, Value::String("#sup".into()));
}

#[test]
#[ignore = "requires clingo: extremal sources match clingo; complete extremal-value families"]
fn extremal_sources_match_clingo() {
    for &(source, wanted) in ORDINARY.iter().chain(FORMULA) {
        assert_eq!(oracle::records(source), expected(wanted), "{source}");
    }
    let wanted = expected(&[&[], &["p(#inf)"], &["p(#sup)"], &["p(#inf)", "p(#sup)"]]);
    for objective in OBJECTIVES {
        let source = format!("{OBJECTIVE_BASE}{objective}");
        assert_eq!(oracle::records(&source), wanted, "{source}");
    }
}
