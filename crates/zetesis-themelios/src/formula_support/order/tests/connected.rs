//! Scheduling changes are checked against complete joins under every finite
//! permutation, while receipts distinguish the selected production order.

use std::{borrow::Cow, cell::Cell};

use super::{atom, less, location, order};
use crate::expansion::Budget;
use crate::formula_binding::Binding;
use crate::formula_ir::{Expression, LiteralIr, Operation};
use crate::formula_support::{Computation, Context, Counters, Join, testing::Fixture};
use crate::grounding_observer::{GroundingOutcome, GroundingPhase, GroundingWork, Profile};
use crate::{ExpansionLimits, FormulaFailure, FormulaLimits, GroundingObserver, ProgramSite};
use themelios_program::{program::Relation, term::BinaryOp};
use zetesis_core::{Atom, AtomPattern, Predicate, Term, Value, ValueNodeRef};

#[derive(Default)]
struct Observer(Cell<Option<GroundingWork>>);
impl GroundingObserver for Observer {
    fn enter(&self) {}
    fn exit(&self) {}
    fn details_enabled(&self) -> bool {
        true
    }
    fn phase_exit(
        &self,
        _: GroundingPhase,
        _: Option<ProgramSite>,
        _: GroundingOutcome,
        work: GroundingWork,
    ) {
        self.0.set(Some(work));
    }
}

struct Run {
    result: Result<Vec<Vec<i32>>, FormulaFailure>,
    order: Vec<usize>,
    rows: u64,
    work: u64,
}

fn execute(
    fixture: &mut Fixture,
    literals: &[LiteralIr],
    variables: usize,
    permutation: Option<&[usize]>,
) -> Run {
    fixture.with(location(), |support, computation, counters| {
        let observer = Observer::default();
        let profile = Profile::new(Some(&observer));
        let previous = std::mem::replace(&mut counters.observed, profile.work());
        let before = counters.accounting.work;
        let mut actual_order = Vec::new();
        let result = profile.phase(GroundingPhase::RuleInstantiation, Some(location()), || {
            let limits = FormulaLimits::default();
            let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
            let prefix = Binding::new(computation, &limits, counters, location())?;
            let mut plan = super::super::Plan::new(
                literals,
                &prefix,
                variables,
                super::super::SourceRows {
                    relations: support,
                    pivot: None,
                },
                &mut budget,
                location(),
                None,
            )?;
            if let Some(permutation) = permutation {
                assert_eq!(permutation.len(), plan.patterns.len());
                plan.patterns.sort_by_key(|pattern| {
                    permutation
                        .iter()
                        .position(|&source| source == pattern.source)
                        .unwrap()
                });
                plan.decisions = super::super::Decisions::checked(
                    literals,
                    &plan.patterns,
                    &vec![false; variables],
                    &mut budget,
                    location(),
                    &mut |_| Ok(()),
                )?;
            }
            actual_order = plan.patterns.iter().map(|pattern| pattern.source).collect();
            let mut join = Join::with_plan(
                literals,
                &prefix,
                variables,
                support,
                Cow::Owned(plan),
                None,
                Context::new(computation, &limits, counters, location()),
            )?;
            complete(&mut join, computation, &mut budget, counters, variables)
        });
        counters.observed = previous;
        Run {
            result,
            order: actual_order,
            rows: observer.0.get().unwrap().join_rows.unwrap(),
            work: counters.accounting.work - before,
        }
    })
}

fn complete(
    join: &mut Join<'_, '_>,
    computation: &mut Computation<'_, '_>,
    budget: &mut Budget,
    counters: &mut Counters,
    variables: usize,
) -> Result<Vec<Vec<i32>>, FormulaFailure> {
    let mut found = Vec::new();
    while let Some(row) = join.next_row(
        computation,
        &FormulaLimits::default(),
        budget,
        counters,
        location(),
    )? {
        if row.passes {
            found.push(
                (0..variables)
                    .map(|slot| {
                        match row
                            .values
                            .read(slot, computation.read(), location())
                            .unwrap()
                            .descriptor()
                        {
                            ValueNodeRef::Number(value) => value,
                            _ => panic!("numeric join fixture"),
                        }
                    })
                    .collect::<Vec<_>>(),
            );
        }
    }
    join.take_family().finish()?;
    found.sort();
    Ok(found)
}

fn row(name: &str, values: &[i32]) -> Atom {
    Atom::new(
        Predicate::new(name, values.len()).unwrap(),
        values.iter().copied().map(Value::Number).collect(),
    )
    .unwrap()
}

fn graph(width: i32) -> (Fixture, Vec<LiteralIr>, Vec<Vec<i32>>) {
    let mut atoms = Vec::new();
    let mut expected = Vec::new();
    for value in 0..width {
        for name in ["p", "q", "z"] {
            atoms.push(row(name, &[value]));
        }
        for next in [(value + 1) % width, (value + 2) % width] {
            atoms.push(row("edge", &[value, next]));
            for other in 0..width {
                expected.push(vec![value, other, next]);
            }
        }
    }
    expected.sort();
    let mut fixture = Fixture::from_atoms(atoms, location());
    let literals = vec![
        atom(&mut fixture, "p", &[0]),
        atom(&mut fixture, "q", &[1]),
        atom(&mut fixture, "z", &[2]),
        atom(&mut fixture, "edge", &[0, 2]),
    ];
    (fixture, literals, expected)
}

#[test]
fn connected_order_preserves_every_complete_binding() {
    let (mut fixture, literals, expected) = graph(4);
    assert_eq!(
        execute(&mut fixture, &literals, 3, None).result.unwrap(),
        expected
    );
    for first in 0..4 {
        for second in 0..4 {
            for third in 0..4 {
                for fourth in 0..4 {
                    let permutation = [first, second, third, fourth];
                    if (0..4).any(|index| permutation[..index].contains(&permutation[index])) {
                        continue;
                    }
                    assert_eq!(
                        execute(&mut fixture, &literals, 3, Some(&permutation))
                            .result
                            .unwrap(),
                        expected,
                        "source occurrence permutation {permutation:?}",
                    );
                }
            }
        }
    }
}

#[test]
fn connected_order_avoids_unrelated_domain_products() {
    let (mut fixture, literals, expected) = graph(16);
    let selected = execute(&mut fixture, &literals, 3, None);
    let independent = execute(&mut fixture, &literals, 3, Some(&[0, 1, 2, 3]));
    assert_eq!(selected.order, [0, 3, 2, 1]);
    assert_eq!(selected.result.unwrap(), expected);
    assert_eq!(independent.result.unwrap(), expected);
    assert!(selected.rows < independent.rows);
    assert!(selected.work < independent.work);
}

fn intervals() -> (Fixture, Vec<LiteralIr>, Vec<Vec<i32>>) {
    let mut atoms = Vec::new();
    for task in 0..2 {
        for time in 0..4 {
            for name in ["start", "end"] {
                atoms.push(row(name, &[task, time]));
            }
        }
    }
    let mut fixture = Fixture::from_atoms(atoms, location());
    let literals = vec![
        atom(&mut fixture, "start", &[0, 1]),
        atom(&mut fixture, "end", &[0, 2]),
        atom(&mut fixture, "start", &[3, 4]),
        atom(&mut fixture, "end", &[3, 5]),
        less(1, 5),
        less(4, 2),
    ];
    let mut expected = Vec::new();
    for first in 0..2 {
        for start in 0..4 {
            for end in 0..4 {
                for second in 0..2 {
                    for other_start in 0..end {
                        for other_end in start + 1..4 {
                            expected.push(vec![first, start, end, second, other_start, other_end]);
                        }
                    }
                }
            }
        }
    }
    (fixture, literals, expected)
}

#[test]
fn ready_comparisons_preserve_interval_bindings() {
    let (mut fixture, literals, expected) = intervals();
    assert_eq!(
        execute(&mut fixture, &literals, 6, None).result.unwrap(),
        expected,
    );
    for first in 0..4 {
        for second in 0..4 {
            for third in 0..4 {
                for fourth in 0..4 {
                    let permutation = [first, second, third, fourth];
                    if (0..4).any(|index| permutation[..index].contains(&permutation[index])) {
                        continue;
                    }
                    assert_eq!(
                        execute(&mut fixture, &literals, 6, Some(&permutation))
                            .result
                            .unwrap(),
                        expected,
                        "source occurrence permutation {permutation:?}",
                    );
                }
            }
        }
    }
}

#[test]
fn ready_comparisons_avoid_wide_connected_prefixes() {
    let (mut fixture, literals, expected) = intervals();
    let selected = execute(&mut fixture, &literals, 6, None);
    let connected = execute(&mut fixture, &literals, 6, Some(&[0, 1, 2, 3]));
    // The cross-interval comparison becomes ready at depth two; sharing the
    // first task alone would first combine each start with all four ends.
    assert_eq!(selected.order, [0, 3, 1, 2]);
    assert_eq!(selected.result.unwrap(), expected);
    assert_eq!(connected.result.unwrap(), expected);
    assert_eq!(selected.rows, 8 + 8 * 8 + 24 * 4 + 96 * 4);
    assert_eq!(connected.rows, 8 + 8 * 4 + 32 * 8 + 96 * 4);
    assert!(selected.rows < connected.rows);
    assert!(selected.work < connected.work);
}

#[test]
fn empty_sources_precede_nonempty_generators() {
    let mut fixture = Fixture::default();
    let literals = [
        atom(&mut fixture, "unit", &[]),
        atom(&mut fixture, "seed", &[0]),
        atom(&mut fixture, "edge", &[0, 1]),
        atom(&mut fixture, "empty", &[2]),
    ];
    assert_eq!(
        order(&mut fixture, &literals, &[1, 1, 100, 0], 3),
        ["unit", "empty", "seed", "edge"],
    );
}

#[test]
fn constant_arguments_enable_posting_preference() {
    let mut fixture = Fixture::default();
    let constant = fixture.pattern(
        &AtomPattern::new(
            Predicate::new("edge", 2).unwrap(),
            vec![Term::Constant(Value::Number(7)), Term::Variable(0)],
        )
        .unwrap(),
        location(),
    );
    let literals = [
        atom(&mut fixture, "independent", &[1]),
        LiteralIr::Atom(themelios_program::program::DefaultNegation::None, constant),
    ];
    assert_eq!(
        order(&mut fixture, &literals, &[1, 100], 2),
        ["edge", "independent"]
    );
}

#[test]
fn nested_bindings_do_not_claim_a_flat_posting() {
    use crate::formula_pattern::{ArgumentPattern, PatternAtom, PatternNode};
    use crate::formula_support::{PatternOccurrence, PositivePattern};
    let mut fixture = Fixture::default();
    let whole = fixture.pattern(
        &AtomPattern::new(
            Predicate::new("structured", 1).unwrap(),
            vec![Term::Variable(1)],
        )
        .unwrap(),
        location(),
    );
    let shape = fixture.constructor(ValueNodeRef::Tuple { arity: 1 }, location());
    let pattern = PatternAtom {
        atom: whole,
        arguments: vec![ArgumentPattern {
            position: 0,
            nodes: vec![PatternNode::Constructor(shape), PatternNode::Slot(0)],
        }],
    };
    fixture.with(location(), |_, computation, counters| {
        let flat = computation
            .static_pattern(whole, &FormulaLimits::default(), counters, location())
            .unwrap();
        let occurrence = PatternOccurrence {
            source: 0,
            pattern: PositivePattern::Structural(pattern.bind(flat)),
        };
        let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
        assert!(
            !super::super::has_bound_argument(&occurrence, &[true, false], &mut budget, location())
                .unwrap()
        );
        assert!(
            super::super::has_bound_argument(&occurrence, &[true, true], &mut budget, location())
                .unwrap()
        );
    });
}

#[test]
fn skewed_postings_preserve_complete_bindings() {
    // All edge rows have the same indexed first argument. Connectivity is a
    // preference, not a promise of smaller fanout than the independent domain.
    let mut atoms = vec![row("p", &[0]), row("q", &[0]), row("q", &[1])];
    atoms.extend((0..8).map(|value| row("edge", &[0, value])));
    let mut fixture = Fixture::from_atoms(atoms, location());
    let literals = [
        atom(&mut fixture, "p", &[0]),
        atom(&mut fixture, "q", &[1]),
        atom(&mut fixture, "edge", &[0, 2]),
    ];
    let mut expected = (0..2)
        .flat_map(|other| (0..8).map(move |value| vec![0, other, value]))
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(
        execute(&mut fixture, &literals, 3, None).result.unwrap(),
        expected
    );
    assert_eq!(
        execute(&mut fixture, &literals, 3, Some(&[0, 1, 2]))
            .result
            .unwrap(),
        expected
    );
}

fn arithmetic_fixture(operator: BinaryOp, left: i32, right: i32) -> (Fixture, Vec<LiteralIr>) {
    let mut fixture = Fixture::from_atoms(
        [
            row("p", &[left]),
            row("q", &[0]),
            row("q", &[1]),
            row("edge", &[left, right]),
        ],
        location(),
    );
    let literals = vec![
        atom(&mut fixture, "p", &[0]),
        atom(&mut fixture, "q", &[1]),
        atom(&mut fixture, "edge", &[0, 2]),
        LiteralIr::Compare(
            Expression {
                nodes: vec![
                    Operation::Variable(0),
                    Operation::Variable(2),
                    Operation::Binary(operator, 0, 1),
                ],
            },
            Relation::Eq,
            Expression {
                nodes: vec![Operation::Variable(1)],
            },
        ),
    ];
    (fixture, literals)
}

#[test]
fn connected_order_preserves_integer_overflow() {
    let (mut fixture, literals) = arithmetic_fixture(BinaryOp::Add, i32::MAX, 1);
    for permutation in [None, Some(&[0, 1, 2][..])] {
        assert!(matches!(
            execute(&mut fixture, &literals, 3, permutation).result,
            Err(FormulaFailure::Expansion(
                crate::ExpansionFailure::Evaluation {
                    error: themelios_program::term::EvalError::Overflow,
                    ..
                }
            ))
        ));
    }
}

#[test]
fn connected_order_preserves_undefined_division() {
    let (mut fixture, literals) = arithmetic_fixture(BinaryOp::Div, 1, 0);
    for permutation in [None, Some(&[0, 1, 2][..])] {
        assert!(matches!(
            execute(&mut fixture, &literals, 3, permutation).result,
            Err(FormulaFailure::Expansion(
                crate::ExpansionFailure::Evaluation {
                    error: themelios_program::term::EvalError::Undefined,
                    ..
                }
            ))
        ));
    }
}
