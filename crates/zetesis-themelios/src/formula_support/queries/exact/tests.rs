//! Complete-key selection agrees with source rows and the original join route.

mod cheap;

use std::cell::Cell;

use super::*;
use crate::formula_support::{Counters, testing};
use crate::test_support::{compiled_atoms, location};
use crate::{FormulaLimits, FormulaResource};
use zetesis_algebra::{Cancellation, Stop};
use zetesis_core::{Atom, AtomPattern, Predicate, Sign, Term, Value, ValueLimits, ValueNode};

thread_local! {
    static CONTROL: Cell<(bool, usize)> = const { Cell::new((true, 0)) };
}

pub(super) fn enabled() -> bool {
    CONTROL.get().0
}

pub(super) fn record() {
    CONTROL.set((enabled(), CONTROL.get().1 + 1));
}

fn scoped<T>(enabled: bool, action: impl FnOnce() -> T) -> (T, usize) {
    struct Restore((bool, usize));
    impl Drop for Restore {
        fn drop(&mut self) {
            CONTROL.set(self.0);
        }
    }
    let _restore = Restore(CONTROL.replace((enabled, 0)));
    let result = action();
    (result, CONTROL.get().1)
}

fn atom(values: &[i32]) -> Atom {
    Atom::new(
        Predicate::new("p", values.len()).unwrap(),
        values.iter().copied().map(Value::Number).collect(),
    )
    .unwrap()
}

fn pattern(terms: Vec<Term>) -> AtomPattern {
    AtomPattern::new(Predicate::new("p", terms.len()).unwrap(), terms).unwrap()
}

#[test]
fn complete_keys_match_a_full_scan_without_retained_growth() {
    let atoms = [atom(&[3, 2]), atom(&[1, 2]), atom(&[3, 1])];
    let mut fixture = testing::Fixture::from_atoms(atoms, location());
    let pattern = pattern(vec![Term::Variable(0), Term::Variable(1)]);
    fixture.with(location(), |support, _, counters| {
        let limits = FormulaLimits::default();
        let source = support
            .resolve((&pattern).into(), &limits, counters, location())
            .unwrap()
            .unwrap();
        let bytes = support.live_bytes();
        for first in 0..=4 {
            for second in 0..=3 {
                let values = [Some(Value::Number(first)), Some(Value::Number(second))];
                let key = pattern.key(values.as_slice()).unwrap();
                let expected: Vec<_> = (0..source.row_count())
                    .filter(|&position| {
                        let row = source.row(position).unwrap();
                        (0..2)
                            .all(|column| key.value(column).unwrap() == row.value(column).unwrap())
                    })
                    .collect();
                let actual = support
                    .exact_at(
                        Some(source),
                        PositivePattern::Flat((&pattern).into()),
                        values.as_slice().into(),
                        (None, 0),
                        GroundingWork::new(&limits, counters, location()),
                    )
                    .unwrap()
                    .unwrap()
                    .collect::<Vec<_>>();
                assert_eq!(actual, expected, "({first}, {second})");
                assert_eq!(support.live_bytes(), bytes);
            }
        }
    });
}

#[test]
fn complete_key_selects_one_row_from_a_shared_posting() {
    let atoms = (0..8).flat_map(|first| (0..8).map(move |second| atom(&[first, second])));
    let mut fixture = testing::Fixture::from_atoms(atoms, location());
    let pattern = pattern(vec![Term::Variable(0), Term::Variable(1)]);
    fixture.with(location(), |support, _, counters| {
        let limits = FormulaLimits::default();
        let rows = support
            .resolve((&pattern).into(), &limits, counters, location())
            .unwrap();
        let values = [Some(Value::Number(3)), Some(Value::Number(4))];
        let posting = support
            .probe_at(
                rows,
                (&pattern).into(),
                values.as_slice().into(),
                &limits,
                counters,
                location(),
            )
            .unwrap()
            .unwrap();
        let exact = support
            .exact_at(
                rows,
                PositivePattern::Flat((&pattern).into()),
                values.as_slice().into(),
                (None, 0),
                GroundingWork::new(&limits, counters, location()),
            )
            .unwrap()
            .unwrap();
        assert_eq!(posting.len(), 8);
        assert!(posting.contains(&28));
        assert_eq!(exact, 28..29);
    });
}

#[test]
fn a_nullary_key_keeps_the_original_singleton() {
    let mut fixture = testing::Fixture::from_atoms([atom(&[])], location());
    let pattern = pattern(vec![]);
    fixture.with(location(), |support, _, counters| {
        let limits = FormulaLimits::default();
        let rows = support
            .resolve((&pattern).into(), &limits, counters, location())
            .unwrap();
        let actual = support
            .exact_at(
                rows,
                PositivePattern::Flat((&pattern).into()),
                (&[] as &[Option<Value>]).into(),
                (None, 0),
                GroundingWork::new(&limits, counters, location()),
            )
            .unwrap();
        assert_eq!(actual, Some(0..1));
    });
}

#[test]
fn constants_aliases_and_foreign_structural_values_keep_typed_identity() {
    let nested = Value::from_nodes(
        vec![
            ValueNode::Function {
                sign: Sign::Positive,
                name: "f".into(),
                arity: 1,
            },
            ValueNode::Number(7),
        ],
        ValueLimits::default(),
    )
    .unwrap();
    let predicate = Predicate::with_sign("p", 3, Sign::Negative).unwrap();
    let owned = Atom::new(
        predicate.clone(),
        vec![nested.clone(), nested.clone(), Value::String("7".into())],
    )
    .unwrap();
    let mut fixture = testing::Fixture::from_atoms([owned], location());
    let pattern = AtomPattern::new(
        predicate,
        vec![
            Term::Variable(0),
            Term::Variable(0),
            Term::Constant(Value::String("7".into())),
        ],
    )
    .unwrap();
    fixture.with(location(), |support, _, counters| {
        let limits = FormulaLimits::default();
        let rows = support
            .resolve((&pattern).into(), &limits, counters, location())
            .unwrap();
        for (value, expected) in [(nested, 0..1), (Value::Number(7), 0..0)] {
            assert_eq!(
                support
                    .exact_at(
                        rows,
                        PositivePattern::Flat((&pattern).into()),
                        [Some(value)].as_slice().into(),
                        (None, 0),
                        GroundingWork::new(&limits, counters, location()),
                    )
                    .unwrap(),
                Some(expected)
            );
        }
    });
}

#[test]
fn partial_and_missing_relations_decline_without_inventing_empty_membership() {
    let mut fixture = testing::Fixture::from_atoms([atom(&[1, 2])], location());
    let pattern = pattern(vec![Term::Variable(0), Term::Variable(1)]);
    fixture.with(location(), |support, _, counters| {
        let limits = FormulaLimits::default();
        let rows = support
            .resolve((&pattern).into(), &limits, counters, location())
            .unwrap();
        assert_eq!(
            support
                .exact_at(
                    rows,
                    PositivePattern::Flat((&pattern).into()),
                    [Some(Value::Number(1)), None].as_slice().into(),
                    (None, 0),
                    GroundingWork::new(&limits, counters, location()),
                )
                .unwrap(),
            None
        );
        assert_eq!(
            support
                .exact_at(
                    None,
                    PositivePattern::Flat((&pattern).into()),
                    (&[] as &[Option<Value>]).into(),
                    (None, 0),
                    GroundingWork::new(&limits, counters, location()),
                )
                .unwrap(),
            None
        );
        let error = support
            .exact_at(
                rows,
                PositivePattern::Flat((&pattern).into()),
                [Some(Value::Number(1))].as_slice().into(),
                (None, 0),
                GroundingWork::new(&limits, counters, location()),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            FormulaFailure::UnsafeVariable { variable: 1, .. }
        ));
    });
}

#[test]
fn exact_rows_obey_every_original_occurrence_delta_partition() {
    let mut fixture =
        testing::Fixture::from_atoms([atom(&[3]), atom(&[1]), atom(&[2])], location());
    let pattern = pattern(vec![Term::Variable(0)]);
    fixture.with(location(), |support, _, counters| {
        let limits = FormulaLimits::default();
        let rows = support
            .resolve((&pattern).into(), &limits, counters, location())
            .unwrap()
            .unwrap();
        assert_eq!(rows.old_rows(), 2);
        for (value, row) in [(3, 0), (1, 1), (2, 2)] {
            for pivot in [None, Some(0), Some(1), Some(2)] {
                let partition = delta::interval(pivot, 1, rows.old_rows(), rows.row_count());
                let expected = if partition.contains(&row) {
                    row..row + 1
                } else {
                    0..0
                };
                let actual = support
                    .exact_at(
                        Some(rows),
                        PositivePattern::Flat((&pattern).into()),
                        [Some(Value::Number(value))].as_slice().into(),
                        (pivot, 1),
                        GroundingWork::new(&limits, counters, location()),
                    )
                    .unwrap();
                assert_eq!(actual, Some(expected));
            }
        }
    });
}

#[test]
fn complete_key_work_refusals_preserve_the_exact_owner_and_receipt() {
    let mut fixture = testing::Fixture::from_atoms([atom(&[2, 3]), atom(&[1, 4])], location());
    let pattern = pattern(vec![Term::Variable(0), Term::Variable(1)]);
    fixture.with(location(), |support, _, setup| {
        let defaults = FormulaLimits::default();
        let rows = support
            .resolve((&pattern).into(), &defaults, setup, location())
            .unwrap();
        let before_bytes = support.live_bytes();
        let execute = |maximum, counters: &mut Counters| {
            let limits = FormulaLimits {
                max_work: maximum,
                ..defaults
            };
            support.exact_at(
                rows,
                PositivePattern::Flat((&pattern).into()),
                [Some(Value::Number(1)), Some(Value::Number(4))]
                    .as_slice()
                    .into(),
                (None, 0),
                GroundingWork::new(&limits, counters, location()),
            )
        };
        let mut full = Counters::default();
        assert_eq!(execute(defaults.max_work, &mut full).unwrap(), Some(1..2));
        let required = full.accounting.work;
        assert!(required > 0);
        for maximum in 0..required {
            let mut counters = Counters::default();
            let failure = execute(maximum, &mut counters).unwrap_err();
            assert!(matches!(failure, FormulaFailure::Limit {
                resource: FormulaResource::Work, limit, observed, ..
            } if limit == u128::from(maximum) && observed > limit));
            assert!(counters.accounting.work <= maximum);
            assert_eq!(support.live_bytes(), before_bytes);
        }
        let mut exact = Counters::default();
        assert_eq!(execute(required, &mut exact).unwrap(), Some(1..2));
        assert_eq!(exact.accounting.work, required);
    });
}

#[test]
fn cancelled_point_query_returns_no_selection() {
    let mut fixture = testing::Fixture::from_atoms([atom(&[1, 2])], location());
    let pattern = pattern(vec![Term::Variable(0), Term::Variable(1)]);
    fixture.with(location(), |support, _, setup| {
        let limits = FormulaLimits::default();
        let rows = support
            .resolve((&pattern).into(), &limits, setup, location())
            .unwrap();
        let cancellation = Cancellation::default();
        let mut counters = Counters::default();
        counters.attach_test_control(&cancellation);
        cancellation.cancel();
        let failure = support
            .exact_at(
                rows,
                PositivePattern::Flat((&pattern).into()),
                [Some(Value::Number(1)), Some(Value::Number(2))]
                    .as_slice()
                    .into(),
                (None, 0),
                GroundingWork::new(&limits, &mut counters, location()),
            )
            .unwrap_err();
        assert_eq!(failure.interruption(), Some(Stop::Cancelled));
        assert_eq!(counters.accounting.work, 0);
    });
}

#[test]
fn final_matcher_and_scalar_gates_keep_identical_ground_subjects() {
    for source in [
        "a(1).a(2).b(1,1).b(1,2).b(2,1).b(2,2).{p}.h:-a(X),b(X,X),p.",
        "-a(1).-a(2).b(1,1).b(1,2).b(2,1).b(2,2).{p}.h:- -a(X),b(X,X),not p.",
        "a(f(1)).a(f(2)).b(f(1),f(1)).b(f(1),f(2)).b(f(2),f(1)).b(f(2),f(2)).{p}.h:-a(X),b(X,X),not not p.",
        "a(f(1)).a(g(2)).b(1,1).b(1,2).b(2,1).b(2,2).{p}.h:-a(f(X)),b(X,X),p.",
        "a(1).a(2).b(1,1).b(1,2).b(2,1).b(2,2).{p}.h:-a(X),b(X,X),p,X<2.",
    ] {
        let compile = |enabled| {
            scoped(enabled, || {
                crate::formula_ground::ground(testing::prepare(source), None, None).unwrap()
            })
        };
        let (actual, points) = compile(true);
        let (reference, disabled) = compile(false);
        assert!(points > 0, "point route must be exercised: {source}");
        assert_eq!(disabled, 0);
        assert_eq!(
            compiled_atoms(&actual),
            compiled_atoms(&reference),
            "{source}"
        );
        assert_eq!(actual.theory.nodes(), reference.theory.nodes(), "{source}");
        assert_eq!(
            actual.theory.operands(),
            reference.theory.operands(),
            "{source}"
        );
        assert_eq!(actual.theory.roots(), reference.theory.roots(), "{source}");
        assert_eq!(actual.origins, reference.origins, "{source}");
        assert_eq!(actual.warnings, reference.warnings, "{source}");
    }
}
