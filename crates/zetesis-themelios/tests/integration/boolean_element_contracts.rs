//! Boolean head elements measure eligibility without inventing atom support.
//!
//! Each pool-expanded Boolean choice occurrence has one identity per outer
//! group; its local witnesses combine eligibility. These
//! source/model contracts and the independent reduct evaluator do not prescribe
//! a physical lowering. Clingo agreement is separate empirical corroboration.

use crate::support::objective_dependency_records as objective_dependencies;

use crate::support::head_element_reference as reference;

use reference::{Selection, cost_records, expected, external, models};
use zetesis_reference_support::formula;
use zetesis_themelios::{
    AdmissionOptions, AnalysisBasis, BundleAdmissionOptions, BundleLimits, CountPlanLimits,
    CountPlanStatus, ExpansionFailure, ExpansionLimits, FormulaFailure, FormulaLimits,
    FormulaResource, SourceBundle, admit_bundle_formula, admit_formula, prepare_formula,
};

const CASES: &[(&str, &[&[&str]])] = &[
    ("{#true}.", &[&[]]),
    ("{#false}.", &[&[]]),
    ("1{#true}1.", &[&[]]),
    ("1{#false}1.", &[]),
    ("0{#true}0.", &[]),
    ("0{#false}0.", &[&[]]),
    ("1{#true;a}1.", &[&[]]),
    ("1{#false;a}1.", &[&["a"]]),
    ("0{#false;a}0.", &[&[]]),
    ("2{#true;#true}2.", &[&[]]),
    ("{#true:p(1;1)}=2.p(1).", &[&["p(1)"]]),
    ("{#true:p(1;1)}=1.p(1).", &[]),
    ("{#true:p(1;1),q(1;1)}=4.p(1).q(1).", &[&["p(1)", "q(1)"]]),
    ("{not #false:p(1;1)}=2.p(1).", &[&["p(1)"]]),
    ("{not not #true:p(1;1)}=2.p(1).", &[&["p(1)"]]),
    ("{#true:p(X;X)}=2.p(1..2).", &[&["p(1)", "p(2)"]]),
    ("{#true:p(X;X)}=4.p(1..2).", &[]),
    ("a.2{#true;#true}2.", &[&["a"]]),
    ("a.2{#true:a;#true:a}2.", &[&["a"]]),
    ("1{#true}1.1{#true}1.", &[&[]]),
    ("2{#true}2.2{#true}2.", &[]),
    ("1{#true}1.1{#true;#true}1.", &[]),
    ("1{#true;#true}1.1{#true}1.", &[]),
    ("2{#true;#true}2.2{#true}2.", &[]),
    ("{a}.1{#true:a}1.1{#true:a;#true:a}1.", &[]),
    ("2{#true;#true}2.2{#true;#true}2.", &[&[]]),
    ("a.2{#true:a;#true:a}2.a.2{#true:a;#true:a}2.", &[&["a"]]),
    ("#const n=1.1{#true}1.n{#true}n.", &[&[]]),
    ("a.b.2{#true:a;#true:b}2.", &[&["a", "b"]]),
    ("a.b.1{#true:a;#true:b}1.", &[]),
    ("d(1..2).1{#true:d(X)}1.", &[&["d(1)", "d(2)"]]),
    ("d(1..2).2{#true:d(X)}2.", &[]),
    ("{a;b}.1{#true:a;#true:b}1.", &[&["a"], &["b"]]),
    ("1{#true:a}1.", &[]),
    ("a:-a.1{#true:a}1.", &[]),
    ("a:-a.1{#true:not not a}1.", &[]),
    ("{a}.1{#true:a}1.", &[&["a"]]),
    ("{a}.1{#false:a}1.", &[]),
    ("1#count{0:#true;0:a}1.", &[&[], &["a"]]),
    ("1#count{0:#false;0:a}1.", &[&["a"]]),
    ("1#count{:#true;:a}1.", &[&[], &["a"]]),
    ("1#count{:#false;:a}1.", &[&["a"]]),
    ("1#count{0:#true;0:#true}1.", &[&[]]),
    ("2#count{0:#true;0:#true}2.", &[]),
    (
        "{a}.1#count{1:#true:a;1:b}1.",
        &[&["a"], &["b"], &["a", "b"]],
    ),
    ("0#sum+{0:#true;0:a}0.", &[&[], &["a"]]),
    ("-1#sum{-1:#true;0:a}-1.", &[&[], &["a"]]),
    ("0#sum{-1:#false;0:a}0.", &[&[], &["a"]]),
    ("0#sum{0:#true:a}0.", &[&[]]),
    ("1#min{1:#true;1:a}1.", &[&[], &["a"]]),
    ("1#max{1:#true;1:a}1.", &[&[], &["a"]]),
    ("0#min{0:#false;0:a}0.", &[&["a"]]),
    ("0#max{0:#false;0:a}0.", &[&["a"]]),
    ("0#min{0:#true:a}0.", &[]),
    ("0#max{0:#true:not a}0.a:-a.", &[&[]]),
    ("1{#true}1:-#false.", &[&[]]),
    ("1{#false}1:-#false.", &[&[]]),
    ("{o}.1{#true}1:-o.", &[&[], &["o"]]),
    ("{o}.1{#false}1:-not o.", &[&["o"]]),
    ("{a}.1{#true:not a;#false:a}1.", &[&[]]),
    ("{a}.1{#true:not not a;#false:not a}1.", &[&["a"]]),
    ("d(0..1).1{#true:d(X)}1:-d(X).", &[&["d(0)", "d(1)"]]),
];

#[test]
fn constants_preserve_complete_model_contracts() {
    for &(source, records) in CASES {
        assert_eq!(models(source), expected(records), "{source}");
    }
}

#[test]
fn constant_heads_add_no_catalog_atoms() {
    for source in [
        "{#true;#false}.",
        "1#count{0:#true;0:#false}1.",
        "0#min{0:#true}0.",
    ] {
        assert!(formula(source).atoms().is_empty(), "{source}");
    }
}

#[test]
#[ignore = "requires clingo: original constant sources match clingo"]
fn original_constant_sources_match_clingo() {
    for &(source, _) in CASES {
        external(source);
    }
    for source in objectives() {
        external(&source);
    }
    external(CONDITION_OBJECTIVE);
    println!(
        "complete_original_sources={}",
        CASES.len() + objectives().len() + 1
    );
}

const CONDITION_OBJECTIVE: &str = "{a;b}.1{#true:a;#true:b}1.#minimize{1:a;1:b}.";

#[test]
fn boolean_eligibility_preserves_optimum_ties() {
    let records = cost_records(CONDITION_OBJECTIVE);
    let expected: std::collections::BTreeSet<_> = expected(&[&["a"], &["b"]])
        .into_iter()
        .map(|model| (model, Some(vec![1])))
        .collect();
    assert_eq!(records, expected);
}

#[test]
fn boolean_choice_analysis_reports_its_basis() {
    for source in ["2{#true;#true}2.", "1{#true}1.1{#true;#true}1."] {
        let prepared = prepare_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap();
        assert_eq!(
            prepared.analysis_basis(),
            AnalysisBasis::DependencyProjection
        );
        let admitted = prepared.ground().unwrap();
        assert_eq!(
            admitted.analysis_basis(),
            AnalysisBasis::DependencyProjection
        );
    }
    assert_eq!(
        formula("1{a;b}1.").analysis_basis(),
        AnalysisBasis::NormalizedProgram
    );
}

fn objectives() -> Vec<String> {
    [
        "1{#true;a}1.",
        "1#count{0:#true;0:a}1.",
        "1#min{1:#true;1:a}1.",
        "1#max{1:#true;1:a}1.",
    ]
    .map(|head| format!("{{x;y}}.{head}#minimize{{1@3,k:x;0@1,l:y}}."))
    .into()
}

#[test]
fn independent_objectives_retain_every_optimum_tie() {
    for source in objectives() {
        let records = cost_records(&source);
        let best = records.iter().map(|record| &record.1).min().unwrap();
        assert_eq!(*best, Some(vec![0, 0]));
        let optimum: Vec<_> = records.iter().filter(|record| &record.1 == best).collect();
        assert!(optimum.len() >= 2, "both y interpretations remain optimal");
        assert!(optimum.iter().all(|record| !record.0.contains("x")));
        assert!(optimum.iter().any(|record| record.0.contains("y")));
        assert!(optimum.iter().any(|record| !record.0.contains("y")));
    }
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(128))]

    #[test]
    fn constant_elements_match_frozen_worlds(
        rows in proptest::collection::vec((-2_i32..=2, 0_u8..3, 0_u8..4, 0_u8..10), 1..7),
        lower in -3_i32..=3, upper in -3_i32..=3,
        body in 0_u8..4, measure in 0_u8..6,
    ) {
        let rows = rows.into_iter().map(|(value, key, head, condition)| {
            (if measure == 4 { value.abs() } else { value }, key, head, condition)
        }).collect();
        Selection { rows, lower, upper, body, measure }.check_frozen();
    }

    #[test]
    fn reordered_elements_preserve_model_identity(
        rows in proptest::collection::vec((-2_i32..=2, 0_u8..3, 0_u8..4, 0_u8..10), 1..7),
        lower in -3_i32..=3, upper in -3_i32..=3,
        body in 0_u8..4, measure in 0_u8..6,
    ) {
        let rows = rows.into_iter().map(|(value, key, head, condition)| {
            (if measure == 4 { value.abs() } else { value }, key, head, condition)
        }).collect();
        let mut selection = Selection { rows, lower, upper, body, measure };
        let before = models(&selection.source());
        selection.rows.reverse();
        proptest::prop_assert_eq!(models(&selection.source()), before);
    }
}

#[test]
fn false_constants_cannot_hide_undefined_weights() {
    for function in ["#sum", "#sum+"] {
        let source = format!("0{function}{{1/0:#false}}0:-#false.");
        let error = admit_formula(
            source,
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap_err();
        assert!(
            matches!(
                error,
                FormulaFailure::Expansion(ExpansionFailure::Evaluation { .. })
            ),
            "{error}"
        );
    }
}

#[test]
fn local_constant_witnesses_require_safe_bindings() {
    let error = admit_formula(
        "1{#true:not p(X)}1.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap_err();
    assert!(
        matches!(error, FormulaFailure::UnsafeVariable { .. }),
        "{error}"
    );
}

#[test]
fn constant_activity_observes_element_refusal() {
    let error = admit_formula(
        "1#count{0:#true;1:#false}1.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits {
            aggregate: zetesis_ferraris::AggregateLimits {
                max_elements: 0,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(
        matches!(
            error,
            FormulaFailure::Limit {
                resource: FormulaResource::AggregateElements,
                limit: 0,
                observed: 1,
                ..
            }
        ),
        "{error}"
    );
}

#[test]
fn occurrence_keys_own_the_choice_element_budget() {
    let limits = FormulaLimits {
        aggregate: zetesis_ferraris::AggregateLimits {
            max_elements: 1,
            ..Default::default()
        },
        ..Default::default()
    };
    admit_formula(
        "d(1..2).1{#true:d(X)}1.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        limits,
    )
    .unwrap();
    let error = admit_formula(
        "2{#true;#true}2.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        limits,
    )
    .unwrap_err();
    assert!(
        matches!(
            error,
            FormulaFailure::Limit {
                resource: FormulaResource::AggregateElements,
                limit: 1,
                observed: 2,
                ..
            }
        ),
        "{error}"
    );
}

#[test]
fn constants_do_not_certify_atom_count_plans() {
    for source in [
        "2{#true;a;b;c;d}2.{a;b}1.{c;d}1.",
        "2#count{0:#true;1:a;2:b;3:c;4:d}2.{a;b}1.{c;d}1.",
    ] {
        let ordinary = formula(source);
        let planned = prepare_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap()
        .ground_with_count_plan(
            CountPlanLimits::default(),
            &zetesis_cpu::Cancellation::default(),
            None,
        )
        .unwrap();
        assert!(
            matches!(planned.count_plan(), CountPlanStatus::NoPlan(_)),
            "{source}"
        );
        assert_eq!(planned.atoms(), ordinary.atoms());
        assert_eq!(planned.theory().nodes(), ordinary.theory().nodes());
        assert_eq!(planned.theory().roots(), ordinary.theory().roots());
        assert_eq!(planned.formula_origins(), ordinary.formula_origins());
    }
}

#[test]
fn boolean_tuple_producers_preserve_scored_answers() {
    for source in [
        "1#count{1:#true;1:a}1.#minimize{1:a}.",
        "1#sum{1:#true;1:a}1.#minimize{1:a}.",
        "1#sum+{1:#true;1:a}1.#minimize{1:a}.",
        "1#min{1:#true;1:a}1.#minimize{1:a}.",
        "1#max{1:#true;1:a}1.#minimize{1:a}.",
    ] {
        objective_dependencies::check(source);
    }
}

#[test]
fn bundle_origins_do_not_multiply_activity() {
    for (rule, atom_count, satisfiable) in [
        ("1{#true;a}1.", 1, true),
        ("2{#true;#true;a}2.", 1, true),
        ("2{#true}2.", 0, false),
        ("1{not #false;a}1.", 1, true),
        ("2{not not #true;not not #true;a}2.", 1, true),
        ("2{not #false}2.", 0, false),
    ] {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join("first.lp"),
            format!("#include \"second.lp\".\n{rule}"),
        )
        .unwrap();
        std::fs::write(directory.path().join("second.lp"), rule).unwrap();
        let bundle =
            SourceBundle::load(directory.path().join("first.lp"), BundleLimits::default()).unwrap();
        let admitted = admit_bundle_formula(
            bundle,
            BundleAdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap();
        assert_eq!(
            admitted.analysis_basis(),
            AnalysisBasis::DependencyProjection
        );
        assert_eq!(admitted.atoms().len(), atom_count);
        let mut search = zetesis_sat::StableModels::new(
            admitted.theory(),
            zetesis_sat::Limits::default(),
            zetesis_cpu::Cancellation::default(),
        )
        .unwrap();
        let complete: Vec<_> = search
            .by_ref()
            .map(|model| model.unwrap().atoms().collect::<Vec<_>>())
            .collect();
        assert!(search.exhausted());
        let expected = if satisfiable {
            vec![Vec::<usize>::new()]
        } else {
            vec![]
        };
        assert_eq!(complete, expected, "{rule}");
        assert!(!admitted.formula_origins().is_empty());
        let sources: std::collections::BTreeSet<_> = admitted
            .formula_origins()
            .iter()
            .flatten()
            .map(|origin| origin.location().expect("parsed source").source)
            .collect();
        assert_eq!(sources.len(), 2, "both original files retain provenance");
        for origins in admitted.formula_origins() {
            for origin in origins {
                assert_eq!(
                    admitted
                        .bundle()
                        .get(origin.location().expect("parsed source").source)
                        .unwrap()
                        .source()
                        .slice(origin.location().expect("parsed source").span)
                        .unwrap(),
                    rule
                );
            }
        }
    }
}

#[test]
fn reordered_bundle_rules_preserve_occurrences() {
    let rules = ["1{#true;a}1.", "2{#true;#true;b}2."];
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("first.lp"),
        format!("#include \"second.lp\".\n{}{}", rules[0], rules[1]),
    )
    .unwrap();
    std::fs::write(
        directory.path().join("second.lp"),
        format!("{}{}", rules[1], rules[0]),
    )
    .unwrap();
    let bundle =
        SourceBundle::load(directory.path().join("first.lp"), BundleLimits::default()).unwrap();
    let admitted = admit_bundle_formula(
        bundle,
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    assert_eq!(admitted.atoms().len(), 2);
    let mut search = zetesis_sat::StableModels::new(
        admitted.theory(),
        zetesis_sat::Limits::default(),
        zetesis_cpu::Cancellation::default(),
    )
    .unwrap();
    let complete: Vec<_> = search
        .by_ref()
        .map(|model| model.unwrap().atoms().collect::<Vec<_>>())
        .collect();
    assert!(search.exhausted());
    assert_eq!(complete, vec![Vec::<usize>::new()]);
    let mut source_rules = std::collections::BTreeSet::new();
    for origins in admitted.formula_origins() {
        for origin in origins {
            let source = admitted
                .bundle()
                .get(origin.location().expect("parsed source").source)
                .unwrap()
                .source()
                .slice(origin.location().expect("parsed source").span)
                .unwrap();
            assert!(rules.contains(&source));
            source_rules.insert((
                origin.location().expect("parsed source").source,
                source.to_owned(),
            ));
        }
    }
    assert_eq!(
        source_rules.len(),
        4,
        "each original rule in both files is represented"
    );
}

#[test]
fn merged_bundle_rules_keep_original_multiplicity() {
    for rules in [
        ["1{#true}1.", "1{#true;#true}1."],
        ["1{#true;#true}1.", "1{#true}1."],
        ["2{#true;#true}2.", "2{#true}2."],
        ["1{not #false}1.", "1{not #false;not #false}1."],
        ["1{not not #true;not not #true}1.", "1{not not #true}1."],
    ] {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join("first.lp"),
            format!("#include \"second.lp\".\n{}", rules[0]),
        )
        .unwrap();
        std::fs::write(directory.path().join("second.lp"), rules[1]).unwrap();
        let bundle =
            SourceBundle::load(directory.path().join("first.lp"), BundleLimits::default()).unwrap();
        let admitted = admit_bundle_formula(
            bundle,
            BundleAdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap();
        assert!(admitted.atoms().is_empty());
        let mut search = zetesis_sat::StableModels::new(
            admitted.theory(),
            zetesis_sat::Limits::default(),
            zetesis_cpu::Cancellation::default(),
        )
        .unwrap();
        assert!(search.next().is_none(), "{rules:?}");
        assert!(
            search.exhausted(),
            "source inconsistency is a complete result"
        );
    }
}
