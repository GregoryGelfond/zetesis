use super::*;
use crate::formula_support::testing::Fixture;
use crate::test_support::location;
use crate::{FormulaLimits, FormulaResource};
use themelios_program::term::BinaryOp;
use zetesis_core::{Atom, AtomPattern, Predicate, Term, Value};

fn fixture(values: &[[i32; 2]]) -> (Fixture, crate::formula_support::components::Pattern) {
    let mut fixture = Fixture::from_atoms(
        values.iter().map(|row| {
            Atom::new(
                Predicate::new("p", 2).unwrap(),
                row.iter().copied().map(Value::Number).collect(),
            )
            .unwrap()
        }),
        location(),
    );
    let pattern = fixture.pattern(
        &AtomPattern::new(
            Predicate::new("p", 2).unwrap(),
            vec![Term::Variable(0), Term::Variable(1)],
        )
        .unwrap(),
        location(),
    );
    (fixture, pattern)
}

fn expression(operator: BinaryOp) -> Expression {
    Expression::new(vec![
        Operation::Variable(0),
        Operation::Variable(1),
        Operation::Binary(operator, 0, 1),
    ])
}

fn positions(rows: &Rows<'_, '_>) -> Vec<usize> {
    (0..).map_while(|position| rows.get(position)).collect()
}

#[test]
fn keys_keep_source_order_across_cache_growth() {
    let (mut fixture, pattern) = fixture(&[[3, 0], [1, 2], [2, 1], [0, 0], [4, 4]]);
    fixture.with(location(), |support, computation, counters| {
        let limits = FormulaLimits::default();
        let pattern = computation
            .static_pattern(pattern, &limits, counters, location())
            .unwrap();
        let source = support
            .resolve(pattern, &limits, counters, location())
            .unwrap()
            .unwrap();
        let first = support
            .expression_key(
                source,
                &expression(BinaryOp::Add),
                pattern,
                3,
                0..source.row_count(),
                Context::new(computation, &limits, counters, location()),
            )
            .unwrap()
            .unwrap();
        let expected = (0..source.row_count())
            .filter(|&position| {
                let row = source.row(position).unwrap();
                number(row.value(0).unwrap().descriptor()).unwrap()
                    + number(row.value(1).unwrap().descriptor()).unwrap()
                    == 3
            })
            .collect::<Vec<_>>();
        assert_eq!(positions(&first), expected);
        let second = support
            .expression_key(
                source,
                &expression(BinaryOp::Sub),
                pattern,
                0,
                0..source.row_count(),
                Context::new(computation, &limits, counters, location()),
            )
            .unwrap()
            .unwrap();
        assert!(!positions(&second).is_empty());
        assert_eq!(
            support
                .expression_keys
                .cache
                .borrow()
                .as_ref()
                .unwrap()
                .entries
                .len(),
            2
        );
        assert_eq!(positions(&first), expected);
        let cut = support
            .expression_key(
                source,
                &expression(BinaryOp::Add),
                pattern,
                3,
                1..3,
                Context::new(computation, &limits, counters, location()),
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            positions(&cut),
            expected
                .into_iter()
                .filter(|row| (1..3).contains(row))
                .collect::<Vec<_>>()
        );
    });
}

#[test]
fn a_failed_key_declines_the_entire_index() {
    let (mut fixture, pattern) = fixture(&[[4, 2], [1, 0], [8, 4]]);
    fixture.with(location(), |support, computation, counters| {
        let limits = FormulaLimits::default();
        let pattern = computation
            .static_pattern(pattern, &limits, counters, location())
            .unwrap();
        let source = support
            .resolve(pattern, &limits, counters, location())
            .unwrap()
            .unwrap();
        let entry_count = support.entries.get();
        for _ in 0..2 {
            assert!(
                support
                    .expression_key(
                        source,
                        &expression(BinaryOp::Div),
                        pattern,
                        2,
                        0..source.row_count(),
                        Context::new(computation, &limits, counters, location())
                    )
                    .unwrap()
                    .is_none()
            );
        }
        let cache = support.expression_keys.cache.borrow();
        let entries = &cache.as_ref().unwrap().entries;
        assert_eq!(entries.len(), 1);
        assert!(entries[0].integer.is_none());
        assert_eq!(support.entries.get(), entry_count);
    });
}

#[test]
fn expression_indexes_consume_the_shared_entry_limit() {
    let (mut fixture, pattern) = fixture(&[[1, 1], [2, 2], [3, 3]]);
    fixture.with(location(), |support, computation, counters| {
        let defaults = FormulaLimits::default();
        let pattern = computation
            .static_pattern(pattern, &defaults, counters, location())
            .unwrap();
        let source = support
            .resolve(pattern, &defaults, counters, location())
            .unwrap()
            .unwrap();
        let before = support.entries.get();
        let limits = FormulaLimits {
            max_support_index_entries: before + 2,
            ..defaults
        };
        assert!(matches!(
            support.expression_key(
                source,
                &expression(BinaryOp::Add),
                pattern,
                4,
                0..source.row_count(),
                Context::new(computation, &limits, counters, location())
            ),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::SupportIndexEntries,
                ..
            })
        ));
        assert_eq!(support.entries.get(), before);
        assert!(support.expression_keys.cache.borrow().is_none());
    });
}

fn bounded_query(allowance: u64) -> u64 {
    let (mut fixture, pattern) = fixture(&[[3, 0], [1, 2], [2, 1], [0, 0]]);
    fixture.with(location(), |support, computation, counters| {
        let defaults = FormulaLimits::default();
        let pattern = computation
            .static_pattern(pattern, &defaults, counters, location())
            .unwrap();
        let source = support
            .resolve(pattern, &defaults, counters, location())
            .unwrap()
            .unwrap();
        let before = counters.accounting.work;
        let limits = FormulaLimits {
            max_work: before.saturating_add(allowance),
            ..defaults
        };
        let result = support.expression_key(
            source,
            &expression(BinaryOp::Add),
            pattern,
            3,
            0..source.row_count(),
            Context::new(computation, &limits, counters, location()),
        );
        let spent = counters.accounting.work - before;
        if allowance == u64::MAX {
            assert_eq!(positions(&result.unwrap().unwrap()).len(), 3);
        } else {
            assert!(matches!(
                result,
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::Work,
                    ..
                })
            ));
            // A stop may follow publication but can never retain a partially
            // checked or partially sorted certificate.
            if let Some(cache) = support.expression_keys.cache.borrow().as_ref() {
                for entry in &cache.entries {
                    let entries = entry.integer.as_ref().unwrap().entries.slice();
                    assert_eq!(entries.len(), source.row_count());
                    assert!(entries.windows(2).all(|pair| pair[0] <= pair[1]));
                }
            }
            let restored = support
                .expression_key(
                    source,
                    &expression(BinaryOp::Add),
                    pattern,
                    3,
                    0..source.row_count(),
                    Context::new(computation, &defaults, counters, location()),
                )
                .unwrap()
                .unwrap();
            assert_eq!(positions(&restored).len(), 3);
        }
        spent
    })
}

#[test]
fn every_stopped_build_keeps_only_complete_certificates() {
    let needed = bounded_query(u64::MAX);
    for allowance in 0..needed {
        bounded_query(allowance);
    }
}

#[test]
fn a_cancelled_cache_hit_returns_no_posting() {
    let (mut fixture, pattern) = fixture(&[[1, 1], [2, 2]]);
    fixture.with(location(), |support, computation, counters| {
        let limits = FormulaLimits::default();
        let pattern = computation
            .static_pattern(pattern, &limits, counters, location())
            .unwrap();
        let source = support
            .resolve(pattern, &limits, counters, location())
            .unwrap()
            .unwrap();
        let first = support
            .expression_key(
                source,
                &expression(BinaryOp::Add),
                pattern,
                2,
                0..source.row_count(),
                Context::new(computation, &limits, counters, location()),
            )
            .unwrap()
            .unwrap();
        assert_eq!(positions(&first).len(), 1);
        let cancellation = zetesis_cpu::Cancellation::default();
        cancellation.cancel();
        counters.attach_test_control(&cancellation);
        assert!(matches!(
            support.expression_key(
                source,
                &expression(BinaryOp::Add),
                pattern,
                2,
                0..source.row_count(),
                Context::new(computation, &limits, counters, location())
            ),
            Err(FormulaFailure::Interrupted {
                reason: zetesis_cpu::Stop::Cancelled,
                ..
            })
        ));
        assert_eq!(positions(&first).len(), 1);
    });
}

#[test]
fn invalid_expression_arguments_decline_without_panicking() {
    let (mut fixture, _) = fixture(&[[1, 2]]);
    fixture.with(location(), |_, computation, counters| {
        let limits = FormulaLimits::default();
        let mut context = Context::new(&*computation, &limits, counters, location());
        let mut scratch = buffer(&mut context).unwrap();
        for nodes in [
            vec![
                Operation::Variable(0),
                Operation::Unary(themelios_program::term::UnaryOp::Negate, 1),
            ],
            vec![
                Operation::Variable(0),
                Operation::Binary(BinaryOp::Add, 0, 9),
            ],
            vec![Operation::Absolute(0)],
        ] {
            assert_eq!(
                evaluate(&nodes, &mut scratch, &mut context, |_, _| Ok(Some(1))).unwrap(),
                None
            );
        }
    });
}

#[test]
fn a_missing_constant_is_an_owner_failure_not_a_decline() {
    let scalar = Fixture::default().scalar(&Value::Number(37), location());
    let (mut fixture, pattern) = fixture(&[[1, 2]]);
    fixture.with(location(), |support, computation, counters| {
        let limits = FormulaLimits::default();
        let pattern = computation
            .static_pattern(pattern, &limits, counters, location())
            .unwrap();
        let source = support
            .resolve(pattern, &limits, counters, location())
            .unwrap()
            .unwrap();
        let key = Expression::new(vec![
            Operation::Variable(0),
            Operation::Constant(scalar),
            Operation::Binary(BinaryOp::Add, 0, 1),
        ]);
        assert!(matches!(
            support.expression_key(
                source,
                &key,
                pattern,
                38,
                0..1,
                Context::new(computation, &limits, counters, location())
            ),
            Err(FormulaFailure::TemplateCatalog { .. })
        ));
        assert!(support.expression_keys.cache.borrow().is_none());
    });
}

#[test]
fn a_storage_refusal_never_becomes_a_data_decline() {
    let (mut fixture, pattern) = fixture(&[[1, 2]]);
    fixture.with(location(), |support, computation, counters| {
        let defaults = FormulaLimits::default();
        let pattern = computation
            .static_pattern(pattern, &defaults, counters, location())
            .unwrap();
        let source = support
            .resolve(pattern, &defaults, counters, location())
            .unwrap()
            .unwrap();
        let limits = FormulaLimits {
            max_support_bytes: support.live_bytes(),
            ..defaults
        };
        assert!(matches!(
            support.expression_key(
                source,
                &expression(BinaryOp::Add),
                pattern,
                3,
                0..1,
                Context::new(computation, &limits, counters, location())
            ),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::SupportBytes,
                ..
            })
        ));
        assert!(support.expression_keys.cache.borrow().is_none());
    });
}

mod ranges;
