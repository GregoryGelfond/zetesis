use std::collections::BTreeSet;

use super::*;
use crate::{ConstraintCheckLimits, ConstraintVerdict, HybridFormula};
use zetesis_core::Model;

fn groups(owner: &HybridFormula) -> usize {
    let view = owner.core_theory().view();
    (0..view.len())
        .filter(|&node| {
            matches!(view.node(node).unwrap(), Node::Threshold(row)
            if row.bound == 2 && row.weights.iter().all(|&weight| weight == 1))
        })
        .count()
}

fn family(owner: &HybridFormula) -> BTreeSet<Vec<usize>> {
    let atoms = owner.atom_catalog().atoms().len();
    assert!(atoms <= 7, "bounded complete family");
    let cancellation = Cancellation::default();
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    (0..1_usize << atoms)
        .filter_map(|mask| {
            let selected: Vec<_> = (0..atoms).filter(|atom| mask & (1 << atom) != 0).collect();
            let candidate = Interpretation::new(owner.core_theory(), selected.clone()).unwrap();
            let stable = zetesis_ferraris::check(
                owner.core_theory(),
                &candidate,
                Limits::default(),
                &cancellation,
            )
            .unwrap()
            .accepted();
            let model =
                Model::from_positions(owner.atom_catalog(), selected.iter().copied()).unwrap();
            let satisfied =
                checker.check(&model, &cancellation).unwrap() == ConstraintVerdict::Satisfied;
            (stable && satisfied).then_some(selected)
        })
        .collect()
}

fn compare(source: &str, templates: usize, instances: u64, native: usize) {
    let actual = crate::test_support::hybrid(source);
    let reference = without(|| crate::test_support::hybrid(source));
    assert_eq!(actual.streamed_templates(), templates, "{source}");
    assert_eq!(actual.streamed_instances(), instances, "{source}");
    assert_eq!(groups(&actual), native, "{source}");
    assert_eq!(groups(&reference), 0, "ordinary captured route");
    assert_eq!(
        actual.atom_catalog().atoms(),
        reference.atom_catalog().atoms()
    );
    assert_eq!(family(&actual), family(&reference), "{source}");
}

#[test]
fn capture_replaces_a_complete_template() {
    let source = "{p(1..3)}. :- p(X),p(Y),X!=Y.";
    compare(source, 0, 0, 1);
    let reference = without(|| crate::test_support::hybrid(source));
    assert_eq!(reference.streamed_templates(), 1);
    assert_eq!(
        reference.streamed_instances(),
        6,
        "all original pairs were captured"
    );
}

#[test]
fn guarded_facts_supply_exact_pair_bodies() {
    compare(
        "{p(1..3)}. :- p(X),p(Y),edge(X,Y). \
        edge(1,2). edge(1,3). edge(2,3).",
        0,
        0,
        1,
    );
}

#[test]
fn possible_guards_keep_the_original_stream() {
    compare("{p(1..3);g}. :- p(X),p(Y),g,X!=Y.", 1, 6, 0);
}

#[test]
fn a_missing_edge_keeps_the_original_stream() {
    compare("{p(1..4)}. :- p(X),p(Y),X<Y,X+Y<6.", 1, 4, 0);
}

#[test]
fn one_declined_component_keeps_the_whole_template() {
    let source = "{p(1,1);p(1,2);p(1,3);p(2,1);p(2,2)}. \
        :- p(G,X),p(G,Y),X!=Y.";
    compare(source, 1, 8, 0);
    assert_eq!(
        native_groups(&compile(source)).len(),
        1,
        "eager emission can independently replace the complete component"
    );
}

#[test]
fn retained_streams_keep_only_unreplaced_templates() {
    compare("{p(1..3);q}. :- p(X),p(Y),X!=Y. :- p(X),q.", 1, 3, 1);
}

#[test]
fn retained_hybrid_grounding_keeps_the_same_disposition() {
    let source = "{p(1..3)}. :- p(X),p(Y),X!=Y.";
    let grounded =
        crate::formula_ground::ground_retained(testing::prepare(source), None, true).unwrap();
    assert_eq!(native_groups(&grounded.compiled).len(), 1);
    let (rules, instances) = grounded.streamed.as_ref().expect("hybrid retained owner");
    assert!(rules.is_empty());
    assert_eq!(*instances, 0);
}

#[test]
fn declined_capture_preserves_scalar_failure_sites() {
    let source = "{p(0..2);g}. :- p(X),p(Y),g,X!=Y,A=1/X-(2147483647+Y).";
    let failure = || {
        crate::test_support::prepared(source)
            .ground_hybrid()
            .unwrap_err()
    };
    let actual = failure();
    let reference = without(failure);
    assert!(matches!(
        actual.cause(),
        crate::FormulaFailure::Expansion(crate::ExpansionFailure::Evaluation {
            error: crate::observation::EvaluationError::Overflow,
            ..
        })
    ));
    assert_eq!(format!("{actual:?}"), format!("{reference:?}"));
}

#[test]
fn declined_capture_keeps_mixed_zero_warnings() {
    let source = "{p(0..2);g}. :- p(X),p(Y),g,X!=Y,A=1/X.";
    compare(source, 1, 4, 0);
    let actual = crate::test_support::hybrid(source);
    let reference = without(|| crate::test_support::hybrid(source));
    assert!(matches!(
        actual.warnings(),
        [crate::FormulaWarning::ZeroDivisor { .. }]
    ));
    assert_eq!(actual.warnings(), reference.warnings());
}
