//! Exercise the actual `instantiate_rule` dispatch, with its retained family owner.

use std::num::NonZeroUsize;

use super::*;
use crate::formula_ground::{Purpose, Schedule};
use crate::formula_support::{Counters, SourceExecution, testing, with_source_workers};
use crate::{ExpansionLimits, FormulaLimits, FormulaResource, ProgramSite};
use zetesis_algebra::{Cancellation, Stop};
use zetesis_core::{Atom, ValueLimits};

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    atoms: Vec<Atom>,
    atom_origins: Vec<Vec<ProgramSite>>,
    roots: Vec<usize>,
    root_origins: Vec<Vec<ProgramSite>>,
    nodes: Vec<String>,
}

struct Attempt {
    result: Result<Snapshot, FormulaFailure>,
    work: u64,
    substitutions: u64,
    tasks: usize,
}

#[derive(Default)]
struct Bound {
    work: Option<u64>,
    bytes: Option<usize>,
    cancelled: bool,
}

fn snapshot(
    builder: &mut Builder<'_, '_, '_>,
    location: ProgramSite,
) -> Result<Snapshot, FormulaFailure> {
    let mut atoms = Vec::new();
    let mut atom_origins = Vec::new();
    for index in 0..builder.catalog.len() {
        atoms.push(
            builder
                .catalog
                .get(
                    index,
                    builder.computation,
                    builder.counters,
                    builder.limits,
                    location,
                )?
                .to_atom(ValueLimits::default())
                .unwrap(),
        );
        atom_origins.push(builder.metadata.origins(index).collect());
    }
    Ok(Snapshot {
        atoms,
        atom_origins,
        roots: builder.roots.clone(),
        root_origins: builder.origins.clone(),
        nodes: (0..builder.nodes.view().len())
            .map(|index| format!("{:?}", builder.nodes.view().node(index).unwrap()))
            .collect(),
    })
}

fn selected_source<'source>(
    prepared: &'source crate::formula_ir::Prepared,
    support: &crate::formula_support::Support<'source>,
    computation: &mut crate::formula_support::Computation<'_, 'source>,
    counters: &mut Counters,
    limits: &FormulaLimits,
    execution: &SourceExecution,
) -> Result<Snapshot, FormulaFailure> {
    let mut budget = crate::expansion::Budget::new(ExpansionLimits::default(), usize::MAX);
    let location = prepared.rules_without_facts().last().unwrap().location;
    let mut builder = Builder::empty(
        computation,
        limits,
        &mut budget,
        counters,
        Purpose::Theory,
        None,
        location,
    )?;
    builder.executor = execution.executor();
    builder.initialize(location)?;
    for (index, rule) in prepared.rules_without_facts().iter().enumerate() {
        if !rule.body.is_empty() {
            builder.instantiate_rule(rule, index, None, support, Schedule::Eager(None))?;
        }
    }
    snapshot(&mut builder, location)
}

fn run(source: &str, workers: Option<usize>, bound: &Bound) -> Attempt {
    let mut attempt = None;
    let mut action = || {
        testing::with_completed_source(source, |prepared, support, computation, counters| {
            let before = (counters.accounting.work, counters.accounting.substitutions);
            let limits = FormulaLimits {
                max_work: bound
                    .work
                    .map_or(FormulaLimits::default().max_work, |work| before.0 + work),
                max_support_bytes: bound
                    .bytes
                    .unwrap_or(FormulaLimits::default().max_support_bytes),
                ..FormulaLimits::default()
            };
            let cancellation = Cancellation::default();
            let execution = SourceExecution::new();
            if bound.cancelled {
                cancellation.cancel();
            }
            counters.attach_test_control(&cancellation);
            let result = selected_source(
                prepared,
                support,
                computation,
                counters,
                &limits,
                &execution,
            );
            counters.detach_test_control();
            attempt = Some(Attempt {
                result,
                work: counters.accounting.work - before.0,
                substitutions: counters.accounting.substitutions - before.1,
                tasks: 0,
            });
        });
    };
    let tasks = if let Some(workers) = workers {
        with_source_workers(NonZeroUsize::new(workers).unwrap(), action).1
    } else {
        action();
        0
    };
    let mut attempt = attempt.unwrap();
    attempt.tasks = tasks;
    attempt
}

fn same(source: &str) {
    let serial = run(source, None, &Bound::default()).result.unwrap();
    let one = run(source, Some(1), &Bound::default());
    let many = run(source, Some(14), &Bound::default());
    assert!(one.tasks > 0, "actual final emission tasks: {source}");
    assert_eq!(one.tasks, many.tasks);
    assert_eq!(
        (one.work, one.substitutions),
        (many.work, many.substitutions)
    );
    assert_eq!(one.result.unwrap(), serial, "one worker: {source}");
    assert_eq!(many.result.unwrap(), serial, "fourteen workers: {source}");
}

#[test]
fn every_activation_and_origin_survives_an_already_supported_head() {
    same(
        "p(1;2;3).q(1;2;3).r(1;2;3).k(7;8).h(X,Y):-p(X),q(X),k(Y).h(X,Y):-p(X),r(X),k(Y).h(X,Y):-p(X),q(X),k(Y).",
    );
}

#[test]
fn repeated_variables_classical_negation_and_flat_constants_preserve_rows() {
    for source in [
        "p(1;2).q(1;2).k(7;8).h(X,X,Y):-p(X),p(X),q(X),k(Y).",
        "-p(1;2).q(1;2).k(7;8).-h(X,7,X,Y):- -p(X),q(X),k(Y).",
        "p(f(1);f(2)).q(f(1);f(2)).k(7;8).h(X,Y):-p(X),q(X),k(Y).",
    ] {
        same(source);
    }
}

#[test]
fn wide_multiblock_bodies_keep_exact_balanced_nodes_and_source_roots() {
    same(
        "p(1..18).q(1;2).r(1;2).s(1;2).t(1;2).u(1;2).h(A,B,C,D,E,F):-p(A),q(B),r(C),s(D),t(E),u(F).",
    );
}

#[test]
fn no_selected_descendant_publishes_no_vocabulary() {
    let source = "p(1;2;3).q(4;5).k(7;8).h(X,Y):-p(X),q(X),k(Y).";
    same(source);
    let result = run(source, Some(14), &Bound::default()).result.unwrap();
    assert!(result.atoms.is_empty());
    assert!(result.roots.is_empty());
    assert_eq!(result.nodes.len(), 2);
}

#[test]
fn rich_consumers_stay_on_the_original_serial_path() {
    for source in [
        "p(1;2).q(1;2).h(X):-p(X),q(X),X<2.",
        "p(1;2).q(1;2).h(X+1):-p(X),q(X).",
        "p(1;2).q(1;2).h(f(X)):-p(X),q(X).",
    ] {
        let reference = run(source, None, &Bound::default()).result.unwrap();
        let actual = run(source, Some(14), &Bound::default());
        assert_eq!(actual.tasks, 0, "fallback: {source}");
        assert_eq!(actual.result.unwrap(), reference);
    }
}

#[test]
fn cancellation_and_entry_refusal_keep_typed_failure_without_source_publication() {
    let source = "p(1..16).q(1..16).h(X,Y):-p(X),q(Y).";
    let cancelled = run(
        source,
        Some(14),
        &Bound {
            cancelled: true,
            ..Bound::default()
        },
    );
    assert_eq!(
        cancelled.result.unwrap_err().interruption(),
        Some(Stop::Cancelled)
    );
    assert_eq!(cancelled.tasks, 0);
    for work in [0, 1] {
        let refused = run(
            source,
            Some(14),
            &Bound {
                work: Some(work),
                ..Bound::default()
            },
        );
        assert!(matches!(
            refused.result,
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Work,
                ..
            })
        ));
        assert_eq!(refused.work, work);
        assert_eq!(refused.tasks, 0);
    }
    let bytes = run(
        source,
        Some(14),
        &Bound {
            bytes: Some(0),
            ..Bound::default()
        },
    );
    assert!(matches!(
        bytes.result,
        Err(FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes,
            ..
        })
    ));
    assert_eq!(bytes.tasks, 0);
}

#[test]
fn actual_complete_preparation_preserves_original_and_every_frozen_truth() {
    use zetesis_ferraris::{Interpretation, Limits, models, models_reduct};
    for (source, staged) in [
        ("{p(1);q(1);r(1)}.h(X):-p(X),q(X).h(X):-p(X),r(X).", false),
        ("{-p(1);q(1)}.-h(X,7,X):- -p(X),q(X).", false),
        (
            "edge(1,2).edge(2,3).edge(3,1).{p(1)}.h(X):-p(X),edge(X,Y),edge(Y,Z),edge(Z,X).",
            true,
        ),
        (
            "edge(1,2).edge(2,3).edge(3,1).{-p(1)}.-h(X,7,X):- -p(X),edge(X,Y),edge(Y,Z),edge(Z,X).",
            true,
        ),
    ] {
        let serial = crate::formula_ground::ground(testing::prepare(source), None, None).unwrap();
        let atoms = crate::test_support::compiled_atoms(&serial);
        assert!(atoms.len() <= 6);
        for workers in [1, 14] {
            let (parallel, tasks) =
                with_source_workers(NonZeroUsize::new(workers).unwrap(), || {
                    crate::formula_ground::ground(testing::prepare(source), None, None).unwrap()
                });
            assert_eq!(tasks > 0, staged, "selected witness route: {source}");
            assert_eq!(crate::test_support::compiled_atoms(&parallel), atoms);
            let cancellation = Cancellation::default();
            for candidate in 0..1 << atoms.len() {
                let selected =
                    |bits: usize| (0..atoms.len()).filter(move |index| bits & (1 << index) != 0);
                let left = Interpretation::new(&serial.theory, selected(candidate)).unwrap();
                let right = Interpretation::new(&parallel.theory, selected(candidate)).unwrap();
                assert_eq!(
                    models(&serial.theory, &left, Limits::default(), &cancellation).unwrap(),
                    models(&parallel.theory, &right, Limits::default(), &cancellation).unwrap()
                );
                for tested in 0..1 << atoms.len() {
                    let left_test = Interpretation::new(&serial.theory, selected(tested)).unwrap();
                    let right_test =
                        Interpretation::new(&parallel.theory, selected(tested)).unwrap();
                    assert_eq!(
                        models_reduct(
                            &serial.theory,
                            &left,
                            &left_test,
                            Limits::default(),
                            &cancellation
                        )
                        .unwrap(),
                        models_reduct(
                            &parallel.theory,
                            &right,
                            &right_test,
                            Limits::default(),
                            &cancellation
                        )
                        .unwrap()
                    );
                }
            }
        }
    }
}

#[test]
fn cancellation_during_original_publication_keeps_the_typed_stop() {
    use super::super::testing::{self as route, Point};
    let cancellation = Cancellation::default();
    let mut preparation = testing::prepare("p(1..16).q(1;2).h(X,Y):-p(X),q(Y).");
    preparation.budget = preparation
        .budget
        .with_cancellation(Some(cancellation.clone()));
    let ((result, counts), tasks) = with_source_workers(NonZeroUsize::new(14).unwrap(), || {
        route::controlled(true, Some((Point::Publish, 1, cancellation)), || {
            crate::formula_ground::ground(preparation, None, None)
        })
    });
    assert!(tasks > 0);
    assert_eq!(counts.published, 1);
    assert_eq!(result.unwrap_err().interruption(), Some(Stop::Cancelled));
}

#[test]
fn single_generator_rules_retain_exact_serial_witnessed_emission() {
    for source in [
        "p(1;2;3).q(1;2;3).r(1;2;3).h(X):-p(X),q(X).h(X):-p(X),r(X).h(X):-p(X),q(X).",
        "p(1;2).q(1;2).h(X,X):-p(X),p(X),q(X).",
        "-p(1;2).q(1;2).-h(X,7,X):- -p(X),q(X).",
        "p(f(1);f(2)).q(f(1);f(2)).h(X):-p(X),q(X).",
        "a.b.h:-a,b.",
        "p(1;2;3).q(4;5).h(X):-p(X),q(X).",
    ] {
        let expected = run(source, None, &Bound::default()).result.unwrap();
        for workers in [1, 14] {
            let actual = run(source, Some(workers), &Bound::default());
            assert_eq!(actual.tasks, 0, "no expanding suffix: {source}");
            assert_eq!(actual.result.unwrap(), expected);
        }
    }
}
