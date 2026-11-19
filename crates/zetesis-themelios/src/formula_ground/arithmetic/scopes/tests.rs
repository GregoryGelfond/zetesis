//! Empty-scope reuse is scoped to one immutable source fragment and succeeds
//! only after the reference traversal; no binding or family evidence is reused.

use std::cell::Cell;

use super::*;
use crate::formula::Compiled;
use crate::formula_ground::ground;
use crate::formula_support::testing;
use crate::test_support::compiled_atoms;
use crate::{FormulaLimits, FormulaResource};
use zetesis_algebra::{Cancellation, Stop};

thread_local! {
    static REFERENCE: Cell<bool> = const { Cell::new(false) };
}

struct Restore(bool);
impl Drop for Restore {
    fn drop(&mut self) {
        REFERENCE.set(self.0);
    }
}

pub(super) fn reference() -> bool {
    REFERENCE.get()
}

pub(super) fn controlled<T>(reference: bool, run: impl FnOnce() -> T) -> T {
    let _restore = Restore(REFERENCE.replace(reference));
    run()
}

const FLAT: &str = "a.b.#minimize{1:a,b}.";

#[test]
fn a_flat_source_fragment_is_scanned_once_without_storage_or_evidence() {
    eprintln!("empty-scope certificate bytes={}", size_of::<Scopes<'_>>());
    testing::with_completed_source(FLAT, |program, support, computation, counters| {
        let objective = &program.objectives[0];
        let literals = objective.condition.literals();
        assert_eq!(literals.len(), 2);
        let limits = FormulaLimits::default();
        let binding = Binding::new(computation, &limits, counters, objective.location).unwrap();
        let mut budget = testing::budget();
        let mut warnings = Warnings::default();
        let work = counters.accounting.work;
        let live = counters.workspace_bytes();
        let mut scopes = Scopes::new(literals);
        assert!(scopes.pending.is_some(), "construction proves nothing");
        assert_eq!(counters.accounting.work, work);
        for _ in 0..4 {
            scopes
                .validate(
                    &binding,
                    support,
                    &mut budget,
                    &mut warnings,
                    Context::new(&mut *computation, &limits, counters, objective.location),
                )
                .unwrap();
            assert!(scopes.pending.is_none());
            assert_eq!(counters.accounting.work - work, 2);
            assert_eq!(counters.workspace_bytes(), live);
        }
        assert!(warnings.into_values().is_empty());
    });
}

#[test]
fn every_incomplete_first_scan_keeps_its_exact_work_stop_and_must_retry() {
    for remaining in 0..2 {
        testing::with_completed_source(FLAT, |program, support, computation, counters| {
            let objective = &program.objectives[0];
            let ordinary = FormulaLimits::default();
            let binding =
                Binding::new(computation, &ordinary, counters, objective.location).unwrap();
            let mut budget = testing::budget();
            let mut warnings = Warnings::default();
            let before = counters.accounting.work;
            let limited = FormulaLimits {
                max_work: before + remaining,
                ..ordinary
            };
            let mut scopes = Scopes::new(objective.condition.literals());
            let result = scopes.validate(
                &binding,
                support,
                &mut budget,
                &mut warnings,
                Context::new(&mut *computation, &limited, counters, objective.location),
            );
            assert!(matches!(result, Err(FormulaFailure::Limit {
                resource: FormulaResource::Work, location, ..
            }) if location == objective.location));
            assert_eq!(counters.accounting.work - before, remaining);
            assert!(scopes.pending.is_some());
            let retry = counters.accounting.work;
            scopes
                .validate(
                    &binding,
                    support,
                    &mut budget,
                    &mut warnings,
                    Context::new(&mut *computation, &ordinary, counters, objective.location),
                )
                .unwrap();
            assert_eq!(counters.accounting.work - retry, 2);
            assert!(scopes.pending.is_none());
        });
    }
}

#[test]
fn cancellation_precedes_first_scan_refusal_and_does_not_publish_absence() {
    testing::with_completed_source(FLAT, |program, support, computation, counters| {
        let objective = &program.objectives[0];
        let ordinary = FormulaLimits::default();
        let binding = Binding::new(computation, &ordinary, counters, objective.location).unwrap();
        let cancellation = Cancellation::default();
        cancellation.cancel();
        *counters = std::mem::take(counters).with_cancellation(Some(&cancellation));
        let before = counters.accounting.work;
        let limited = FormulaLimits {
            max_work: before,
            ..ordinary
        };
        let mut scopes = Scopes::new(objective.condition.literals());
        let result = scopes.validate(
            &binding,
            support,
            &mut testing::budget(),
            &mut Warnings::default(),
            Context::new(&mut *computation, &limited, counters, objective.location),
        );
        assert!(matches!(
            result,
            Err(FormulaFailure::Interrupted {
                reason: Stop::Cancelled,
                ..
            })
        ));
        assert_eq!(counters.accounting.work, before);
        assert!(scopes.pending.is_some());
        *counters = std::mem::take(counters).with_cancellation(None);
        scopes
            .validate(
                &binding,
                support,
                &mut testing::budget(),
                &mut Warnings::default(),
                Context::new(&mut *computation, &ordinary, counters, objective.location),
            )
            .unwrap();
        assert_eq!(counters.accounting.work - before, 2);
        assert!(scopes.pending.is_none());
    });
}

#[test]
fn a_certified_empty_scan_has_no_charge_or_poll_but_does_not_clear_cancellation() {
    testing::with_completed_source(FLAT, |program, support, computation, counters| {
        let objective = &program.objectives[0];
        let ordinary = FormulaLimits::default();
        let binding = Binding::new(computation, &ordinary, counters, objective.location).unwrap();
        let mut scopes = Scopes::new(objective.condition.literals());
        scopes
            .validate(
                &binding,
                support,
                &mut testing::budget(),
                &mut Warnings::default(),
                Context::new(&mut *computation, &ordinary, counters, objective.location),
            )
            .unwrap();
        let cancellation = Cancellation::default();
        cancellation.cancel();
        *counters = std::mem::take(counters).with_cancellation(Some(&cancellation));
        let before = counters.accounting.work;
        let limited = FormulaLimits {
            max_work: before,
            ..ordinary
        };
        scopes
            .validate(
                &binding,
                support,
                &mut testing::budget(),
                &mut Warnings::default(),
                Context::new(&mut *computation, &limited, counters, objective.location),
            )
            .unwrap();
        assert_eq!(counters.accounting.work, before);
        let mut fresh = Scopes::new(objective.condition.literals());
        assert!(matches!(
            fresh.validate(
                &binding,
                support,
                &mut testing::budget(),
                &mut Warnings::default(),
                Context::new(&mut *computation, &limited, counters, objective.location)
            ),
            Err(FormulaFailure::Interrupted {
                reason: Stop::Cancelled,
                ..
            })
        ));
        assert!(matches!(
            counters.work(&limited, objective.location),
            Err(FormulaFailure::Interrupted {
                reason: Stop::Cancelled,
                ..
            })
        ));
    });
}

#[test]
fn each_fragment_owns_its_absence_proof() {
    testing::with_completed_source(FLAT, |program, support, computation, counters| {
        let objective = &program.objectives[0];
        let limits = FormulaLimits::default();
        let binding = Binding::new(computation, &limits, counters, objective.location).unwrap();
        let mut left = Scopes::new(objective.condition.literals());
        let right = Scopes::new(objective.condition.literals());
        left.validate(
            &binding,
            support,
            &mut testing::budget(),
            &mut Warnings::default(),
            Context::new(&mut *computation, &limits, counters, objective.location),
        )
        .unwrap();
        assert!(left.pending.is_none());
        assert!(right.pending.is_some());
    });
}

#[test]
fn an_empty_aggregate_still_requires_each_outer_bindings_validation() {
    testing::with_completed_source(
        ":~#count{}=0.[1]",
        |program, support, computation, counters| {
            let objective = &program.objectives[0];
            assert!(matches!(
                objective.condition.literals(),
                [LiteralIr::Aggregate(_)]
            ));
            let limits = FormulaLimits::default();
            let binding = Binding::new(computation, &limits, counters, objective.location).unwrap();
            let mut scopes = Scopes::new(objective.condition.literals());
            let before = counters.accounting.work;
            for completed in 1..=3 {
                scopes
                    .validate(
                        &binding,
                        support,
                        &mut testing::budget(),
                        &mut Warnings::default(),
                        Context::new(&mut *computation, &limits, counters, objective.location),
                    )
                    .unwrap();
                assert!(scopes.pending.is_some());
                assert_eq!(counters.accounting.work - before, completed);
            }
        },
    );
}

#[test]
fn conditionals_and_nested_witness_projection_never_cache_by_first_binding() {
    for (source, conditional) in [
        ("d(1).q(1).:~q(X):d(X).[1]", true),
        ("p(f(1)).:~not p(f(_)).[1]", false),
    ] {
        testing::with_completed_source(source, |program, support, computation, counters| {
            let objective = &program.objectives[0];
            assert!(
                objective
                    .condition
                    .literals()
                    .iter()
                    .any(|literal| if conditional {
                        matches!(literal, LiteralIr::Conditional(_))
                    } else {
                        matches!(
                            literal,
                            LiteralIr::ProjectedAtom(
                                _,
                                super::super::ExistentialProjection::Witnesses { .. }
                            )
                        )
                    }),
                "fixture reaches its scope-bearing source form: {source}"
            );
            let limits = FormulaLimits::default();
            let binding = Binding::new(computation, &limits, counters, objective.location).unwrap();
            let mut scopes = Scopes::new(objective.condition.literals());
            for _ in 0..2 {
                let before = counters.accounting.work;
                scopes
                    .validate(
                        &binding,
                        support,
                        &mut testing::budget(),
                        &mut Warnings::default(),
                        Context::new(&mut *computation, &limits, counters, objective.location),
                    )
                    .unwrap();
                assert!(scopes.pending.is_some());
                assert!(counters.accounting.work > before);
            }
        });
    }
}

#[test]
fn flat_anonymous_projection_is_absent_only_from_nested_scope_scanning() {
    testing::with_completed_source(
        "p(1).:~not p(_).[1]",
        |program, support, computation, counters| {
            let objective = &program.objectives[0];
            assert!(matches!(
                objective.condition.literals(),
                [LiteralIr::ProjectedAtom(
                    _,
                    super::super::ExistentialProjection::Arguments { .. }
                )]
            ));
            let limits = FormulaLimits::default();
            let binding = Binding::new(computation, &limits, counters, objective.location).unwrap();
            let mut scopes = Scopes::new(objective.condition.literals());
            scopes
                .validate(
                    &binding,
                    support,
                    &mut testing::budget(),
                    &mut Warnings::default(),
                    Context::new(&mut *computation, &limits, counters, objective.location),
                )
                .unwrap();
            assert!(scopes.pending.is_none());
        },
    );
}

fn assert_same_program(before: &Compiled, after: &Compiled) {
    assert_eq!(compiled_atoms(before), compiled_atoms(after));
    assert_eq!(before.warnings, after.warnings);
    assert_eq!(before.theory.nodes(), after.theory.nodes());
    assert_eq!(before.theory.operands(), after.theory.operands());
    assert_eq!(before.theory.roots(), after.theory.roots());
    assert_eq!(before.origins, after.origins);
    assert!(
        before
            .objectives
            .templates()
            .iter()
            .eq(after.objectives.templates().iter())
    );
    assert_eq!(
        before.objectives.priorities(),
        after.objectives.priorities()
    );
    assert_eq!(before.objective_origins, after.objective_origins);
    assert_eq!(before.objective_declarations, after.objective_declarations);
}

#[test]
fn both_objective_consumers_retain_exact_theory_templates_and_source_origins() {
    for source in [
        "d(1;2).e(3;4).#minimize{X@0,X,Y:d(X),e(Y)}.",
        "d(1;2).e(3;4).#minimize{X+1@0,X,Y:d(X),e(Y)}.",
        "d(0;1).#minimize{1/X:d(X)}.",
        "d(0;1).#minimize{1,1/X:d(X)}.",
        "{a;b}.#minimize{2@1,k:a;2@1,k:b}.",
        "d(1;2).{p(1);p(2)}.:~d(K),N=#count{X:p(X),X=K}.[N@K,K]",
        "d(1;2).q(1).:~q(X):d(X).[1]",
        "p(f(1)).:~not p(f(_)).[1]",
    ] {
        let before = controlled(true, || ground(testing::prepare(source), None, None)).unwrap();
        let after = controlled(false, || ground(testing::prepare(source), None, None)).unwrap();
        assert_same_program(&before, &after);
    }
}

#[test]
fn the_cache_cannot_hide_independent_scalar_field_failures() {
    for source in [
        "d(0;1).#minimize{2147483647+X:d(X)}.",
        "d(0;1).#minimize{1,2147483647+X:d(X)}.",
    ] {
        let before = controlled(true, || ground(testing::prepare(source), None, None)).unwrap_err();
        let after = controlled(false, || ground(testing::prepare(source), None, None)).unwrap_err();
        assert!(matches!(
            after,
            FormulaFailure::Expansion(crate::ExpansionFailure::Evaluation { .. })
        ));
        assert_eq!(format!("{before:?}"), format!("{after:?}"));
    }
}

#[test]
fn the_reference_control_restores_after_unwind() {
    assert!(std::panic::catch_unwind(|| controlled(true, || panic!("test unwind"))).is_err());
    assert!(!reference());
}
