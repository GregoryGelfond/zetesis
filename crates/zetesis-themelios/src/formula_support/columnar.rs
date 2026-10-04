//! Complete the experimental equality selection with the existing join.
//!
//! One fixed positive pattern keeps its selected rows valid for the whole
//! cursor. This is a bounded caller control, not a production join strategy.
//! The ordinary probe, full scan and column filter retain distinct work
//! populations. No equality selection discharges matching or arithmetic.

use crate::ProgramSite;
use crate::formula_support::Context;
use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Span};
use themelios_program::program::{DefaultNegation, Relation as Comparison};
use themelios_program::term::BinaryOp;
use zetesis_core::relation::Limits;
use zetesis_core::{
    Atom, AtomPattern, Predicate, TemplateTerm, Term, Value, ValueLimits, ValueNode, ValueNodeRef,
};

use super::{Budget, Computation, Counters, Join, Support, SupportCatalog};
use crate::formula_ir::{Expression, LiteralIr, Operation};
use crate::formula_pattern::{ArgumentPattern, PatternAtom, PatternNode};
use crate::{ExpansionFailure, ExpansionLimits, FormulaFailure, FormulaLimits, FormulaResource};

#[derive(Clone, Copy)]
enum Route {
    Indexed,
    Scan,
    Columns,
}

fn location() -> ProgramSite {
    ProgramSite::source(themelios_base::span::Location {
        source: SourceId::new(0),
        span: Span::empty(ByteOffset::new(0)),
    })
}

fn support(rows: Vec<Vec<Value>>) -> SupportCatalog {
    let mut support = SupportCatalog::default();
    for values in rows {
        let atom = Atom::new(Predicate::new("row", values.len()).unwrap(), values).unwrap();
        support = support
            .insert(
                &atom,
                &FormulaLimits::default(),
                &mut Counters::default(),
                location(),
            )
            .unwrap();
    }
    support
}

fn pattern(catalog: &mut SupportCatalog, terms: Vec<Term>) -> super::components::Pattern {
    let pattern = AtomPattern::new(Predicate::new("row", terms.len()).unwrap(), terms).unwrap();
    super::testing::admit_pattern(catalog, &pattern, &mut Counters::default(), location())
}

fn positive(pattern: super::components::Pattern) -> LiteralIr {
    LiteralIr::Atom(DefaultNegation::None, pattern)
}

fn numbers(values: &[i32]) -> Vec<Value> {
    values.iter().map(|&value| Value::Number(value)).collect()
}

fn constant(catalog: &mut SupportCatalog, value: i32) -> Operation {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut admission = catalog
        .component_admission(&limits, &mut counters, location())
        .unwrap();
    let scalar = admission
        .scalar(
            (&Value::Number(value)).into(),
            &limits,
            &mut counters,
            location(),
        )
        .unwrap();
    admission
        .finish(&limits, &mut counters, location())
        .unwrap();
    Operation::Constant(scalar)
}

fn tuple(catalog: &mut SupportCatalog, arity: usize) -> super::components::Constructor {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut admission = catalog
        .component_admission(&limits, &mut counters, location())
        .unwrap();
    let constructor = admission
        .constructor(
            ValueNodeRef::Tuple { arity },
            &limits,
            &mut counters,
            location(),
        )
        .unwrap();
    admission
        .finish(&limits, &mut counters, location())
        .unwrap();
    constructor
}

fn number(catalog: &mut SupportCatalog, value: i32) -> Expression {
    Expression {
        nodes: vec![constant(catalog, value)],
    }
}

fn evaluate(
    route: Route,
    catalog: &mut SupportCatalog,
    literals: &[LiteralIr],
    prefix: &[Value],
    variables: usize,
    limits: &FormulaLimits,
) -> Result<Vec<Vec<Value>>, FormulaFailure> {
    let mut budget = Budget::new(ExpansionLimits::default(), usize::MAX);
    let mut counters = Counters::default();
    let (relations, mut append) =
        catalog.split(&FormulaLimits::default(), &mut counters, location())?;
    let support = Support::indexed(&relations, &FormulaLimits::default(), &counters, location())?;
    let mut computation = Computation::new(&mut append, &support);
    let atom = match &literals[0] {
        LiteralIr::Atom(DefaultNegation::None, atom) => atom,
        LiteralIr::PatternAtom(pattern) => &pattern.atom,
        _ => panic!("positive first pattern"),
    };
    let atom =
        computation.static_pattern(*atom, &FormulaLimits::default(), &mut counters, location())?;
    let relation = support
        .relation_with(
            atom.predicate(),
            &FormulaLimits::default(),
            &mut counters,
            location(),
        )?
        .expect("fixture relation");
    let all = relation.all(Limits::default()).unwrap();
    let keys: Vec<_> = atom
        .terms()
        .iter()
        .enumerate()
        .filter_map(|(column, term)| {
            let value = match term {
                TemplateTerm::Constant(value) => Some(value),
                TemplateTerm::Variable(variable) => prefix.get(variable).map(Into::into),
            }?;
            Some((column, value))
        })
        .collect();
    let query = relation.query(&keys, Limits::default()).unwrap();
    let selected = relation.select(&query, &all, Limits::default()).unwrap();
    let prefix_values: Vec<_> = prefix.iter().cloned().map(Some).collect();
    let prefix_binding =
        super::testing::binding(&prefix_values, &mut computation, &mut counters, location());
    let mut join = Join::new(
        literals,
        &prefix_binding,
        variables,
        &support,
        &mut budget,
        Context::new(&computation, limits, &mut counters, location()),
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
    while let Some(binding) = join.next(
        &mut computation,
        limits,
        &mut budget,
        &mut counters,
        location(),
    )? {
        bindings.push(
            (0..binding.len())
                .map(|slot| {
                    binding
                        .read(slot, computation.read(), location())
                        .unwrap()
                        .to_value(ValueLimits::default())
                        .unwrap()
                })
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
    support: &mut SupportCatalog,
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
    let mut support = support(
        (0..3)
            .flat_map(|left| (0..3).map(move |right| numbers(&[left, right, left + right])))
            .collect(),
    );
    let literals = [positive(pattern(
        &mut support,
        vec![
            Term::Constant(Value::Number(1)),
            Term::Variable(0),
            Term::Variable(1),
        ],
    ))];
    agree(
        &mut support,
        &literals,
        &numbers(&[2]),
        2,
        &[numbers(&[2, 3])],
    );
}

#[test]
fn repeated_variables_still_require_the_matcher() {
    let mut support = support(vec![numbers(&[1, 2]), numbers(&[2, 2]), numbers(&[3, 4])]);
    let literals = [positive(pattern(
        &mut support,
        vec![Term::Variable(0), Term::Variable(0)],
    ))];
    agree(&mut support, &literals, &[], 1, &[numbers(&[2])]);
}

#[test]
fn structural_matches_preserve_transactional_bindings() {
    let tuple_value = |left, right| {
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
    let unequal = tuple_value(1, 2);
    let equal = tuple_value(2, 2);
    let mut support = support(vec![vec![unequal], vec![equal.clone()]]);
    let literals = [LiteralIr::PatternAtom(PatternAtom {
        atom: pattern(&mut support, vec![Term::Variable(0)]),
        arguments: vec![ArgumentPattern {
            position: 0,
            nodes: vec![
                PatternNode::Constructor(tuple(&mut support, 2)),
                PatternNode::Slot(1),
                PatternNode::Slot(1),
            ],
        }],
    })];
    agree(
        &mut support,
        &literals,
        &[],
        2,
        &[vec![equal, Value::Number(2)]],
    );
}

#[test]
fn column_filters_retain_checked_binding_generation() {
    let mut support = support(vec![numbers(&[1, 4]), numbers(&[2, 5]), numbers(&[1, 6])]);
    let literals = [
        positive(pattern(
            &mut support,
            vec![Term::Constant(Value::Number(1)), Term::Variable(0)],
        )),
        LiteralIr::Bind {
            target: 1,
            value: Expression {
                nodes: vec![
                    Operation::Variable(0),
                    constant(&mut support, 2),
                    Operation::Binary(BinaryOp::Mul, 0, 1),
                ],
            },
        },
        LiteralIr::Compare(
            Expression {
                nodes: vec![Operation::Variable(1)],
            },
            Comparison::Gt,
            number(&mut support, 10),
        ),
    ];
    agree(&mut support, &literals, &[], 2, &[numbers(&[6, 12])]);
}

#[test]
fn column_filters_preserve_arithmetic_failures() {
    let mut support = support(vec![numbers(&[1])]);
    let literals = [
        positive(pattern(
            &mut support,
            vec![Term::Constant(Value::Number(1))],
        )),
        LiteralIr::TupleCompare(
            vec![number(&mut support, 0)],
            Comparison::Eq,
            vec![number(&mut support, 1)],
        ),
        LiteralIr::TupleCompare(
            vec![Expression {
                nodes: vec![
                    constant(&mut support, 1),
                    constant(&mut support, 0),
                    Operation::Binary(BinaryOp::Div, 0, 1),
                ],
            }],
            Comparison::Eq,
            vec![number(&mut support, 0)],
        ),
    ];
    for route in [Route::Indexed, Route::Scan, Route::Columns] {
        assert!(matches!(
            evaluate(
                route,
                &mut support,
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
    let mut support = support(vec![numbers(&[1]), numbers(&[2])]);
    let literals = [positive(pattern(&mut support, vec![Term::Variable(0)]))];
    for route in [Route::Indexed, Route::Scan, Route::Columns] {
        assert!(matches!(
            evaluate(
                route,
                &mut support,
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
