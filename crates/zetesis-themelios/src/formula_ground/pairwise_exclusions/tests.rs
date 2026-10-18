use std::cell::Cell;

use super::*;
use crate::formula::Compiled;
use crate::formula_ground::ground;
use crate::formula_support::testing;
use crate::test_support::compiled_atoms;
use crate::{ExpansionLimits, FormulaLimits};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, Limits, RegionLimits, models, models_reduct, producers};

thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(true) };
}

pub(super) fn enabled() -> bool {
    ENABLED.get()
}

fn without<T>(run: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ENABLED.set(self.0);
        }
    }
    let _restore = Restore(ENABLED.replace(false));
    run()
}

fn compile(source: &str) -> Compiled {
    ground(testing::prepare(source), None, None).unwrap()
}

fn native_groups(compiled: &Compiled) -> Vec<Vec<usize>> {
    let view = compiled.theory.view();
    (0..view.len())
        .filter_map(|node| match view.node(node).unwrap() {
            Node::Threshold(row) if row.bound == 2 && row.weights.iter().all(|&w| w == 1) => {
                Some(row.conditions.to_vec())
            }
            _ => None,
        })
        .collect()
}

fn interpretations(compiled: &Compiled) -> Vec<Interpretation> {
    let atoms = compiled_atoms(compiled);
    assert!(atoms.len() <= 6, "bounded exhaustive population");
    (0..1_usize << atoms.len())
        .map(|bits| {
            Interpretation::new(
                &compiled.theory,
                (0..atoms.len()).filter(|&atom| bits & (1 << atom) != 0),
            )
            .unwrap()
        })
        .collect()
}

const CASES: &[&str] = &[
    "{p(0..2)}. :- p(X),p(Y),X!=Y,A=1/X.",
    "{p(1..3)}. :- p(X),p(Y),X!=Y.",
    "{p(1..3)}. :- p(X),p(Y),X<Y.",
    "{p(1..4)}. :- p(X),p(Y),X<Y,X+Y<6.",
    "{p(1..3);q(1..3)}. :- p(X),p(Y),X<Y. :- q(X),q(Y),X<Y.",
    "{-p(1);-p(2);-p(3);p(1)}. :- -p(X),-p(Y),X!=Y.",
    "{p(1..3)}. :- p(X),p(X),p(Y),X<Y.",
    "{p(1..3);q}. :- p(X),p(Y),q,X<Y.",
    "{p(1..3);q}. :- p(X),p(Y),not q,X<Y.",
    "{p(1..3)}. :- p(X),p(Y),X<=Y.",
    "{p(1..3)}. :- p(X),p(Y),edge(X,Y). edge(1,2). edge(1,3). edge(2,3).",
    "{p(1..3)}. :- p(X),p(Y),Z=X+1,Z<=Y.",
];

#[test]
fn complete_exclusions_preserve_original_truth() {
    let cancellation = Cancellation::default();
    for source in CASES {
        let actual = compile(source);
        let reference = without(|| compile(source));
        assert_eq!(
            compiled_atoms(&actual),
            compiled_atoms(&reference),
            "{source}"
        );
        for (actual_i, reference_i) in interpretations(&actual)
            .iter()
            .zip(interpretations(&reference))
        {
            assert_eq!(
                models(&actual.theory, actual_i, Limits::default(), &cancellation).unwrap(),
                models(
                    &reference.theory,
                    &reference_i,
                    Limits::default(),
                    &cancellation
                )
                .unwrap(),
                "{source}",
            );
        }
    }
}

#[test]
fn complete_exclusions_preserve_arbitrary_frozen_truth() {
    let cancellation = Cancellation::default();
    for source in CASES {
        let actual = compile(source);
        let reference = without(|| compile(source));
        assert_eq!(
            compiled_atoms(&actual),
            compiled_atoms(&reference),
            "{source}"
        );
        let actual_i = interpretations(&actual);
        let reference_i = interpretations(&reference);
        for (candidate, actual_m) in actual_i.iter().enumerate() {
            for (tested, actual_j) in actual_i.iter().enumerate() {
                assert_eq!(
                    models_reduct(
                        &actual.theory,
                        actual_m,
                        actual_j,
                        Limits::default(),
                        &cancellation
                    )
                    .unwrap(),
                    models_reduct(
                        &reference.theory,
                        &reference_i[candidate],
                        &reference_i[tested],
                        Limits::default(),
                        &cancellation
                    )
                    .unwrap(),
                    "M={candidate}, J={tested}: {source}",
                );
            }
        }
    }
}

#[test]
fn certified_rows_replace_the_pair_nodes() {
    let source = "{p(1..4)}. :- p(X),p(Y),X!=Y.";
    let actual = compile(source);
    let reference = without(|| compile(source));
    assert_eq!(
        native_groups(&actual)
            .iter()
            .map(Vec::len)
            .collect::<Vec<_>>(),
        vec![4]
    );
    assert!(native_groups(&reference).is_empty());
    let pair_nodes = |compiled: &Compiled| {
        let view = compiled.theory.view();
        (0..view.len())
            .filter(|&node| {
                matches!(view.node(node).unwrap(),
            Node::And(children) if children.len() == 2 && children.iter().all(|&child|
                matches!(view.node(child).unwrap(), Node::Atom(_))))
            })
            .count()
    };
    assert_eq!(pair_nodes(&actual), 0, "replaced nodes are never allocated");
    assert!(pair_nodes(&reference) > 0);
    assert!(actual.theory.nodes().len() < reference.theory.nodes().len());
    assert!(actual.theory.roots().len() < reference.theory.roots().len());
}

#[test]
fn missing_edges_cannot_be_supplied_by_duplicates() {
    // Both directions of four edges form a triangle with one attached vertex;
    // the absent edges still forbid certifying the four-atom component.
    let source = "{p(1..4)}. :- p(X),p(Y),X!=Y,X+Y<6.";
    assert!(native_groups(&compile(source)).is_empty());
}

#[test]
fn coherence_does_not_join_distinct_source_groups() {
    let source = "{p(1..3);-p(1..3)}. \
        :- p(X),p(Y),X!=Y. :- -p(X),-p(Y),X!=Y.";
    let compiled = compile(source);
    let groups = native_groups(&compiled);
    assert_eq!(groups.len(), 2);
    assert!(groups.iter().all(|group| group.len() == 3));
    assert!(groups[0].iter().all(|node| !groups[1].contains(node)));
}

#[test]
fn a_native_constraint_keeps_complete_producer_knowledge() {
    let compiled = compile("{p(1..3)}. :- p(X),p(Y),X!=Y.");
    assert_eq!(native_groups(&compiled).len(), 1, "native route exercised");
    assert!(
        producers(
            &compiled.theory,
            RegionLimits::default(),
            &Cancellation::default()
        )
        .unwrap()
        .producers
        .is_some()
    );
}

#[test]
fn replacement_roots_keep_source_evidence() {
    let source = "{p(1..3)}. :- p(X),p(Y),X!=Y.";
    let actual = compile(source);
    let reference = without(|| compile(source));
    let view = actual.theory.view();
    let replaced = actual
        .theory
        .roots()
        .iter()
        .position(|&root| {
            matches!(
                view.node(root).unwrap(), Node::Implies(body, super::super::FALSUM)
                    if matches!(view.node(body).unwrap(), Node::Threshold(_))
            )
        })
        .expect("native exclusion root");
    let view = reference.theory.view();
    let mut witnesses = 0;
    for (index, &root) in reference.theory.roots().iter().enumerate() {
        if matches!(view.node(root).unwrap(), Node::Implies(body, super::super::FALSUM)
            if matches!(view.node(body).unwrap(), Node::And(_)))
        {
            assert_eq!(actual.origins[replaced], reference.origins[index]);
            witnesses += 1;
        }
    }
    assert_eq!(
        witnesses, 6,
        "every ordered source instance supplies the same evidence"
    );
}

#[test]
fn pair_collection_keeps_independent_scalar_failures() {
    for source in [
        "{p(0..2)}. :- p(X),p(Y),X!=Y,A=1/X-(2147483647+Y).",
        "{p(1..3)}. :- p(X),p(Y),X<Y,A=X+2147483647.",
    ] {
        let failure = || {
            crate::admit_formula(
                source.into(),
                crate::AdmissionOptions::default(),
                ExpansionLimits::default(),
                FormulaLimits::default(),
            )
            .expect_err("reached scalar failure")
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
}

#[test]
fn pair_collection_keeps_mixed_zero_warnings() {
    // Dropping the X=0 substitutions leaves all unordered pairs covered by
    // the surviving orientations; the family warns but remains defined.
    let source = "{p(0..2)}. :- p(X),p(Y),X!=Y,A=1/X.";
    let actual = compile(source);
    let reference = without(|| compile(source));
    assert_eq!(native_groups(&actual).len(), 1);
    assert!(native_groups(&reference).is_empty());
    assert!(matches!(
        actual.warnings.as_slice(),
        [crate::FormulaWarning::ZeroDivisor { .. }]
    ));
    assert_eq!(actual.warnings, reference.warnings);
}

#[test]
fn native_exclusions_keep_positive_and_tight_certificates() {
    use zetesis_ferraris::{PositivePlan, PositivePlanLimits, TightPlan, TightPlanLimits};

    // Derived p atoms survive explicit-fact substitution. The violated
    // constraint is a valid completed plan result, not a grammar refusal.
    let compiled = compile("d(1..3). p(X):-d(X). :-p(X),p(Y),X!=Y.");
    let groups = native_groups(&compiled);
    assert_eq!(groups.len(), 1, "native exclusion route exercised");
    assert_eq!(groups[0].len(), 3);
    let cancellation = Cancellation::default();
    let positive = PositivePlan::compile(
        &compiled.theory,
        PositivePlanLimits::default(),
        &cancellation,
    )
    .expect("native constraints preserve positive-plan eligibility");
    assert!(positive.failed_constraint().is_some());
    TightPlan::compile(&compiled.theory, TightPlanLimits::default(), &cancellation)
        .expect("native constraints preserve tight-plan eligibility");
}

mod hybrid;
mod planning;
