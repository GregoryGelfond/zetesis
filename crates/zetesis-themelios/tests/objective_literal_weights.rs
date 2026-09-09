//! Closed nonnumeric weights supply neither a numeric key nor a priority slot.

#[path = "support/source_records.rs"]
mod reference;
#[path = "support/objective_literal_cases.rs"]
mod sources;

use reference::{Records, admit, exhaustive};
use sources::{DIRECTIONS, PROGRAM, WEIGHTS, cases, directive};
use std::collections::BTreeSet;
use zetesis_core::{Model, Value};
use zetesis_cpu::Control;
use zetesis_themelios::{
    AdmissionOptions, ExpansionFailure, ExpansionLimits, ExpansionResource, FormulaFailure,
    FormulaLimits, FormulaResource, admit_formula,
};

const CONDITION_CASES: &str = r##"{"name":"absent_condition","source":"#minimize{foo:a}.","records":[[[],null]]}
{"name":"weak_condition","source":"{a}. :~a.[foo]","records":[[[],null],[["a"],null]]}"##;

#[test]
fn literal_conditions_preserve_complete_records() {
    for case in reference::cases(CONDITION_CASES) {
        assert_eq!(
            exhaustive(&admit(&case.source, FormulaLimits::default()).unwrap()),
            case.records
        );
    }
}

#[test]
fn literal_weights_preserve_complete_model_records() {
    let cases = cases();
    assert_eq!(cases.len(), 162);
    for case in cases {
        let input = admit(&case.reference.source, FormulaLimits::default())
            .unwrap_or_else(|error| panic!("{}: {error}", case.reference.name));
        assert_eq!(
            input.objectives().priorities(),
            case.priorities,
            "{}",
            case.reference.name
        );
        assert_eq!(
            exhaustive(&input),
            case.reference.records,
            "{}",
            case.reference.name
        );
        assert_eq!(input.source().text(), case.reference.source);
    }
}

#[test]
fn ignored_weights_preserve_the_original_reduct_subject() {
    let ordinary = admit(PROGRAM, FormulaLimits::default()).unwrap();
    for case in cases() {
        let input = admit(&case.reference.source, FormulaLimits::default()).unwrap();
        // Identical original nodes imply identical truth for every original and
        // frozen interpretation, including candidates rejected by the reduct.
        assert_eq!(input.atoms(), ordinary.atoms());
        assert_eq!(input.theory().nodes(), ordinary.theory().nodes());
        assert_eq!(input.theory().roots(), ordinary.theory().roots());
        assert_eq!(input.formula_origins(), ordinary.formula_origins());
    }
}

#[test]
fn symbolic_extremum_literal_weights_leave_no_slot() {
    let source = "b.{a}.n(N):-N=#max{2:a;foo:b}.p(X):-n(X).#minimize{foo@7,X:p(X)}.";
    let input = admit(source, FormulaLimits::default()).unwrap();
    let reference = reference::cases(
        r#"{"name":"symbol_weight","source":"","records":[[["b","n(foo)","p(foo)"],null],[["a","b","n(foo)","p(foo)"],null]]}"#,
    );
    assert_eq!(exhaustive(&input), reference[0].records);
    assert!(!input.objectives().is_present());
}

#[test]
fn empty_programs_keep_objective_absence() {
    for weight in WEIGHTS {
        for direction in DIRECTIONS {
            let source = directive(direction, weight, 7, "");
            let input = admit(&source, FormulaLimits::default()).unwrap();
            assert_eq!(exhaustive(&input), Records::from([(BTreeSet::new(), None)]));
        }
    }
}

#[test]
fn reference_spelling_preserves_atom_sign() {
    let input = admit("a.-b.p(1).-q(2).", FormulaLimits::default()).unwrap();
    let records = exhaustive(&input);
    assert_eq!(
        records,
        Records::from([(
            ["a", "-b", "p(1)", "-q(2)"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            None,
        )])
    );
}

#[test]
fn ignored_weights_do_not_hide_invalid_siblings() {
    for source in [
        "a.#minimize{foo@7;1@X:a}.",
        "a.#minimize{foo@7,X:a}.",
        "a.#minimize{foo@7:not a}.",
        "a.#maximize{foo@7;-2147483648@3}.",
    ] {
        let error = admit(source, FormulaLimits::default()).unwrap_err();
        assert!(!error.diagnostics().is_empty(), "{source}");
    }
}

#[test]
fn ignored_templates_retain_source_limits() {
    let mut limits = FormulaLimits::default();
    limits.objective.max_templates = 0;
    assert!(matches!(
        admit("#minimize{foo}.", limits),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::ObjectiveElements,
            limit: 0,
            observed: 1,
            ..
        })
    ));
}

#[test]
fn literal_admission_limits_are_inclusive() {
    let source = "{a;b}.#minimize{f(1,\"text\")@7,k:a;0@3,k:b}.";
    let reference = exhaustive(&admit(source, FormulaLimits::default()).unwrap());
    for resource in [FormulaResource::Work, FormulaResource::Substitutions] {
        let attempt = |maximum| {
            let mut limits = FormulaLimits::default();
            if resource == FormulaResource::Work {
                limits.max_work = maximum;
            } else {
                limits.max_substitutions = maximum;
            }
            admit(source, limits)
        };
        let exact = threshold(|maximum| attempt(maximum).is_ok());
        assert_eq!(exhaustive(&attempt(exact).unwrap()), reference);
        assert!(matches!(attempt(exact - 1), Err(FormulaFailure::Limit {
            resource: actual, observed, limit, ..
        }) if actual == resource && observed == u128::from(exact)
            && limit == u128::from(exact - 1)));
    }
    let attempt = |maximum| {
        admit_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits {
                max_scalar_bytes: usize::try_from(maximum).unwrap(),
                ..Default::default()
            },
            FormulaLimits::default(),
        )
    };
    let exact = threshold(|maximum| attempt(maximum).is_ok());
    assert_eq!(exhaustive(&attempt(exact).unwrap()), reference);
    assert!(matches!(attempt(exact - 1), Err(FormulaFailure::Expansion(
        ExpansionFailure::Limit {
            resource: ExpansionResource::ScalarBytes, observed, limit, ..
        }
    )) if observed == u128::from(exact) && limit == u128::from(exact - 1)));
}

fn threshold(mut succeeds: impl FnMut(u64) -> bool) -> u64 {
    let (mut lower, mut upper) = (0, 65_536);
    assert!(succeeds(upper));
    while lower + 1 < upper {
        let middle = lower + (upper - lower) / 2;
        if succeeds(middle) {
            upper = middle;
        } else {
            lower = middle;
        }
    }
    upper
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config { cases: 128, ..Default::default() })]

    #[test]
    fn ignored_literals_cannot_create_numeric_keys(
        weight in 0_usize..WEIGHTS.len(),
        selected in proptest::bool::ANY,
        amount in -8_i32..9,
        priority in -3_i32..4,
    ) {
        let source = format!("{{a}}.#minimize{{{}@{},k:a;{amount}@{priority},k:a;{amount}@{priority},k:a}}.",
            WEIGHTS[weight], priority + 1);
        let input = admit(&source, FormulaLimits::default()).unwrap();
        let model = Model::new(input.atoms().iter().filter(|atom| selected && atom.values().is_empty()).cloned());
        let evaluation = zetesis_objective::evaluate(input.objectives(), &model,
            zetesis_objective::Limits::default(), &Control::default()).unwrap();
        proptest::prop_assert_eq!(evaluation.score().costs(), &[(priority, if selected { i64::from(amount) } else { 0 })]);
        proptest::prop_assert_eq!(evaluation.contributions().len(), usize::from(selected));
        if let Some(key) = evaluation.contributions().first() {
            proptest::prop_assert_eq!(key.weight(), amount);
            proptest::prop_assert_eq!(key.tuple(), &[Value::Symbol("k".into())]);
        }
    }
}

#[test]
#[ignore = "requires independent clingo; unchanged literal-weight source matrix"]
fn literal_weight_sources_match_fresh_clingo() {
    for case in reference::cases(CONDITION_CASES) {
        external(&case.name, &case.source, &case.records);
    }
    for case in cases() {
        external(
            &case.reference.name,
            &case.reference.source,
            &case.reference.records,
        );
    }
    for (index, weight) in WEIGHTS.into_iter().enumerate() {
        for direction in DIRECTIONS {
            let source = directive(direction, weight, 7, "");
            external(
                &format!("empty_{index}_{direction:?}"),
                &source,
                &Records::from([(BTreeSet::new(), None)]),
            );
        }
    }
    for (name, source) in [
        ("signed", "a.-b.p(1).-q(2).#minimize{foo@7: -b}."),
        (
            "mixed_extremum",
            "b.{a}.n(N):-N=#max{2:a;foo:b}.p(X):-n(X).#minimize{foo@7,X:p(X)}.",
        ),
    ] {
        external(
            name,
            source,
            &exhaustive(&admit(source, FormulaLimits::default()).unwrap()),
        );
        assert_eq!(
            reference::clingo(source),
            exhaustive(&admit(source, FormulaLimits::default()).unwrap())
        );
    }
}

fn external(name: &str, source: &str, expected: &Records) {
    let capture = reference::capture_clingo(source);
    assert_eq!(&capture.records, expected, "{name}");
    println!(
        "reference_json: {}",
        serde_json::json!({
            "name": name,
            "source": source,
            "arguments": ["0", "--outf=2", "--opt-mode=enum", "--warn=none"],
            "status": capture.status,
            "stdout": capture.output,
            "stderr": capture.diagnostics,
        })
    );
}
