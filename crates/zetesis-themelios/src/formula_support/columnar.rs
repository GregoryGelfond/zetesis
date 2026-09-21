//! Complete the experimental equality selection with the existing join.
//!
//! One fixed positive pattern keeps its selected rows valid for the whole
//! cursor. This is a bounded caller control, not a production join strategy.
//! The ordinary probe, full scan and column filter retain distinct work
//! populations. No equality selection discharges matching or arithmetic.

use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Location, Span};
use themelios_program::program::{DefaultNegation, Relation as Comparison};
use themelios_program::term::BinaryOp;
use zetesis_core::relation::{Limits, Relation};
use zetesis_core::{Atom, AtomPattern, Predicate, Term, Value, ValueLimits, ValueNode};

use super::{Budget, Counters, Join, Support, SupportCatalog};
use crate::formula_ir::{Expression, LiteralIr, Operation};
use crate::formula_pattern::{ArgumentPattern, PatternAtom, PatternNode};
use crate::{ExpansionFailure, ExpansionLimits, FormulaFailure, FormulaLimits, FormulaResource};

#[derive(Clone, Copy)]
enum Route {
    Indexed,
    Scan,
    Columns,
}

fn location() -> Location {
    Location {
        source: SourceId::new(0),
        span: Span::empty(ByteOffset::new(0)),
    }
}

fn support(rows: Vec<Vec<Value>>) -> SupportCatalog {
    let mut support = SupportCatalog::default();
    for values in rows {
        let atom = Atom::new(Predicate::new("row", values.len()).unwrap(), values).unwrap();
        support = support
            .insert(
                atom,
                &FormulaLimits::default(),
                &mut Counters::default(),
                location(),
            )
            .unwrap();
    }
    support
}

fn pattern(terms: Vec<Term>) -> AtomPattern {
    AtomPattern::new(Predicate::new("row", terms.len()).unwrap(), terms).unwrap()
}

fn positive(pattern: AtomPattern) -> LiteralIr {
    LiteralIr::Atom(DefaultNegation::None, pattern)
}

fn numbers(values: &[i32]) -> Vec<Value> {
    values.iter().map(|&value| Value::Number(value)).collect()
}

fn number(value: i32) -> Expression {
    Expression {
        nodes: vec![Operation::Constant(Value::Number(value))],
    }
}

fn evaluate(
    route: Route,
    catalog: &SupportCatalog,
    literals: &[LiteralIr],
    prefix: &[Value],
    variables: usize,
    limits: &FormulaLimits,
) -> Result<Vec<Vec<Value>>, FormulaFailure> {
    let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
    let mut counters = Counters::default();
    let relations = catalog.snapshot(
        &FormulaLimits::default(),
        &mut Counters::default(),
        location(),
    )?;
    let support = Support::indexed(
        &relations,
        &crate::FormulaLimits::default(),
        &crate::formula_support::Counters::default(),
        location(),
    )
    .unwrap();
    let atom = match &literals[0] {
        LiteralIr::Atom(DefaultNegation::None, atom) => atom,
        LiteralIr::PatternAtom(pattern) => &pattern.atom,
        _ => panic!("positive first pattern"),
    };
    let original: Vec<_> = support
        .rows(atom.predicate())
        .map(|row| {
            Atom::new(
                row.predicate().clone(),
                super::row_values(row).cloned().collect(),
            )
            .unwrap()
        })
        .collect();
    let relation = Relation::from_atoms(atom.predicate(), &original, Limits::default()).unwrap();
    let all = relation.all(Limits::default()).unwrap();
    let keys: Vec<_> = atom
        .terms()
        .iter()
        .enumerate()
        .filter_map(|(column, term)| {
            let value = match term {
                Term::Constant(value) => Some(value),
                Term::Variable(variable) => prefix.get(*variable),
            }?;
            Some((column, value))
        })
        .collect();
    let query = relation.query(&keys, Limits::default()).unwrap();
    let selected = relation.select(&query, &all, Limits::default()).unwrap();
    let mut join = Join::new(
        literals,
        &crate::formula_binding::complete(prefix.iter().cloned()),
        variables,
        &support,
        &mut budget,
        location(),
    )?;
    assert_eq!(join.plan.patterns.len(), 1, "fixed single-pattern control");
    match route {
        Route::Indexed => {}
        Route::Scan => {
            join.probes[0] = Some(super::Probe::Indexed(super::delta::Rows::Posting(
                all.positions(),
            )));
        }
        Route::Columns => {
            // Contiguous source mapping is checked by construction. No new
            // tuple matcher, binding path or arithmetic evaluator is used.
            assert!(
                selected.positions().iter().all(|&position| {
                    relation.row(position).unwrap().source_index() == position
                })
            );
            join.probes[0] = Some(super::Probe::Indexed(super::delta::Rows::Posting(
                selected.positions(),
            )));
        }
    }
    let mut bindings = Vec::new();
    while let Some(binding) = join.next(limits, &mut budget, &mut counters, location())? {
        bindings.push(
            binding
                .slots()
                .iter()
                .map(|slot| slot.clone().expect("complete scope"))
                .collect(),
        );
    }
    assert_eq!(
        join.traversal,
        super::Traversal::Finished,
        "only complete cursor exhaustion is success"
    );
    // These controls traverse completed support. Undefined rows are omitted
    // by every route, but exhaustion cannot erase an all-undefined family.
    join.take_family().finish()?;
    Ok(bindings)
}

fn agree(
    support: &SupportCatalog,
    literals: &[LiteralIr],
    prefix: &[Value],
    variables: usize,
    expected: &[Vec<Value>],
) {
    for route in [Route::Indexed, Route::Scan, Route::Columns] {
        assert_eq!(
            evaluate(
                route,
                support,
                literals,
                prefix,
                variables,
                &FormulaLimits::default()
            )
            .unwrap(),
            expected
        );
    }
}

#[test]
fn bound_equalities_preserve_complete_join_bindings() {
    let support = support(
        (0..3)
            .flat_map(|left| (0..3).map(move |right| numbers(&[left, right, left + right])))
            .collect(),
    );
    let literals = [positive(pattern(vec![
        Term::Constant(Value::Number(1)),
        Term::Variable(0),
        Term::Variable(1),
    ]))];
    agree(&support, &literals, &numbers(&[2]), 2, &[numbers(&[2, 3])]);
}

#[test]
fn repeated_variables_still_require_the_matcher() {
    let support = support(vec![numbers(&[1, 2]), numbers(&[2, 2]), numbers(&[3, 4])]);
    let literals = [positive(pattern(vec![
        Term::Variable(0),
        Term::Variable(0),
    ]))];
    agree(&support, &literals, &[], 1, &[numbers(&[2])]);
}

#[test]
fn structural_matches_preserve_transactional_bindings() {
    let tuple = |left, right| {
        Value::from_nodes(
            vec![
                ValueNode::Tuple { arity: 2 },
                ValueNode::Number(left),
                ValueNode::Number(right),
            ],
            ValueLimits::default(),
        )
        .unwrap()
    };
    let unequal = tuple(1, 2);
    let equal = tuple(2, 2);
    let support = support(vec![vec![unequal], vec![equal.clone()]]);
    let literals = [LiteralIr::PatternAtom(PatternAtom {
        atom: pattern(vec![Term::Variable(0)]),
        arguments: vec![ArgumentPattern {
            position: 0,
            nodes: vec![
                PatternNode::Tuple(2),
                PatternNode::Slot(1),
                PatternNode::Slot(1),
            ],
        }],
    })];
    agree(
        &support,
        &literals,
        &[],
        2,
        &[vec![equal, Value::Number(2)]],
    );
}

#[test]
fn column_filters_retain_checked_binding_generation() {
    let support = support(vec![numbers(&[1, 4]), numbers(&[2, 5]), numbers(&[1, 6])]);
    let literals = [
        positive(pattern(vec![
            Term::Constant(Value::Number(1)),
            Term::Variable(0),
        ])),
        LiteralIr::Bind {
            target: 1,
            value: Expression {
                nodes: vec![
                    Operation::Variable(0),
                    Operation::Constant(Value::Number(2)),
                    Operation::Binary(BinaryOp::Mul, 0, 1),
                ],
            },
        },
        LiteralIr::Compare(
            Expression {
                nodes: vec![Operation::Variable(1)],
            },
            Comparison::Gt,
            number(10),
        ),
    ];
    agree(&support, &literals, &[], 2, &[numbers(&[6, 12])]);
}

#[test]
fn column_filters_preserve_arithmetic_failures() {
    let support = support(vec![numbers(&[1])]);
    let literals = [
        positive(pattern(vec![Term::Constant(Value::Number(1))])),
        LiteralIr::TupleCompare(vec![number(0)], Comparison::Eq, vec![number(1)]),
        LiteralIr::TupleCompare(
            vec![Expression {
                nodes: vec![
                    Operation::Constant(Value::Number(1)),
                    Operation::Constant(Value::Number(0)),
                    Operation::Binary(BinaryOp::Div, 0, 1),
                ],
            }],
            Comparison::Eq,
            vec![number(0)],
        ),
    ];
    for route in [Route::Indexed, Route::Scan, Route::Columns] {
        assert!(matches!(
            evaluate(
                route,
                &support,
                &literals,
                &[],
                0,
                &FormulaLimits::default()
            ),
            Err(FormulaFailure::Expansion(
                ExpansionFailure::Evaluation { .. }
            ))
        ));
    }
}

#[test]
fn bounded_join_failure_is_not_complete_exhaustion() {
    let support = support(vec![numbers(&[1]), numbers(&[2])]);
    let literals = [positive(pattern(vec![Term::Variable(0)]))];
    for route in [Route::Indexed, Route::Scan, Route::Columns] {
        assert!(matches!(
            evaluate(
                route,
                &support,
                &literals,
                &[],
                1,
                &FormulaLimits {
                    max_work: 0,
                    ..FormulaLimits::default()
                }
            ),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Work,
                ..
            })
        ));
    }
}
