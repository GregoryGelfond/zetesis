use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Location, Span};
use themelios_program::program::{DefaultNegation, Relation};
use themelios_program::term::BinaryOp;
use zetesis_core::{Atom, AtomPattern, Predicate, Term, Value};

use crate::expansion::Budget;
use crate::formula_binding::Binding;
use crate::formula_ir::{Expression, LiteralIr, Operation};
use crate::formula_support::{Counters, Join, Support, SupportCatalog, Traversal};
use crate::{
    ExpansionFailure, ExpansionLimits, ExpansionResource, FormulaFailure, FormulaLimits,
    FormulaResource,
};

fn location() -> Location {
    Location {
        source: SourceId::new(73),
        span: Span::empty(ByteOffset::new(9)),
    }
}

fn atom(name: &str, values: Vec<Value>) -> Atom {
    Atom::new(Predicate::new(name, values.len()).unwrap(), values).unwrap()
}

fn pattern(name: &str, variables: &[usize]) -> LiteralIr {
    LiteralIr::Atom(
        DefaultNegation::None,
        AtomPattern::new(
            Predicate::new(name, variables.len()).unwrap(),
            variables.iter().map(|&slot| Term::Variable(slot)).collect(),
        )
        .unwrap(),
    )
}

fn numbers(values: &[i32]) -> Vec<Value> {
    values.iter().map(|&value| Value::Number(value)).collect()
}

fn with_join<T>(
    literals: &[LiteralIr],
    variables: usize,
    atoms: Vec<Atom>,
    use_join: impl FnOnce(&mut Join<'_, '_>, &mut Budget, &mut Counters) -> T,
) -> T {
    let limits = FormulaLimits::default();
    let mut catalog = SupportCatalog::default();
    for atom in atoms {
        catalog = catalog
            .insert(atom, &limits, &mut Counters::default(), location())
            .unwrap();
    }
    let relations = catalog
        .snapshot(&limits, &mut Counters::default(), location())
        .unwrap();
    let support = Support::indexed(&relations, &limits, &Counters::default(), location()).unwrap();
    let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
    let mut join = Join::new(
        literals,
        &Binding::default(),
        variables,
        &support,
        &mut budget,
        location(),
    )
    .unwrap();
    use_join(&mut join, &mut budget, &mut Counters::default())
}

#[test]
fn a_lent_row_uses_the_live_join_slots() {
    with_join(
        &[pattern("p", &[0])],
        1,
        vec![atom("p", numbers(&[7]))],
        |join, budget, counters| {
            let storage = join.values.as_ptr();
            let row = join
                .next_row(&FormulaLimits::default(), budget, counters, location())
                .unwrap()
                .unwrap();
            assert_eq!(row.values.slots().as_ptr(), storage);
            assert_eq!(row.values.read(0, location()).unwrap(), &Value::Number(7));
        },
    );
}

#[test]
fn advancing_a_lent_row_undoes_its_last_depth() {
    let atoms = vec![
        atom("p", numbers(&[1])),
        atom("q", numbers(&[1, 2])),
        atom("q", numbers(&[1, 3])),
    ];
    with_join(
        &[pattern("p", &[0]), pattern("q", &[0, 1])],
        2,
        atoms,
        |join, budget, counters| {
            let limits = FormulaLimits::default();
            for expected in [2, 3] {
                let row = join
                    .next_row(&limits, budget, counters, location())
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    row.values.slots(),
                    &[Some(Value::Number(1)), Some(Value::Number(expected))]
                );
            }
            assert!(
                join.next_row(&limits, budget, counters, location())
                    .unwrap()
                    .is_none()
            );
            assert_eq!(join.traversal, Traversal::Finished);
            assert_eq!(counters.substitutions, 2);
        },
    );
}

#[test]
fn owned_rows_survive_later_join_mutation() {
    with_join(
        &[pattern("p", &[0])],
        1,
        vec![atom("p", numbers(&[1])), atom("p", numbers(&[2]))],
        |join, budget, counters| {
            let limits = FormulaLimits::default();
            let first = join
                .next_owned_row(&limits, budget, counters, location())
                .unwrap()
                .unwrap();
            // Arithmetic family validation needs this exact capability: taking
            // evidence and advancing while a completed row remains in its task.
            assert!(join.take_family().defined);
            let second = join
                .next_row(&limits, budget, counters, location())
                .unwrap()
                .unwrap();
            assert_eq!(first.values.read(0, location()).unwrap(), &Value::Number(1));
            assert_eq!(
                second.values.read(0, location()).unwrap(),
                &Value::Number(2)
            );
        },
    );
}

#[derive(Debug, PartialEq, Eq)]
struct Run {
    rows: Vec<(Vec<Option<Value>>, bool)>,
    failure: Option<String>,
    work: u64,
    substitutions: u64,
}

fn correlated(owned: bool, max_work: u64) -> Run {
    let atoms = [[1, 1], [1, 2], [2, 2], [2, 3]]
        .map(|row| atom("p", numbers(&row)))
        .to_vec();
    with_join(
        &[
            pattern("p", &[0, 1]),
            pattern("p", &[1, 2]),
            pattern("p", &[2, 2]),
        ],
        3,
        atoms,
        |join, budget, counters| {
            let limits = FormulaLimits {
                max_work,
                ..Default::default()
            };
            let mut rows = Vec::new();
            let failure = loop {
                let next = if owned {
                    join.next_owned_row(&limits, budget, counters, location())
                } else {
                    join.next_row(&limits, budget, counters, location())
                };
                match next {
                    Ok(Some(row)) => rows.push((row.values.slots().to_vec(), row.passes)),
                    Ok(None) => break None,
                    Err(error) => break Some(format!("{error:?}")),
                }
            };
            Run {
                rows,
                failure,
                work: counters.work,
                substitutions: counters.substitutions,
            }
        },
    )
}

#[test]
fn lending_preserves_correlated_row_order() {
    let owned = correlated(true, FormulaLimits::default().max_work);
    assert!(owned.failure.is_none());
    assert_eq!(owned.rows.len(), 4);
    assert_eq!(correlated(false, FormulaLimits::default().max_work), owned);
}

#[test]
fn lending_preserves_each_work_limited_prefix() {
    let completed = correlated(true, FormulaLimits::default().max_work);
    assert!(completed.failure.is_none());
    for limit in 0..=completed.work {
        assert_eq!(
            correlated(false, limit),
            correlated(true, limit),
            "work limit {limit}"
        );
    }
}

#[test]
fn rejected_rows_remain_available_for_validation() {
    let variable = |slot| Expression {
        nodes: vec![Operation::Variable(slot)],
    };
    let literals = [
        pattern("p", &[0]),
        LiteralIr::Compare(variable(0), Relation::Lt, variable(0)),
    ];
    with_join(
        &literals,
        1,
        vec![atom("p", numbers(&[1])), atom("p", numbers(&[2]))],
        |join, budget, counters| {
            join.evidence();
            for expected in [1, 2] {
                let row = join
                    .next_row(&FormulaLimits::default(), budget, counters, location())
                    .unwrap()
                    .unwrap();
                assert!(!row.passes);
                assert_eq!(
                    row.values.read(0, location()).unwrap(),
                    &Value::Number(expected)
                );
            }
            assert!(
                join.next_row(&FormulaLimits::default(), budget, counters, location())
                    .unwrap()
                    .is_none()
            );
        },
    );
}

#[test]
fn a_borrowed_frame_copies_no_scalar_payload() {
    let values = vec![Value::Symbol("payload".into())];
    let run = |owned| {
        with_join(
            &[pattern("p", &[0])],
            1,
            vec![atom("p", values.clone())],
            |join, _, counters| {
                let mut budget = Budget::new(
                    ExpansionLimits {
                        max_scalar_bytes: 7,
                        ..Default::default()
                    },
                    usize::MAX,
                );
                let result = if owned {
                    join.next_owned_row(
                        &FormulaLimits::default(),
                        &mut budget,
                        counters,
                        location(),
                    )
                } else {
                    join.next_row(&FormulaLimits::default(), &mut budget, counters, location())
                };
                result.map(|row| row.map(|row| row.values.slots().to_vec()))
            },
        )
    };
    assert_eq!(
        run(false).unwrap(),
        Some(vec![Some(Value::Symbol("payload".into()))])
    );
    assert!(matches!(
        run(true),
        Err(FormulaFailure::Expansion(ExpansionFailure::Limit {
            resource: ExpansionResource::ScalarBytes,
            observed: 14,
            limit: 7,
            ..
        }))
    ));
}

#[test]
fn owned_snapshot_refusal_precedes_filter_failure() {
    let overflow = Expression {
        nodes: vec![
            Operation::Constant(Value::Number(i32::MAX)),
            Operation::Constant(Value::Number(1)),
            Operation::Binary(BinaryOp::Add, 0, 1),
        ],
    };
    let literals = [
        pattern("p", &[0]),
        LiteralIr::TupleCompare(vec![overflow], Relation::Eq, vec![]),
    ];
    with_join(
        &literals,
        1,
        vec![atom("p", vec![Value::Symbol("payload".into())])],
        |join, _, counters| {
            let mut budget = Budget::new(
                ExpansionLimits {
                    max_scalar_bytes: 7,
                    ..Default::default()
                },
                usize::MAX,
            );
            assert!(matches!(
                join.next_owned_row(&FormulaLimits::default(), &mut budget, counters, location()),
                Err(FormulaFailure::Expansion(ExpansionFailure::Limit {
                    resource: ExpansionResource::ScalarBytes,
                    observed: 14,
                    limit: 7,
                    ..
                }))
            ));
        },
    );
    with_join(
        &literals,
        1,
        vec![atom("p", vec![Value::Symbol("payload".into())])],
        |join, budget, counters| {
            assert!(matches!(
                join.next_owned_row(&FormulaLimits::default(), budget, counters, location()),
                Err(FormulaFailure::Expansion(
                    ExpansionFailure::Evaluation { .. }
                ))
            ));
        },
    );
}

#[test]
fn an_empty_join_yields_once() {
    with_join(&[], 0, vec![], |join, budget, counters| {
        let limits = FormulaLimits::default();
        assert!(
            join.next_row(&limits, budget, counters, location())
                .unwrap()
                .unwrap()
                .values
                .slots()
                .is_empty()
        );
        assert!(
            join.next_row(&limits, budget, counters, location())
                .unwrap()
                .is_none()
        );
        assert_eq!(counters.substitutions, 1);
    });
}

#[test]
fn missing_slots_are_not_completed_borrowed_rows() {
    with_join(&[], 1, vec![], |join, budget, counters| {
        assert!(
            matches!(join.next_row(&FormulaLimits::default(), budget, counters, location()), Err(FormulaFailure::UnsafeVariable { variable: 0, location: found }) if found == location())
        );
    });
}

#[test]
fn a_lent_row_obeys_the_substitution_ceiling() {
    with_join(
        &[pattern("p", &[0])],
        1,
        vec![atom("p", numbers(&[1])), atom("p", numbers(&[2]))],
        |join, budget, counters| {
            let limits = FormulaLimits {
                max_substitutions: 1,
                ..Default::default()
            };
            assert!(
                join.next_row(&limits, budget, counters, location())
                    .unwrap()
                    .is_some()
            );
            assert!(matches!(
                join.next_row(&limits, budget, counters, location()),
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::Substitutions,
                    observed: 2,
                    limit: 1,
                    ..
                })
            ));
            assert_eq!(counters.substitutions, 1);
        },
    );
}

fn generated_rows(head: bool) {
    let literals = [
        pattern("p", &[0]),
        LiteralIr::Range {
            target: 1,
            lower: Expression {
                nodes: vec![Operation::Constant(Value::Number(1))],
            },
            upper: Expression {
                nodes: vec![Operation::Variable(0)],
            },
            binder: true,
        },
    ];
    with_join(
        &literals,
        2,
        vec![atom("p", numbers(&[2]))],
        |join, budget, counters| {
            if head {
                join.stage_head(1..2);
            }
            let storage = join.values.as_ptr();
            for expected in [1, 2] {
                let row = join
                    .next_row(&FormulaLimits::default(), budget, counters, location())
                    .unwrap()
                    .unwrap();
                assert_ne!(row.values.slots().as_ptr(), storage);
                assert_eq!(
                    row.values.slots(),
                    &[Some(Value::Number(2)), Some(Value::Number(expected))]
                );
            }
            assert!(
                join.next_row(&FormulaLimits::default(), budget, counters, location())
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
        with_join(&[], 0, vec![], |join, budget, counters| {
            let limits = FormulaLimits {
                max_work: 1,
                ..Default::default()
            };
            let found = if owned {
                join.next_owned_row(&limits, budget, counters, location())
                    .unwrap()
                    .is_some()
            } else {
                join.next_row(&limits, budget, counters, location())
                    .unwrap()
                    .is_some()
            };
            assert!(found);
            assert_eq!(join.traversal, Traversal::EmptyVisited);
            let error = if owned {
                join.next_owned_row(&limits, budget, counters, location())
                    .err()
            } else {
                join.next_row(&limits, budget, counters, location()).err()
            };
            assert!(matches!(
                error,
                Some(FormulaFailure::Limit {
                    resource: FormulaResource::Work,
                    observed: 2,
                    limit: 1,
                    ..
                })
            ));
            assert_eq!(join.traversal, Traversal::EmptyVisited);
            let limits = FormulaLimits {
                max_work: 2,
                ..Default::default()
            };
            assert!(
                join.next_row(&limits, budget, counters, location())
                    .unwrap()
                    .is_none()
            );
            assert_eq!(join.traversal, Traversal::Finished);
            assert_eq!(counters.work, 2);
            assert!(
                join.next_row(&limits, budget, counters, location())
                    .unwrap()
                    .is_none()
            );
            assert_eq!(counters.work, 2);
        });
    }
}

#[test]
fn owned_rows_resume_before_returning() {
    with_join(
        &[pattern("p", &[0])],
        1,
        vec![atom("p", numbers(&[1]))],
        |join, budget, counters| {
            let row = join
                .next_owned_row(&FormulaLimits::default(), budget, counters, location())
                .unwrap()
                .unwrap();
            assert_eq!(row.values.read(0, location()).unwrap(), &Value::Number(1));
            assert_eq!(join.traversal, Traversal::Searching);
            assert_eq!(join.depth, 0);
            assert_eq!(join.values, [None]);
        },
    );
}

#[test]
fn an_existing_head_finishes_without_an_empty_row() {
    with_join(&[], 0, vec![atom("p", vec![])], |join, budget, counters| {
        let head = AtomPattern::new(Predicate::new("p", 0).unwrap(), vec![]).unwrap();
        assert!(
            join.next_support(
                &head,
                &std::collections::BTreeSet::new(),
                &FormulaLimits::default(),
                budget,
                counters,
                location()
            )
            .unwrap()
            .is_none()
        );
        assert_eq!(join.traversal, Traversal::Finished);
        assert_eq!(counters.substitutions, 0);
    });
}
