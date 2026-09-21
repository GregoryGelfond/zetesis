use std::cell::Cell;

use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Location, Span};
use themelios_program::program::{DefaultNegation, Relation};
use themelios_program::term::BinaryOp;
use zetesis_core::{Atom, AtomPattern, Predicate, Term, Value, ValueLimits, ValueNode};

use super::RowFilter;
use crate::expansion::Budget;
use crate::formula_ir::{Expression, HeadIr, LiteralIr, Operation, RuleIr};
use crate::formula_pattern::{ArgumentPattern, PatternAtom, PatternNode};
use crate::formula_support::{Counters, Join, Support, SupportCatalog};
use crate::{ExpansionLimits, FormulaFailure, FormulaLimits};

fn location() -> Location {
    Location {
        source: SourceId::new(74),
        span: Span::empty(ByteOffset::new(12)),
    }
}

fn atom(name: &str, values: &[i32]) -> Atom {
    Atom::new(
        Predicate::new(name, values.len()).unwrap(),
        values.iter().copied().map(Value::Number).collect(),
    )
    .unwrap()
}

fn pattern(name: &str, variables: &[usize]) -> LiteralIr {
    LiteralIr::Atom(
        DefaultNegation::None,
        AtomPattern::new(
            Predicate::new(name, variables.len()).unwrap(),
            variables.iter().copied().map(Term::Variable).collect(),
        )
        .unwrap(),
    )
}

fn rule(body: Vec<LiteralIr>, variables: usize) -> RuleIr {
    RuleIr {
        head: HeadIr::Normal(None),
        body,
        body_variables: variables,
        bindings: None,
        variables,
        origins: vec![location()],
        location: location(),
    }
}

fn with_support<T>(
    atoms: Vec<Atom>,
    consume: impl FnOnce(&Support<'_>, &mut Budget, &mut Counters) -> T,
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
    consume(
        &support,
        &mut Budget::new(ExpansionLimits::default(), usize::MAX),
        &mut Counters::default(),
    )
}

struct Select<F>(F);

impl<F> RowFilter for Select<F>
where
    F: Fn(zetesis_core::relation::Row<'_, '_>) -> bool,
{
    fn permits(
        &self,
        row: zetesis_core::relation::Row<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<bool, FormulaFailure> {
        counters.work(limits, location)?;
        Ok(self.0(row))
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Run {
    rows: Vec<(Vec<Option<Value>>, bool)>,
    failure: Option<String>,
    work: u64,
    substitutions: u64,
}

fn unfiltered(wrapped: bool, max_work: u64) -> Run {
    let rule = rule(vec![pattern("p", &[0, 1]), pattern("p", &[1, 2])], 3);
    with_support(
        vec![atom("p", &[1, 1]), atom("p", &[1, 2]), atom("p", &[2, 3])],
        |support, budget, counters| {
            let mut ordinary = Join::rule(&rule, support, budget).unwrap();
            let mut filtered = Join::filtered_rule(&rule, support, None, None, budget).unwrap();
            let limits = FormulaLimits {
                max_work,
                ..FormulaLimits::default()
            };
            let mut rows = Vec::new();
            let failure = loop {
                let next = if wrapped {
                    filtered.next_row(&limits, budget, counters, location())
                } else {
                    ordinary.next_row(&limits, budget, counters, location())
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
fn absent_selection_preserves_every_work_prefix() {
    let baseline = unfiltered(false, FormulaLimits::default().max_work);
    assert!(baseline.failure.is_none());
    assert_eq!(baseline.rows.len(), 3);
    for limit in 0..=baseline.work {
        assert_eq!(
            unfiltered(true, limit),
            unfiltered(false, limit),
            "limit {limit}"
        );
    }
}

#[test]
fn selected_rows_preserve_correlated_backtracking() {
    let rule = rule(vec![pattern("p", &[0, 1]), pattern("p", &[1, 2])], 3);
    let selection = Select(|row: zetesis_core::relation::Row<'_, '_>| row.value(0) != row.value(1));
    with_support(
        vec![
            atom("p", &[1, 1]),
            atom("p", &[1, 2]),
            atom("p", &[2, 2]),
            atom("p", &[2, 3]),
            atom("p", &[2, 4]),
            atom("p", &[3, 5]),
        ],
        |support, budget, counters| {
            let mut rows =
                Join::filtered_rule(&rule, support, Some(&selection), None, budget).unwrap();
            let mut selected = Vec::new();
            while let Some(row) = rows
                .next_row(&FormulaLimits::default(), budget, counters, location())
                .unwrap()
            {
                assert!(row.passes);
                selected.push(row.values.slots().to_vec());
            }
            let expected = [[1, 2, 3], [1, 2, 4], [2, 3, 5]]
                .map(|values| values.map(|value| Some(Value::Number(value))).to_vec());
            assert_eq!(selected, expected);
            assert_eq!(counters.substitutions, 3);
        },
    );
}

#[test]
fn rejected_rows_copy_no_scalar_payload() {
    let rule = rule(vec![pattern("p", &[0])], 1);
    let selection = Select(|_: zetesis_core::relation::Row<'_, '_>| false);
    let value = Value::Symbol("payload".into());
    with_support(
        vec![Atom::new(Predicate::new("p", 1).unwrap(), vec![value]).unwrap()],
        |support, budget, counters| {
            let mut rows =
                Join::filtered_rule(&rule, support, Some(&selection), None, budget).unwrap();
            let mut no_payload = Budget::new(
                ExpansionLimits {
                    max_scalar_bytes: 0,
                    ..ExpansionLimits::default()
                },
                usize::MAX,
            );
            assert!(
                rows.next_row(
                    &FormulaLimits::default(),
                    &mut no_payload,
                    counters,
                    location()
                )
                .unwrap()
                .is_none()
            );
            assert_eq!(no_payload.usage().scalar_bytes, 0);
            assert_eq!(counters.substitutions, 0);
        },
    );
}

#[test]
fn structural_rows_are_selected_before_matching() {
    let predicate = Predicate::new("p", 1).unwrap();
    let rule = rule(
        vec![LiteralIr::PatternAtom(PatternAtom {
            atom: AtomPattern::new(predicate.clone(), vec![Term::Variable(0)]).unwrap(),
            arguments: vec![ArgumentPattern {
                position: 0,
                nodes: vec![
                    PatternNode::Tuple(2),
                    PatternNode::Slot(1),
                    PatternNode::Slot(1),
                ],
            }],
        })],
        2,
    );
    let value = Value::from_nodes(
        vec![
            ValueNode::Tuple { arity: 2 },
            ValueNode::Number(7),
            ValueNode::Number(7),
        ],
        ValueLimits::default(),
    )
    .unwrap();
    let selection = Select(|_: zetesis_core::relation::Row<'_, '_>| false);
    with_support(
        vec![Atom::new(predicate, vec![value]).unwrap()],
        |support, budget, counters| {
            let mut rows =
                Join::filtered_rule(&rule, support, Some(&selection), None, budget).unwrap();
            let mut no_matching = Budget::new(
                ExpansionLimits {
                    max_term_work: 0,
                    max_scalar_bytes: 0,
                    ..ExpansionLimits::default()
                },
                usize::MAX,
            );
            assert!(
                rows.next_row(
                    &FormulaLimits::default(),
                    &mut no_matching,
                    counters,
                    location()
                )
                .unwrap()
                .is_none()
            );
            assert_eq!(rows.join.values, vec![None, None]);
            assert_eq!(no_matching.usage().term_work, 0);
            assert_eq!(counters.substitutions, 0);
        },
    );
}

#[test]
fn rejected_rows_do_not_evaluate_partial_scalars() {
    let division = || Expression {
        nodes: vec![
            Operation::Constant(Value::Number(1)),
            Operation::Variable(0),
            Operation::Binary(BinaryOp::Div, 0, 1),
        ],
    };
    let rule = rule(
        vec![
            pattern("p", &[0]),
            LiteralIr::Compare(division(), Relation::Eq, division()),
        ],
        1,
    );
    let selection = Select(|_: zetesis_core::relation::Row<'_, '_>| false);
    with_support(vec![atom("p", &[0])], |support, budget, counters| {
        let mut rows = Join::filtered_rule(&rule, support, Some(&selection), None, budget).unwrap();
        // This still uses complete arithmetic coverage internally. The filter
        // belongs to a post-admission witness scan, so it precedes evaluation.
        assert!(rows.join.coverage == crate::formula_support::Coverage::Complete);
        let before = budget.usage();
        assert!(
            rows.next_row(&FormulaLimits::default(), budget, counters, location())
                .unwrap()
                .is_none()
        );
        assert_eq!(budget.usage(), before);
        assert!(rows.join.family.zero.is_none());
    });
}

#[test]
fn empty_positive_inputs_keep_their_scalar_row() {
    let constant = || Expression {
        nodes: vec![Operation::Constant(Value::Number(1))],
    };
    let rule = rule(
        vec![LiteralIr::Compare(constant(), Relation::Eq, constant())],
        0,
    );
    let calls = Cell::new(0);
    let selection = Select(|_: zetesis_core::relation::Row<'_, '_>| {
        calls.set(calls.get() + 1);
        false
    });
    with_support(Vec::new(), |support, budget, counters| {
        let mut rows = Join::filtered_rule(&rule, support, Some(&selection), None, budget).unwrap();
        let row = rows
            .next_row(&FormulaLimits::default(), budget, counters, location())
            .unwrap()
            .unwrap();
        assert!(row.passes);
        assert!(row.values.slots().is_empty());
        assert!(
            rows.next_row(&FormulaLimits::default(), budget, counters, location())
                .unwrap()
                .is_none()
        );
        assert_eq!(calls.get(), 0);
        assert_eq!(counters.substitutions, 1);
    });
}

struct RefuseSecond(Cell<usize>);

impl RowFilter for RefuseSecond {
    fn permits(
        &self,
        _: zetesis_core::relation::Row<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<bool, FormulaFailure> {
        counters.work(limits, location)?;
        self.0.set(self.0.get() + 1);
        if self.0.get() == 2 {
            Err(FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Owner,
                location,
            })
        } else {
            Ok(true)
        }
    }
}

#[test]
fn filter_failure_retains_the_completed_prefix() {
    let rule = rule(vec![pattern("p", &[0])], 1);
    let selection = RefuseSecond(Cell::new(0));
    with_support(
        vec![atom("p", &[1]), atom("p", &[2])],
        |support, budget, counters| {
            let mut rows =
                Join::filtered_rule(&rule, support, Some(&selection), None, budget).unwrap();
            let first = rows
                .next_row(&FormulaLimits::default(), budget, counters, location())
                .unwrap()
                .unwrap();
            assert_eq!(first.values.read(0, location()).unwrap(), &Value::Number(1));
            let before = counters.work;
            let error = rows.next_row(&FormulaLimits::default(), budget, counters, location());
            assert!(matches!(error, Err(FormulaFailure::SupportRelation {
            error: zetesis_core::relation::Failure::Owner,
            location: actual,
        }) if actual == location()));
            assert_eq!(selection.0.get(), 2);
            assert_eq!(counters.substitutions, 1);
            assert!(counters.work > before);
            assert_eq!(rows.join.values, vec![None]);
        },
    );
}

#[test]
fn filter_cancellation_stops_before_binding() {
    let rule = rule(vec![pattern("p", &[0])], 1);
    let cancellation = zetesis_cpu::Cancellation::default();
    let calls = Cell::new(0);
    let selection = Select(|_: zetesis_core::relation::Row<'_, '_>| {
        calls.set(calls.get() + 1);
        cancellation.cancel();
        true
    });
    with_support(vec![atom("p", &[7])], |support, budget, _| {
        let mut rows = Join::filtered_rule(&rule, support, Some(&selection), None, budget).unwrap();
        let mut accounting = crate::formula_support::Accounting::default();
        let result = accounting.with_cancellation(&cancellation, |counters| {
            rows.next_row(&FormulaLimits::default(), budget, counters, location())
                .map(|row| row.is_some())
        });
        assert!(matches!(result, Err(FormulaFailure::Interrupted {
            reason: zetesis_cpu::Stop::Cancelled,
            location: actual,
        }) if actual == location()));
        assert_eq!(calls.get(), 1);
        assert_eq!(accounting.substitutions, 0);
        assert!(accounting.work > 0);
        assert_eq!(rows.join.values, vec![None]);
    });
}

#[test]
fn source_atom_positions_match_relation_occurrences() {
    with_support(
        vec![atom("p", &[9]), atom("p", &[2]), atom("q", &[3])],
        |support, _, _| {
            for (predicate, atoms) in support.source_atoms() {
                assert_eq!(atoms.len(), support.row_count(predicate));
                for (position, atom) in atoms.iter().enumerate() {
                    let row = support.row(predicate, position).unwrap();
                    assert!(std::ptr::eq(row.predicate(), predicate));
                    assert_eq!(row.predicate(), atom.predicate());
                    assert_eq!(row.position(), position);
                    assert_eq!(row.value(0), atom.values().first());
                }
            }
        },
    );
}
