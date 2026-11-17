use super::*;
use crate::formula_support::row_blocks::cursor_rows::CursorRows;
use crate::formula_support::{Counters, Ir, Join, testing};
use crate::{FormulaLimits, FormulaResource};

const SOURCE: &str = "d(1..4).e(1..4).out(X,Y):-d(X),e(Y),X<Y.";

#[test]
fn leaf_permission_is_producer_only_and_keeps_original_plan_and_rows() {
    testing::with_completed_source(SOURCE, |program, support, computation, counters| {
        let rule = program.rules_without_facts().last().unwrap();
        let limits = FormulaLimits::default();
        let mut budget = testing::budget();
        let mut cursor = Join::rule(
            Ir::Source(rule),
            support,
            computation,
            &limits,
            &mut budget,
            counters,
        )
        .unwrap();
        let plan = std::ptr::from_ref(&*cursor.plan);
        let positions = cursor.positions.clone();
        assert!(
            CursorRows::prepare_emission(rule, &mut cursor, computation, &limits, counters)
                .unwrap()
                .is_none()
        );
        {
            let rows =
                CursorRows::prepare_producer(rule, &mut cursor, computation, &limits, counters)
                    .unwrap()
                    .unwrap();
            assert_eq!(std::ptr::from_ref(rows.descriptor().plan), plan);
            assert!(rows.descriptor().len() > 1);
        }
        assert_eq!(cursor.positions, positions);
        let mut count = 0;
        while cursor
            .next(computation, &limits, &mut budget, counters, rule.location)
            .unwrap()
            .is_some()
        {
            count += 1;
        }
        assert_eq!(
            count, 6,
            "the original comparisons remain executable and selective"
        );
    });
}

#[test]
fn leaf_permission_retains_started_cursor_and_comparison_state_gates() {
    testing::with_completed_source(SOURCE, |program, support, computation, counters| {
        let rule = program.rules_without_facts().last().unwrap();
        let limits = FormulaLimits::default();
        let mut budget = testing::budget();
        let mut cursor = Join::rule(
            Ir::Source(rule),
            support,
            computation,
            &limits,
            &mut budget,
            counters,
        )
        .unwrap();
        cursor.comparisons = crate::formula_support::Comparisons::Verified;
        assert!(
            CursorRows::prepare_producer(rule, &mut cursor, computation, &limits, counters)
                .unwrap()
                .is_none()
        );
        assert!(matches!(
            cursor.comparisons,
            crate::formula_support::Comparisons::Verified
        ));
        cursor.comparisons = crate::formula_support::Comparisons::Deferred;
        assert!(
            cursor
                .next(computation, &limits, &mut budget, counters, rule.location)
                .unwrap()
                .is_some()
        );
        let positions = cursor.positions.clone();
        assert!(
            CursorRows::prepare_producer(rule, &mut cursor, computation, &limits, counters)
                .unwrap()
                .is_none()
        );
        assert_eq!(cursor.positions, positions);
        let mut remaining = 0;
        while cursor
            .next(computation, &limits, &mut budget, counters, rule.location)
            .unwrap()
            .is_some()
        {
            remaining += 1;
        }
        assert_eq!(remaining, 5);
    });
}

#[test]
fn producer_descriptor_refusal_retains_the_original_cursor_and_typed_stop() {
    testing::with_completed_source(SOURCE, |program, support, computation, counters| {
        let rule = program.rules_without_facts().last().unwrap();
        let limits = FormulaLimits::default();
        let mut budget = testing::budget();
        let mut cursor = Join::rule(
            Ir::Source(rule),
            support,
            computation,
            &limits,
            &mut budget,
            counters,
        )
        .unwrap();
        let positions = cursor.positions.clone();
        let bounded = FormulaLimits {
            max_work: counters.accounting.work,
            ..limits
        };
        assert!(matches!(
            CursorRows::prepare_producer(rule, &mut cursor, computation, &bounded, counters),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::Work,
                ..
            })
        ));
        assert_eq!(cursor.positions, positions);
        let bounded = FormulaLimits {
            max_support_bytes: 0,
            ..limits
        };
        assert!(matches!(
            CursorRows::prepare_producer(rule, &mut cursor, computation, &bounded, counters),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::SupportBytes,
                ..
            })
        ));
        assert_eq!(cursor.positions, positions);
        let mut count = 0;
        while cursor
            .next(computation, &limits, &mut budget, counters, rule.location)
            .unwrap()
            .is_some()
        {
            count += 1;
        }
        assert_eq!(count, 6);
    });
}

#[test]
fn shared_leaf_shape_reads_have_exact_performed_prefix_charges() {
    testing::with_completed_source(SOURCE, |program, _, _, _| {
        let rule = program.rules_without_facts().last().unwrap();
        let comparison = rule
            .body
            .iter()
            .find_map(|literal| match literal {
                crate::formula_ir::LiteralIr::Compare(comparison) => Some(comparison),
                _ => None,
            })
            .unwrap();
        let (expressions, _) = comparison.operands();
        for allowance in 0..=4 {
            let limits = FormulaLimits {
                max_work: allowance,
                ..FormulaLimits::default()
            };
            let mut counters = Counters::default();
            let result =
                crate::formula_support::evaluation::comparison_leaves_with(expressions, || {
                    counters.work(&limits, rule.location)
                });
            assert_eq!(counters.accounting.work, allowance);
            if allowance == 4 {
                assert!(result.unwrap().is_some());
            } else {
                assert!(matches!(
                    result,
                    Err(FormulaFailure::Limit {
                        resource: FormulaResource::Work,
                        ..
                    })
                ));
            }
        }
        let empty = crate::formula_ir::Expression::new(Vec::new());
        let mut reads = 0;
        assert!(
            crate::formula_support::evaluation::comparison_leaves_with(
                [&empty, expressions[1]],
                || {
                    reads += 1;
                    Ok::<(), FormulaFailure>(())
                }
            )
            .unwrap()
            .is_none()
        );
        assert_eq!(
            reads, 1,
            "declined left arity does not inspect the right operand"
        );
    });
}
