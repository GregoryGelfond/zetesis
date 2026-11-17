//! The source subject is checked against the independent complete formula door.

use crate::test_support::{prepared, zero_divisor_statements};
use crate::{AdmissionOptions, ExpansionLimits, FormulaLimits, SourceAdmission, prepare_formula};
use std::{collections::BTreeSet, sync::Arc};
use zetesis_algebra::Cancellation;
use zetesis_core::{Atom, ValueLimits};
use zetesis_ferraris::{Interpretation, Limits, check};
use zetesis_test_support::programs::unary;

fn admitted(source: &str) -> super::AcyclicSource {
    let SourceAdmission::Admitted(owner) = prepared(source).try_source().unwrap() else {
        panic!("expected complete signed stratified normal source: {source}");
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
fn definite_recursion_matches_original_ferraris_membership() {
    // Definite recursion: no default negation in the component or anything
    // it reads. Negation may read the completed recursive relation from
    // above, and constraints may reject it.
    for source in [
        "a :- a.",
        "a :- b. b :- a. c.",
        "c. a :- c. a :- b. b :- a.",
        "e(1,2). e(2,1). r(X,Y) :- e(X,Y). r(X,Z) :- e(X,Y), r(Y,Z).",
        "s(1). s(2). s(3). e(1,2). e(2,1). r(X) :- e(1,X). r(Y) :- r(X), e(X,Y). \
         u(X) :- s(X), not r(X).",
        "e(1,2). e(2,3). r(1). r(Y) :- r(X), e(X,Y), Y > X. :- r(3).",
        "e(1,2). e(2,3). r(1). r(Y) :- r(X), e(X,Y), Y > X + 1.",
    ] {
        let owner = admitted(source);
        let actual: BTreeSet<_> = interpretation(&owner).into_iter().collect();
        assert_eq!(actual, independent(source), "{source}");
    }
}

#[test]
fn stratified_recursion_matches_original_ferraris_membership() {
    // Recursion through certain components that negate completed lower
    // strata, including nested strata and constraints.
    for source in [
        "p :- q. q :- p, not r. r :- not s.",
        "r :- not s. p :- r. p :- p.",
        "e(1,2). e(2,3). b(3). r(1). r(Y) :- r(X), e(X,Y), not b(Y).",
        "e(1,2). e(2,3). b(2). r(1). r(Y) :- r(X), e(X,Y). u(X) :- r(X), not b(X).",
        "e(1,2). e(2,3). b(2). u(1). u(3). w(Y) :- u(Y), not b(Y). \
         w(Y) :- w(X), e(X,Y), not b(Y). :- w(3).",
        "e(1,2). e(2,1). b(2). n(1). n(2). \
         r(X) :- n(X), not b(X). r(Y) :- r(X), e(X,Y), not b(Y).",
    ] {
        let owner = admitted(source);
        let actual: BTreeSet<_> = interpretation(&owner).into_iter().collect();
        assert_eq!(actual, independent(source), "{source}");
    }
}

#[test]
fn recursion_through_negation_declines_the_class() {
    for source in ["p :- not q. q :- not p.", "p :- q. q :- p, not p."] {
        let SourceAdmission::NotApplicable(preparation) = prepared(source).try_source().unwrap()
        else {
            panic!("recursion through negation admitted: {source}");
        };
        assert!(preparation.ground().is_ok(), "{source}");
    }
}

#[test]
fn a_row_a_decided_negation_blocks_names_no_head() {
    // `q` is certain and complete when the rule of `r` is read, so the row
    // `X = 1` is passed over as support completion passed over it: `r(1)`
    // is no atom of the published catalog, whose four atoms all hold. The
    // row `X = 2` derives `r(2)`.
    let owner = admitted("p(1). p(2). q(1). r(X) :- p(X), not q(X).");
    let published: BTreeSet<Atom> = owner
        .atom_catalog()
        .atoms()
        .iter()
        .map(|atom| atom.to_atom(ValueLimits::default()).unwrap())
        .collect();
    assert_eq!(published.len(), 4);
    assert_eq!(Some(published), interpretation(&owner));
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
fn class_declines_negative_cycles_choices_nested_bodies_and_observations() {
    for source in [
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
fn an_entirely_undefined_family_retains_its_zero_warning() {
    // The rule's one instance divides by zero and is omitted: `q` has no
    // atom, by the source scan and by the formula door alike.
    let source = "p(0). q(X) :- p(X), 1/X > 0.";
    let formula = prepared(source).ground().unwrap();
    let owner = admitted(source);
    assert_eq!(
        zero_divisor_statements(source, owner.warnings()),
        ["q(X) :- p(X), 1/X > 0."]
    );
    assert_eq!(owner.warnings(), formula.warnings());
    let expected = BTreeSet::from([BTreeSet::from([unary("p", 0)])]);
    assert_eq!(
        interpretation(&owner).into_iter().collect::<BTreeSet<_>>(),
        expected
    );
    assert_eq!(independent(source), expected);
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
