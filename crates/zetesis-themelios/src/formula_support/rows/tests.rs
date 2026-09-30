use super::{Frame, Ownership};
use crate::expansion::Budget;
use crate::formula_ir::{Expression, LiteralIr, Operation};
use crate::formula_support::testing::Fixture;
use crate::formula_support::testing::numbers;
use crate::formula_support::{Computation, Counters, Join, Traversal};
use crate::{ExpansionLimits, FormulaFailure, FormulaLimits, FormulaResource};
use themelios_base::{
    source::SourceId,
    span::{ByteOffset, Location, Span},
};
use themelios_program::program::{DefaultNegation, Relation};
use zetesis_core::{Atom, AtomPattern, Predicate, Term, Value, ValueLimits, ValueNodeRef};
use zetesis_test_support::programs::atom;

use crate::formula_support::Context;
fn location() -> Location {
    Location {
        source: SourceId::new(73),
        span: Span::empty(ByteOffset::new(9)),
    }
}
fn pattern(fixture: &mut Fixture, name: &str, variables: &[usize]) -> LiteralIr {
    LiteralIr::Atom(
        DefaultNegation::None,
        fixture.pattern(
            &AtomPattern::new(
                Predicate::new(name, variables.len()).unwrap(),
                variables.iter().map(|&slot| Term::Variable(slot)).collect(),
            )
            .unwrap(),
            location(),
        ),
    )
}
fn with_join<T>(
    literals: impl FnOnce(&mut Fixture) -> Vec<LiteralIr>,
    variables: usize,
    atoms: Vec<Atom>,
    run: impl FnOnce(&mut Join<'_, '_>, &mut Computation<'_, '_>, &mut Budget, &mut Counters) -> T,
) -> T {
    let mut fixture = Fixture::from_atoms(atoms, location());
    let literals = literals(&mut fixture);
    with_fixture(&literals, variables, fixture, run)
}

fn with_fixture<T>(
    literals: &[LiteralIr],
    variables: usize,
    mut fixture: Fixture,
    run: impl FnOnce(&mut Join<'_, '_>, &mut Computation<'_, '_>, &mut Budget, &mut Counters) -> T,
) -> T {
    fixture.with(location(), |support, computation, counters| {
        let limits = FormulaLimits::default();
        let prefix =
            crate::formula_binding::Binding::new(computation, &limits, counters, location())
                .unwrap();
        let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
        let mut join = Join::new(
            literals,
            &prefix,
            variables,
            support,
            &mut budget,
            Context::new(computation, &limits, counters, location()),
        )
        .unwrap();
        run(&mut join, computation, &mut budget, counters)
    })
}
// Test result serialization is explicit and bounded; production frames retain IDs.
pub(super) fn exported(
    binding: &crate::formula_binding::Binding,
    computation: &Computation<'_, '_>,
) -> Vec<Option<Value>> {
    (0..binding.len())
        .map(|slot| {
            binding.slots().key(slot).unwrap().map(|key| {
                computation
                    .read()
                    .term(&key)
                    .unwrap()
                    .to_value(ValueLimits::default())
                    .unwrap()
            })
        })
        .collect()
}

#[test]
fn a_lent_row_uses_the_current_frame_route() {
    with_join(
        |fixture| vec![pattern(fixture, "p", &[0])],
        1,
        vec![atom("p", numbers(&[7]))],
        |join, computation, budget, counters| {
            let row = join
                .next_staged(
                    Ownership::Lend,
                    None,
                    budget,
                    Context::new(computation, &FormulaLimits::default(), counters, location()),
                )
                .unwrap()
                .unwrap();
            assert!(matches!(row.frame, Frame::Current));
            assert_eq!(
                row.frame
                    .binding(&join.values)
                    .read(0, computation.read(), location())
                    .unwrap()
                    .descriptor(),
                ValueNodeRef::Number(7)
            );
            assert_eq!(join.traversal, Traversal::PendingUndo);
        },
    );
}
#[test]
fn advancing_a_lent_row_undoes_its_last_depth() {
    with_join(
        |fixture| vec![pattern(fixture, "p", &[0]), pattern(fixture, "q", &[0, 1])],
        2,
        vec![
            atom("p", numbers(&[1])),
            atom("q", numbers(&[1, 2])),
            atom("q", numbers(&[1, 3])),
        ],
        |join, computation, budget, counters| {
            let limits = FormulaLimits::default();
            for expected in [2, 3] {
                let row = join
                    .next_row(computation, &limits, budget, counters, location())
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    exported(&row.values, computation),
                    vec![Some(Value::Number(1)), Some(Value::Number(expected))]
                );
            }
            assert!(
                join.next_row(computation, &limits, budget, counters, location())
                    .unwrap()
                    .is_none()
            );
            assert_eq!(join.traversal, Traversal::Finished);
            assert_eq!(counters.accounting.substitutions, 2);
        },
    );
}
#[test]
fn owned_rows_survive_later_join_mutation() {
    with_join(
        |fixture| vec![pattern(fixture, "p", &[0])],
        1,
        vec![atom("p", numbers(&[1])), atom("p", numbers(&[2]))],
        |join, computation, budget, counters| {
            let limits = FormulaLimits::default();
            let first = join
                .next_owned_row(computation, &limits, budget, counters, location())
                .unwrap()
                .unwrap();
            assert!(join.take_family().defined);
            let second = join
                .next_row(computation, &limits, budget, counters, location())
                .unwrap()
                .unwrap();
            assert_eq!(
                first
                    .values
                    .read(0, computation.read(), location())
                    .unwrap()
                    .descriptor(),
                ValueNodeRef::Number(1)
            );
            assert_eq!(
                second
                    .values
                    .read(0, computation.read(), location())
                    .unwrap()
                    .descriptor(),
                ValueNodeRef::Number(2)
            );
        },
    );
}
#[derive(Debug)]
struct Run {
    rows: Vec<(Vec<Option<Value>>, bool)>,
    failed: bool,
    work: u64,
}
fn correlated(owned: bool, allowance: u64) -> Run {
    let atoms = [[1, 1], [1, 2], [2, 2], [2, 3]]
        .map(|row| atom("p", numbers(&row)))
        .to_vec();
    with_join(
        |fixture| {
            vec![
                pattern(fixture, "p", &[0, 1]),
                pattern(fixture, "p", &[1, 2]),
                pattern(fixture, "p", &[2, 2]),
            ]
        },
        3,
        atoms,
        |join, computation, budget, counters| {
            let before = counters.accounting.work;
            let limits = FormulaLimits {
                max_work: before.saturating_add(allowance),
                ..Default::default()
            };
            let mut rows = Vec::new();
            let failed = loop {
                let next = if owned {
                    join.next_owned_row(computation, &limits, budget, counters, location())
                } else {
                    join.next_row(computation, &limits, budget, counters, location())
                };
                match next {
                    Ok(Some(row)) => rows.push((exported(&row.values, computation), row.passes)),
                    Ok(None) => break false,
                    Err(FormulaFailure::Limit {
                        resource: FormulaResource::Work,
                        ..
                    }) => break true,
                    Err(error) => panic!("unexpected refusal: {error}"),
                }
            };
            Run {
                rows,
                failed,
                work: counters.accounting.work - before,
            }
        },
    )
}
#[test]
fn lending_preserves_correlated_row_order() {
    let owned = correlated(true, u64::MAX);
    let lent = correlated(false, u64::MAX);
    assert!(!owned.failed && !lent.failed);
    assert_eq!(owned.rows.len(), 4);
    assert_eq!(lent.rows, owned.rows);
}
#[test]
fn every_work_stop_returns_only_a_semantic_prefix() {
    // ID snapshot work differs from lending. Each route must still return an
    // ordered prefix of its complete semantic rows before reporting refusal.
    for owned in [false, true] {
        let complete = correlated(owned, u64::MAX);
        for allowance in 0..complete.work {
            let stopped = correlated(owned, allowance);
            assert!(stopped.failed);
            assert!(complete.rows.starts_with(&stopped.rows));
        }
    }
}
#[test]
fn rejected_rows_remain_available_for_validation() {
    let variable = |slot| Expression {
        nodes: vec![Operation::Variable(slot)],
    };
    with_join(
        |fixture| {
            vec![
                pattern(fixture, "p", &[0]),
                LiteralIr::Compare(variable(0), Relation::Lt, variable(0)),
            ]
        },
        1,
        vec![atom("p", numbers(&[1])), atom("p", numbers(&[2]))],
        |join, computation, budget, counters| {
            join.evidence();
            for expected in [1, 2] {
                let row = join
                    .next_row(
                        computation,
                        &FormulaLimits::default(),
                        budget,
                        counters,
                        location(),
                    )
                    .unwrap()
                    .unwrap();
                assert!(!row.passes);
                assert_eq!(
                    row.values
                        .read(0, computation.read(), location())
                        .unwrap()
                        .descriptor(),
                    ValueNodeRef::Number(expected)
                );
            }
            assert!(
                join.next_row(
                    computation,
                    &FormulaLimits::default(),
                    budget,
                    counters,
                    location()
                )
                .unwrap()
                .is_none()
            );
        },
    );
}
#[test]
fn both_frame_routes_share_the_canonical_text() {
    for owned in [false, true] {
        with_join(
            |fixture| vec![pattern(fixture, "p", &[0])],
            1,
            vec![atom("p", vec![Value::Symbol("payload".into())])],
            |join, computation, _, counters| {
                let predicate = Predicate::new("p", 1).unwrap();
                let source = join.support.row(&predicate, 0).unwrap().value(0).unwrap();
                let ValueNodeRef::Symbol(source) = source.descriptor() else {
                    panic!("symbol")
                };
                let address = source.as_ptr();
                let mut budget = Budget::new(
                    ExpansionLimits {
                        max_scalar_bytes: 0,
                        ..Default::default()
                    },
                    usize::MAX,
                );
                let row = if owned {
                    join.next_owned_row(
                        computation,
                        &FormulaLimits::default(),
                        &mut budget,
                        counters,
                        location(),
                    )
                } else {
                    join.next_row(
                        computation,
                        &FormulaLimits::default(),
                        &mut budget,
                        counters,
                        location(),
                    )
                }
                .unwrap()
                .unwrap();
                let ValueNodeRef::Symbol(value) = row
                    .values
                    .read(0, computation.read(), location())
                    .unwrap()
                    .descriptor()
                else {
                    panic!("symbol")
                };
                assert_eq!(value.as_ptr(), address);
                assert_eq!(budget.usage().scalar_bytes, 0);
            },
        );
    }
}
#[test]
fn owned_snapshots_need_their_own_metadata_allowance() {
    with_join(
        |fixture| vec![pattern(fixture, "p", &[0])],
        1,
        vec![atom("p", numbers(&[1]))],
        |join, computation, budget, counters| {
            let limits = FormulaLimits::default();
            let frame = join
                .next_base(
                    Ownership::Lend,
                    None,
                    budget,
                    Context::new(computation, &limits, counters, location()),
                )
                .unwrap()
                .unwrap();
            assert!(matches!(frame, Frame::Current));
            let observer = computation.lease();
            let current = limits.max_support_bytes
                - computation
                    .allowance(&observer, &limits, location())
                    .unwrap();
            let bounded = FormulaLimits {
                max_support_bytes: current,
                ..limits
            };
            assert!(matches!(
                join.values
                    .copied(computation, &bounded, counters, location()),
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::SupportBytes,
                    ..
                })
            ));
            assert_eq!(
                join.values
                    .read(0, computation.read(), location())
                    .unwrap()
                    .descriptor(),
                ValueNodeRef::Number(1)
            );
        },
    );
}
#[test]
fn an_empty_join_yields_once() {
    with_join(
        |_| vec![],
        0,
        vec![],
        |join, computation, budget, counters| {
            let limits = FormulaLimits::default();
            assert!(
                join.next_row(computation, &limits, budget, counters, location())
                    .unwrap()
                    .unwrap()
                    .values
                    .slots()
                    .is_empty()
            );
            assert!(
                join.next_row(computation, &limits, budget, counters, location())
                    .unwrap()
                    .is_none()
            );
            assert_eq!(counters.accounting.substitutions, 1);
        },
    );
}
#[test]
fn missing_slots_are_not_completed_borrowed_rows() {
    with_join(
        |_| vec![],
        1,
        vec![],
        |join, computation, budget, counters| {
            assert!(matches!(
                join.next_row(
                    computation,
                    &FormulaLimits::default(),
                    budget,
                    counters,
                    location()
                ),
                Err(FormulaFailure::UnsafeVariable { variable: 0, .. })
            ));
        },
    );
}
#[test]
fn a_lent_row_obeys_the_substitution_ceiling() {
    with_join(
        |fixture| vec![pattern(fixture, "p", &[0])],
        1,
        vec![atom("p", numbers(&[1])), atom("p", numbers(&[2]))],
        |join, computation, budget, counters| {
            let limits = FormulaLimits {
                max_substitutions: 1,
                ..Default::default()
            };
            assert!(
                join.next_row(computation, &limits, budget, counters, location())
                    .unwrap()
                    .is_some()
            );
            assert!(matches!(
                join.next_row(computation, &limits, budget, counters, location()),
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::Substitutions,
                    observed: 2,
                    limit: 1,
                    ..
                })
            ));
            assert_eq!(counters.accounting.substitutions, 1);
        },
    );
}
fn generated_rows(head: bool) {
    let mut fixture = Fixture::from_atoms([atom("p", numbers(&[2]))], location());
    let one = fixture.scalar(&Value::Number(1), location());
    let literals = [
        pattern(&mut fixture, "p", &[0]),
        LiteralIr::Range {
            target: 1,
            lower: Expression {
                nodes: vec![Operation::Constant(one)],
            },
            upper: Expression {
                nodes: vec![Operation::Variable(0)],
            },
            binder: true,
        },
    ];
    with_fixture(
        &literals,
        2,
        fixture,
        |join, computation, budget, counters| {
            let limits = FormulaLimits::default();
            if head {
                join.stage_head(1..2, &limits, counters, location())
                    .unwrap();
            }
            for expected in [1, 2] {
                let row = join
                    .next_staged(
                        Ownership::Lend,
                        None,
                        budget,
                        Context::new(computation, &limits, counters, location()),
                    )
                    .unwrap()
                    .unwrap();
                assert!(matches!(row.frame, Frame::Owned(_)));
                assert_eq!(
                    exported(&row.frame.into_owned(), computation),
                    vec![Some(Value::Number(2)), Some(Value::Number(expected))]
                );
            }
            assert!(
                join.next_row(computation, &limits, budget, counters, location())
                    .unwrap()
                    .is_none()
            );
        },
    );
}
#[test]
fn body_generators_return_owned_rows() {
    generated_rows(false);
}
#[test]
fn head_generators_return_owned_rows() {
    generated_rows(true);
}
#[test]
fn empty_exhaustion_still_requires_its_work() {
    for owned in [false, true] {
        with_join(
            |_| vec![],
            0,
            vec![],
            |join, computation, budget, counters| {
                let limits = FormulaLimits::default();
                let found = if owned {
                    join.next_owned_row(computation, &limits, budget, counters, location())
                        .unwrap()
                        .is_some()
                } else {
                    join.next_row(computation, &limits, budget, counters, location())
                        .unwrap()
                        .is_some()
                };
                assert!(found);
                let bounded = FormulaLimits {
                    max_work: counters.accounting.work,
                    ..limits
                };
                assert!(matches!(
                    join.next_row(computation, &bounded, budget, counters, location()),
                    Err(FormulaFailure::Limit {
                        resource: FormulaResource::Work,
                        ..
                    })
                ));
                assert_eq!(join.traversal, Traversal::EmptyVisited);
                assert!(
                    join.next_row(computation, &limits, budget, counters, location())
                        .unwrap()
                        .is_none()
                );
                let finished = counters.accounting.work;
                assert!(
                    join.next_row(computation, &limits, budget, counters, location())
                        .unwrap()
                        .is_none()
                );
                assert_eq!(counters.accounting.work, finished);
            },
        );
    }
}
#[test]
fn owned_rows_resume_before_returning() {
    with_join(
        |fixture| vec![pattern(fixture, "p", &[0])],
        1,
        vec![atom("p", numbers(&[1]))],
        |join, computation, budget, counters| {
            let row = join
                .next_owned_row(
                    computation,
                    &FormulaLimits::default(),
                    budget,
                    counters,
                    location(),
                )
                .unwrap()
                .unwrap();
            assert_eq!(
                row.values
                    .read(0, computation.read(), location())
                    .unwrap()
                    .descriptor(),
                ValueNodeRef::Number(1)
            );
            assert_eq!(join.traversal, Traversal::Searching);
            assert_eq!(join.depth, 0);
            assert!(!join.values.is_bound(0, location()).unwrap());
        },
    );
}
#[test]
fn refused_undo_keeps_the_lent_frame_for_retry() {
    with_join(
        |fixture| vec![pattern(fixture, "p", &[0])],
        1,
        vec![atom("p", numbers(&[1])), atom("p", numbers(&[2]))],
        |join, computation, budget, counters| {
            let limits = FormulaLimits::default();
            join.next_row(computation, &limits, budget, counters, location())
                .unwrap()
                .unwrap();
            let depth = join.depth;
            let bounded = FormulaLimits {
                max_work: counters.accounting.work,
                ..limits
            };
            assert!(
                join.next_row(computation, &bounded, budget, counters, location())
                    .is_err()
            );
            assert_eq!(join.depth, depth);
            assert_eq!(join.traversal, Traversal::PendingUndo);
            assert_eq!(
                join.values
                    .read(0, computation.read(), location())
                    .unwrap()
                    .descriptor(),
                ValueNodeRef::Number(1)
            );
            let next = join
                .next_row(computation, &limits, budget, counters, location())
                .unwrap()
                .unwrap();
            assert_eq!(
                next.values
                    .read(0, computation.read(), location())
                    .unwrap()
                    .descriptor(),
                ValueNodeRef::Number(2)
            );
        },
    );
}
#[test]
fn an_existing_head_finishes_without_an_empty_row() {
    let mut fixture = Fixture::from_atoms([atom("p", vec![])], location());
    let head = fixture.pattern(
        &AtomPattern::new(Predicate::new("p", 0).unwrap(), vec![]).unwrap(),
        location(),
    );
    with_fixture(&[], 0, fixture, |join, computation, budget, counters| {
        assert!(
            join.next_support(
                Some(head),
                computation,
                &FormulaLimits::default(),
                budget,
                counters,
                location()
            )
            .unwrap()
            .is_none()
        );
        assert_eq!(join.traversal, Traversal::Finished);
        assert_eq!(counters.accounting.substitutions, 0);
    });
}

fn selected_support(owned: bool, allowance: u64) -> Run {
    with_join(
        |fixture| {
            vec![
                pattern(fixture, "p", &[0, 1]),
                pattern(fixture, "p", &[1, 2]),
                LiteralIr::Compare(
                    Expression {
                        nodes: vec![Operation::Variable(0)],
                    },
                    Relation::Lt,
                    Expression {
                        nodes: vec![Operation::Variable(2)],
                    },
                ),
            ]
        },
        3,
        [[1, 1], [1, 2], [2, 2], [2, 3]]
            .map(|row| atom("p", numbers(&row)))
            .to_vec(),
        |join, computation, budget, counters| {
            let before = counters.accounting.work;
            let limits = FormulaLimits {
                max_work: before.saturating_add(allowance),
                ..Default::default()
            };
            let mut rows = Vec::new();
            let failed = loop {
                let next = if owned {
                    join.next(computation, &limits, budget, counters, location())
                } else {
                    join.next_support(None, computation, &limits, budget, counters, location())
                };
                match next {
                    Ok(Some(binding)) => rows.push((exported(&binding, computation), true)),
                    Ok(None) => break false,
                    Err(FormulaFailure::Limit {
                        resource: FormulaResource::Work,
                        ..
                    }) => break true,
                    Err(error) => panic!("unexpected refusal: {error}"),
                }
            };
            Run {
                rows,
                failed,
                work: counters.accounting.work - before,
            }
        },
    )
}

#[test]
fn support_lending_preserves_selected_row_order() {
    let owned = selected_support(true, u64::MAX);
    let lent = selected_support(false, u64::MAX);
    assert!(!owned.failed && !lent.failed);
    assert!(!owned.rows.is_empty());
    assert_eq!(lent.rows, owned.rows);
}

#[test]
fn stopped_support_lending_returns_an_ordered_prefix() {
    let complete = selected_support(false, u64::MAX);
    assert!(!complete.failed);
    for allowance in 0..complete.work {
        let stopped = selected_support(false, allowance);
        assert!(stopped.failed);
        assert!(complete.rows.starts_with(&stopped.rows));
    }
}

fn produced_heads(projected: bool) -> Vec<Value> {
    let mut fixture = Fixture::from_atoms(
        [[1, 2], [1, 3], [2, 4]].map(|row| atom("e", numbers(&row))),
        location(),
    );
    let literals = [pattern(&mut fixture, "e", &[0, 1])];
    let head = fixture.pattern(
        &AtomPattern::new(Predicate::new("p", 1).unwrap(), vec![Term::Variable(0)]).unwrap(),
        location(),
    );
    with_fixture(
        &literals,
        2,
        fixture,
        |join, computation, budget, counters| {
            let limits = FormulaLimits::default();
            let mut produced = Vec::new();
            while let Some(binding) = join
                .next_support(
                    projected.then_some(head),
                    computation,
                    &limits,
                    budget,
                    counters,
                    location(),
                )
                .unwrap()
            {
                produced.push(exported(&binding, computation)[0].clone().unwrap());
                let head = computation
                    .static_pattern(head, &limits, counters, location())
                    .unwrap();
                let atom = computation
                    .atom(head, &binding, &limits, counters, location())
                    .unwrap();
                computation
                    .support(&atom, &limits, counters, location())
                    .unwrap();
            }
            produced
        },
    )
}

#[test]
fn support_lending_prunes_already_produced_heads() {
    assert_eq!(produced_heads(false), numbers(&[1, 1, 2]));
    assert_eq!(produced_heads(true), numbers(&[1, 2]));
}

#[test]
fn support_undo_refusal_can_be_retried() {
    with_join(
        |fixture| vec![pattern(fixture, "p", &[0])],
        1,
        vec![atom("p", numbers(&[1])), atom("p", numbers(&[2]))],
        |join, computation, budget, counters| {
            let limits = FormulaLimits::default();
            let first = join
                .next_support(None, computation, &limits, budget, counters, location())
                .unwrap()
                .unwrap();
            assert_eq!(exported(&first, computation), vec![Some(Value::Number(1))]);
            let depth = join.depth;
            let bounded = FormulaLimits {
                max_work: counters.accounting.work,
                ..limits
            };
            assert!(matches!(
                join.next_support(None, computation, &bounded, budget, counters, location()),
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::Work,
                    ..
                })
            ));
            assert_eq!(join.traversal, Traversal::PendingUndo);
            assert_eq!(join.depth, depth);
            assert_eq!(
                exported(&join.values, computation),
                vec![Some(Value::Number(1))]
            );
            let next = join
                .next_support(None, computation, &limits, budget, counters, location())
                .unwrap()
                .unwrap();
            assert_eq!(exported(&next, computation), vec![Some(Value::Number(2))]);
            assert!(
                join.next_support(None, computation, &limits, budget, counters, location())
                    .unwrap()
                    .is_none()
            );
        },
    );
}
