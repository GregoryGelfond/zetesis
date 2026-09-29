//! Ground Boolean/comparison guards preserve the original and every reduct.
//! Exact external records complement a separate finite definition evaluator.

use std::collections::BTreeSet;
use std::fs::{self};

use crate::support::clingo_reports::{enumerated, expected, json_model};
use crate::support::finite_bindings::{Models, exhaustive, holds, native, remap, values};
use crate::support::sourced_admission::{SOURCE, input, options};
use serde_json::Value as Json;
use zetesis_themelios::{
    BundleAdmissionOptions, BundleLimits, ExpansionFailure, ExpansionLimits, ExpansionResource,
    FormulaFailure, FormulaLimits, FormulaResource, SourceBundle, admit_bundle_formula,
    admit_formula,
};

fn cases() -> Vec<Json> {
    include_str!("../fixtures/ground-guards.jsonl")
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn exact_sources_match_external_records_and_an_independent_reduct_evaluator() {
    let cases = cases();
    assert_eq!(cases.len(), 124);
    let mut models = 0;
    for row in cases {
        let source = row["source"].as_str().unwrap();
        let admitted = input(source).unwrap_or_else(|error| panic!("{}: {error}", row["name"]));
        assert_eq!(admitted.source().text(), source);
        let expected = expected(&row);
        models += expected.len();
        assert_eq!(exhaustive(&admitted), expected, "{}", row["name"]);
        assert_eq!(native(&admitted), expected, "{}", row["name"]);
    }
    assert_eq!(models, 120);
}

#[test]
fn ground_guard_replacement_preserves_every_original_and_frozen_pair() {
    for (guard, truth) in [
        ("#true", true),
        ("#false", false),
        ("not #true", false),
        ("not #false", true),
        ("not not #true", true),
        ("not not #false", false),
        ("1<2<3", true),
        ("1<3<2", false),
        ("not 1<2<3", false),
        ("not 1<3<2", true),
        ("not 3<1<2", true),
        ("not 3<2<1", true),
        ("not not 1<2<3", true),
        ("not not 1<3<2", false),
    ] {
        let source = input(&format!("{{p;q}}.p:-q,{guard}.q:-p.")).unwrap();
        let replacement = if truth { "1=1" } else { "1=2" };
        let expanded = input(&format!("{{p;q}}.p:-q,{replacement}.q:-p.")).unwrap();
        assert_eq!(
            source.atoms().iter().collect::<BTreeSet<_>>(),
            expanded.atoms().iter().collect()
        );
        for outer in 0..1 << source.atoms().len() {
            let original = values(source.theory(), outer, None);
            let other = values(
                expanded.theory(),
                remap(outer, source.atoms(), expanded.atoms()),
                None,
            );
            assert_eq!(
                holds(source.theory(), &original),
                holds(expanded.theory(), &other),
                "{guard}"
            );
            for inner in 0..1 << source.atoms().len() {
                assert_eq!(
                    holds(
                        source.theory(),
                        &values(source.theory(), inner, Some(&original))
                    ),
                    holds(
                        expanded.theory(),
                        &values(
                            expanded.theory(),
                            remap(inner, source.atoms(), expanded.atoms()),
                            Some(&other)
                        )
                    ),
                    "{guard}, M={outer}, J={inner}"
                );
            }
        }
    }
}

#[test]
fn guard_conditions_do_not_supply_missing_bindings_or_expand_other_source_profiles() {
    // These original profile refusals are now covered by the separate finite
    // generator plan. The guard itself remains in the compiled body.
    for (source, expected) in [
        ("p(X):-not not X=1.", "p(1)."),
        ("p(X):-not not X=1<2.", "p(1)."),
        ("p(X):-X=1<2.", "p(1)."),
        ("p(X):-0<X<3.", "p(1..2)."),
    ] {
        assert_eq!(
            native(&input(source).unwrap()),
            native(&input(expected).unwrap())
        );
    }
    for source in [
        "p(X):-not X=1.",
        "p(X):-#false.",
        "d(1).p(X):-#count{X:d(X),#true}>0.",
        "d(1).{p(X):#true}.",
    ] {
        let error = input(source).unwrap_err();
        assert!(
            matches!(error, FormulaFailure::UnsafeVariable { .. }),
            "{source}: {error}"
        );
        assert!(
            error
                .diagnostics()
                .iter()
                .all(|diagnostic| diagnostic.primary().location.source == SOURCE)
        );
    }
    // Boolean heads are qualified independently in boolean_heads.rs.
    assert_eq!(
        native(&input("#true.").unwrap()),
        BTreeSet::from([BTreeSet::new()])
    );
    assert_eq!(
        native(&input("q:-p(X):d(X).").unwrap()),
        BTreeSet::from([BTreeSet::from(["q".to_owned()])])
    );
}

#[test]
fn undefined_or_overflowing_guard_values_never_become_boolean_false() {
    for source in [
        "p:-not 1/0=0.",
        "p:-not not 1/0=0.",
        "p:-not 1/0=0=1.",
        "p:-not 0=1=1/0.",
        "p:-#false,not 1/0=0.",
        "p:-not 1/0=0,#false.",
        "{p:not 1/0=0}.",
        "p:-#count{1:not 1/0=0}=1.",
        "d(2147483647).p:-d(X),not X+1=0.",
        "d(0).p:-d(X),not (1,2)=(1,2,1/X).",
        "d(0).p:-d(X),not not (1,2,1/X)!=(1,2).",
    ] {
        let error = input(source).unwrap_err();
        assert!(
            matches!(
                error,
                FormulaFailure::Expansion(ExpansionFailure::Evaluation { .. })
            ),
            "{source}: {error}"
        );
        assert!(!error.diagnostics().is_empty());
    }
}

#[test]
fn negated_guards_omit_undefined_substitutions() {
    let program = input("d(0..2).p(X):-d(X),not 1/X=1.").unwrap();
    let expected = native(&input("d(0..2).p(2).").unwrap());
    assert_eq!(program.warnings().len(), 1);
    assert_eq!(native(&program), expected);
    assert_eq!(exhaustive(&program), expected);
}

#[test]
fn incomplete_guard_construction_and_ground_evaluation_have_located_limits() {
    let source = "d(1..3).p(X):-d(X),not 0<X<3.";
    let error = admit_formula(
        source.into(),
        options(),
        ExpansionLimits {
            max_term_work: 1,
            ..Default::default()
        },
        FormulaLimits::default(),
    )
    .unwrap_err();
    assert!(
        matches!(error, FormulaFailure::Expansion(ExpansionFailure::Limit { resource: ExpansionResource::TermWork, observed, limit, .. }) if observed > limit)
    );
    let error = admit_formula(
        source.into(),
        options(),
        ExpansionLimits::default(),
        FormulaLimits {
            max_work: 0,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(
        matches!(error, FormulaFailure::Limit { resource: FormulaResource::Work, observed, limit, location } if observed > limit && location.source == SOURCE)
    );
    let admitted = input(source).unwrap();
    assert_eq!(native(&admitted), exhaustive(&admitted));
    assert!(
        admitted
            .formula_origins()
            .iter()
            .flatten()
            .all(|location| location.source == SOURCE)
    );
}

#[test]
fn bundle_guards_keep_original_sources_signatures_and_rule_origins() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("root.lp");
    let rules = directory.path().join("rules.lp");
    let source = "#include \"rules.lp\". d(1..3). #show p/1. #defined d/1.";
    let rule = "p(X):-d(X),not 0<X<3.";
    fs::write(&root, source).unwrap();
    fs::write(&rules, rule).unwrap();
    let bundle = SourceBundle::load(&root, BundleLimits::default()).unwrap();
    let admitted = admit_bundle_formula(
        bundle,
        BundleAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let retained: BTreeSet<_> = admitted
        .bundle()
        .sources()
        .iter()
        .map(|source| source.source().text())
        .collect();
    assert_eq!(retained, BTreeSet::from([source, rule]));
    let rule_id = admitted
        .bundle()
        .sources()
        .iter()
        .find(|source| source.source().text() == rule)
        .unwrap()
        .id();
    let origins: Vec<_> = admitted.formula_origins().iter().flatten().collect();
    assert!(origins.iter().any(|location| location.source == rule_id));
    assert!(origins.iter().all(|location| {
        admitted
            .bundle()
            .get(location.source)
            .is_some_and(|source| source.source().slice(location.span).is_ok())
    }));
    assert!(
        admitted
            .atoms()
            .iter()
            .any(|atom| admitted.metadata().output().includes(atom))
    );
    assert!(
        admitted
            .atoms()
            .iter()
            .filter(|atom| admitted.metadata().output().includes(*atom))
            .all(|atom| atom.predicate().name() == "p")
    );
}

#[test]
#[ignore = "requires external clingo; exact sources and complete full-model replay"]
fn ground_guards_match_clingo() {
    for row in cases() {
        let actual = enumerated(row["source"].as_str().unwrap());
        assert_eq!(actual["Models"]["More"], "no");
        let witnesses: Vec<_> = actual["Call"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|call| call["Witnesses"].as_array().into_iter().flatten())
            .collect();
        let records: Models = witnesses
            .iter()
            .map(|witness| {
                assert!(witness["Costs"].is_null());
                json_model(&witness["Value"])
            })
            .collect();
        assert_eq!(records.len(), witnesses.len());
        assert_eq!(
            actual["Models"]["Number"].as_u64(),
            Some(witnesses.len() as u64)
        );
        assert_eq!(records, expected(&row), "{}", row["name"]);
        assert_eq!(
            native(&input(row["source"].as_str().unwrap()).unwrap()),
            records,
            "{}",
            row["name"]
        );
    }
}
