//! The source subject is checked against the independent complete formula door.

use crate::test_support::prepared;
use crate::{AdmissionOptions, ExpansionLimits, FormulaLimits, SourceAdmission, prepare_formula};
use std::{collections::BTreeSet, sync::Arc};
use zetesis_core::{Atom, ValueLimits};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, Limits, check};

fn admitted(source: &str) -> super::AcyclicSource {
    let SourceAdmission::Admitted(owner) = prepared(source).try_source().unwrap() else {
        panic!("expected complete signed acyclic normal source: {source}");
    };
    owner
}

fn interpretation(owner: &super::AcyclicSource) -> Option<BTreeSet<Atom>> {
    owner.selected_positions().map(|positions| {
        positions
            .iter()
            .map(|&position| {
                owner
                    .atom_catalog()
                    .atoms()
                    .at(position)
                    .unwrap()
                    .to_atom(ValueLimits::default())
                    .unwrap()
            })
            .collect()
    })
}

fn independent(source: &str) -> BTreeSet<BTreeSet<Atom>> {
    let formula = prepared(source).ground().unwrap();
    let count = formula.atoms().len();
    assert!(count < 12);
    let mut models = BTreeSet::new();
    for mask in 0..(1_usize << count) {
        let candidate = Interpretation::new(
            formula.theory(),
            (0..count).filter(|&i| mask & (1 << i) != 0),
        )
        .unwrap();
        if check(
            formula.theory(),
            &candidate,
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap()
        .accepted()
        {
            models.insert(
                candidate
                    .atoms()
                    .map(|id| {
                        formula
                            .atoms()
                            .at(id)
                            .unwrap()
                            .to_atom(ValueLimits::default())
                            .unwrap()
                    })
                    .collect(),
            );
        }
    }
    models
}

#[test]
fn complete_source_scan_matches_original_ferraris_membership() {
    for source in [
        "",
        "a.",
        "a :- not b.",
        "c :- a. a :- not b.",
        "p(1). p(2). q(X) :- p(X), X > 1. r(X) :- p(X), not q(X).",
        "q :- a. q :- b. a. r :- q, not s.",
        "a. :- not a.",
        "a. :- a.",
        ":-.",
        "a. -a.",
        "-a. b :- -a. c :- not a.",
        "a :- b. c :- not a. :- a.",
        r"p(1). p(2). q(X) :- p(X), X \ 2 = 0. r(X) :- p(X), not q(X).",
    ] {
        let owner = admitted(source);
        let actual: BTreeSet<_> = interpretation(&owner).into_iter().collect();
        assert_eq!(actual, independent(source), "{source}");
        assert!(owner.statistics().work > 0);
    }
}

#[test]
fn alternative_producers_publish_only_after_their_dependencies() {
    // Authored order deliberately disagrees with dependency order; a false
    // alternative may not erase a true one, and a negative read sees them all.
    let owner = admitted("out :- not p. p :- absent. p :- q. q.");
    let names: BTreeSet<_> = interpretation(&owner)
        .unwrap()
        .into_iter()
        .map(|atom| atom.predicate().name().to_owned())
        .collect();
    assert_eq!(names, BTreeSet::from(["p".to_owned(), "q".to_owned()]));
}

#[test]
fn class_declines_recursion_choices_nested_bodies_and_observations() {
    for source in [
        "a :- a.",
        "a :- not b. b :- not a.",
        "{a}.",
        "a | b.",
        "a :- not not b.",
        "a :- #count {1:b} > 0.",
        "a. #minimize {1:a}.",
        "a. #project a/0.",
    ] {
        let preparation = prepared(source);
        let identity = std::ptr::from_ref(preparation.original_program());
        let SourceAdmission::NotApplicable(preparation) = preparation.try_source().unwrap() else {
            panic!("unqualified source admitted: {source}");
        };
        assert_eq!(std::ptr::from_ref(preparation.original_program()), identity);
        assert!(
            preparation.ground().is_ok(),
            "ordinary door retained: {source}"
        );
    }
}

#[test]
fn source_owner_preserves_text_typed_identity_and_is_send_sync() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<super::AcyclicSource>();
    let text = "a. b :- a.";
    let preparation = prepared(text);
    let identity = std::ptr::from_ref(preparation.original_program());
    let original = Arc::new(preparation.original_program().clone());
    let SourceAdmission::Admitted(owner) = preparation.try_source().unwrap() else {
        panic!()
    };
    assert_eq!(std::ptr::from_ref(owner.original_program()), identity);
    assert_eq!(owner.source().unwrap().text(), text);
    assert!(owner.same_instance(&owner.clone()));
    let typed = crate::prepare_program_formula(
        Arc::clone(&original),
        crate::ProgramAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    let SourceAdmission::Admitted(typed) = typed.try_source().unwrap() else {
        panic!()
    };
    assert!(std::ptr::eq(typed.original_program(), original.as_ref()));
    assert!(typed.source().is_none());
    assert!(!owner.same_instance(&typed));
    assert_eq!(interpretation(&owner), interpretation(&typed));
}

#[test]
fn formula_node_limits_are_not_spent_by_direct_source_evaluation() {
    let mut limits = FormulaLimits::default();
    limits.theory.max_nodes = 0;
    let preparation = prepare_formula(
        "a. b :- a.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        limits,
    )
    .unwrap();
    assert!(matches!(
        preparation.try_source().unwrap(),
        SourceAdmission::Admitted(_)
    ));
    assert!(
        prepare_formula(
            "a. b :- a.".into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            limits
        )
        .unwrap()
        .ground()
        .is_err()
    );
}

#[test]
fn original_family_errors_survive_false_bodies_and_constraints() {
    for source in [
        "p(2147483647). q(X) :- p(X), X+1 > 0, not absent.",
        "p(0). q(X) :- p(X), 1/X > 0.",
        "p(2147483647). :- . q(X) :- p(X), X+1 > 0.",
    ] {
        let expected = prepared(source).ground().unwrap_err();
        let actual = prepared(source).try_source().unwrap_err();
        assert_eq!(
            actual.cause().to_string(),
            expected.cause().to_string(),
            "{source}"
        );
        assert_eq!(
            actual.site(),
            expected.site(),
            "original statement remains exact"
        );
    }
}

#[test]
fn defined_false_sibling_retains_the_original_zero_warning() {
    let source = "p(0). p(1). q(X) :- p(X), 1/X > 1.";
    let formula = prepared(source).ground().unwrap();
    let owner = admitted(source);
    assert_eq!(owner.warnings(), formula.warnings());
    assert_eq!(
        interpretation(&owner).into_iter().collect::<BTreeSet<_>>(),
        independent(source)
    );
}
