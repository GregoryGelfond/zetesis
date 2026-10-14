//! Certificate semantics and transactional publication, without a solver route.
use super::super::{
    Analysis, Binding, Guard, Head, Limits, Relation, Rule, RuleId, Status, Stop, Widening,
    tests::{Key, Reader, assert_work_stop, leave_work, test_budget},
};
use super::*;

type Engine<'a> = Input<'a, Reader, u8>;
type Failure = Error<&'static str>;

fn number(input: &mut Engine<'_>, number: i32) -> Result<ExpressionId, Failure> {
    let key = input.constant(Key::Number(number))?;
    input.expression(Expression::Constant(key))
}

fn analyze_ranges(
    limits: Limits,
    ranges: &[(i32, i32)],
    requests: impl FnOnce(&mut Engine<'_>, &[ExpressionId]) -> Result<Vec<ExpressionId>, Failure>,
) -> (Analysis<Key, u8>, RuleId) {
    let mut id = None;
    let result = super::super::analyze(&Reader, limits, |input| {
        input.context(27);
        let mut rule = Rule {
            slots: ranges.len(),
            ..Rule::default()
        };
        let mut slots = Vec::new();
        for (slot, &(low, high)) in ranges.iter().enumerate() {
            let predicate = input.predicate(1)?;
            let argument = input.argument(predicate, 0)?;
            input.seed_interval(argument, low, high)?;
            rule.bindings.push(Binding::Positive { slot, argument });
            slots.push(input.expression(Expression::Slot(slot))?);
        }
        rule.integer_queries = requests(input, &slots)?;
        id = Some(input.rule(rule)?);
        Ok(())
    })
    .unwrap();
    (result, id.expect("the tested ceiling admits lowering"))
}

#[test]
fn request_order_includes_declines_and_duplicates() {
    let (result, id) = analyze_ranges(Limits::default(), &[(2, 7)], |input, slots| {
        let two = number(input, 2)?;
        let sum = input.expression(Expression::Binary(BinaryOp::Add, slots[0], two))?;
        let opaque = input.expression(Expression::Unknown {
            integer: true,
            reason: Widening::HeadTerm,
        })?;
        Ok(vec![sum, opaque, sum])
    });
    let rule = result.rule(id).unwrap();
    assert_eq!(rule.integer_query_count(), 3);
    let first = rule.integer(0).unwrap();
    assert_eq!(first.range().bounds(), (4, 9));
    assert!(rule.integer(1).is_none());
    assert_eq!(rule.integer(2).unwrap().expression(), first.expression());
    assert!(rule.integer(3).is_none());
}

#[test]
fn remainder_uses_complete_divisor_intervals() {
    let (result, id) = analyze_ranges(Limits::default(), &[(2, 20), (2, 20)], |input, slots| {
        Ok(vec![input.expression(Expression::Binary(
            BinaryOp::Mod,
            slots[0],
            slots[1],
        ))?])
    });
    assert_eq!(
        result
            .rule(id)
            .unwrap()
            .integer(0)
            .unwrap()
            .range()
            .bounds(),
        (0, 19)
    );
}

#[test]
fn signed_remainder_certificates_cover_checked_results() {
    for left in [(-9, -2), (-4, 5), (0, 0), (2, 9)] {
        for right in [(-5, -2), (-1, -1), (1, 1), (2, 5), (i32::MIN, i32::MIN)] {
            let (result, id) = analyze_ranges(Limits::default(), &[left, right], |input, slots| {
                Ok(vec![input.expression(Expression::Binary(
                    BinaryOp::Mod,
                    slots[0],
                    slots[1],
                ))?])
            });
            let (low, high) = result
                .rule(id)
                .unwrap()
                .integer(0)
                .unwrap()
                .range()
                .bounds();
            for dividend in left.0..=left.1 {
                for divisor in right.0..=right.1 {
                    let value = dividend.checked_rem(divisor).unwrap();
                    assert!(
                        (low..=high).contains(&value),
                        "{dividend} % {divisor} outside [{low},{high}]"
                    );
                }
            }
        }
    }
}

#[test]
fn remainder_faults_decline_even_when_the_math_is_zero() {
    for ranges in [[(1, 5), (-1, 1)], [(i32::MIN, i32::MIN), (-1, -1)]] {
        let (result, id) = analyze_ranges(Limits::default(), &ranges, |input, slots| {
            Ok(vec![input.expression(Expression::Binary(
                BinaryOp::Mod,
                slots[0],
                slots[1],
            ))?])
        });
        assert!(result.rule(id).unwrap().integer(0).is_none());
    }
}

#[test]
fn produced_only_zero_exclusion_cannot_certify_remainder() {
    let mut id = None;
    let result = super::super::analyze(&Reader, Limits::default(), |input| {
        let predicate = input.predicate(1)?;
        let argument = input.argument(predicate, 0)?;
        input.seed_interval(argument, -1, 1)?;
        let divisor = input.expression(Expression::Slot(0))?;
        let zero = number(input, 0)?;
        let one = number(input, 1)?;
        let remainder = input.expression(Expression::Binary(BinaryOp::Mod, one, divisor))?;
        id = Some(input.rule(Rule {
            slots: 1,
            bindings: vec![Binding::Positive { slot: 0, argument }],
            guards: vec![Guard::Compare {
                left: divisor,
                relation: Relation::Gt,
                right: zero,
            }],
            integer_queries: vec![remainder],
            ..Rule::default()
        })?);
        Ok(())
    })
    .unwrap();
    let rule = result.rule(id.unwrap()).unwrap();
    assert_eq!(
        rule.produced()[0].interval,
        Interval::Bounded { low: 1, high: 1 }
    );
    assert_eq!(
        rule.domains()[0].interval,
        Interval::Bounded { low: -1, high: 1 }
    );
    assert!(rule.integer(0).is_none());
}

#[test]
fn mixed_empty_and_unbounded_slots_decline() {
    for kind in 0..4 {
        let mut id = None;
        let result = super::super::analyze(&Reader, Limits::default(), |input| {
            let mut rule = Rule {
                slots: 1,
                ..Rule::default()
            };
            if kind != 2 {
                let predicate = input.predicate(1)?;
                let argument = input.argument(predicate, 0)?;
                if kind == 0 {
                    for key in [Key::Number(1), Key::Symbol("z")] {
                        let value = input.constant(key)?;
                        input.seed_value(argument, value)?;
                    }
                }
                if kind == 3 {
                    input.seed_interval(argument, 1, 2)?;
                    let value = input.expression(Expression::Unknown {
                        integer: false,
                        reason: Widening::HeadForm,
                    })?;
                    input.rule(Rule {
                        heads: vec![Head { argument, value }],
                        ..Rule::default()
                    })?;
                }
                rule.bindings.push(Binding::Positive { slot: 0, argument });
            }
            let slot = input.expression(Expression::Slot(0))?;
            let zero = number(input, 0)?;
            rule.guards.push(Guard::Compare {
                left: slot,
                relation: Relation::Gt,
                right: zero,
            });
            rule.integer_queries.push(slot);
            id = Some(input.rule(rule)?);
            Ok(())
        })
        .unwrap();
        assert!(
            result.rule(id.unwrap()).unwrap().integer(0).is_none(),
            "operand case {kind}"
        );
    }
}

#[test]
fn subtraction_cannot_hide_an_undefined_child() {
    for opaque in [false, true] {
        let (result, id) = analyze_ranges(Limits::default(), &[], |input, _| {
            let child = if opaque {
                input.expression(Expression::Unknown {
                    integer: true,
                    reason: Widening::HeadTerm,
                })?
            } else {
                let one = number(input, 1)?;
                let zero = number(input, 0)?;
                input.expression(Expression::Binary(BinaryOp::Div, one, zero))?
            };
            Ok(vec![input.expression(Expression::Binary(
                BinaryOp::Sub,
                child,
                child,
            ))?])
        });
        assert!(result.rule(id).unwrap().integer(0).is_none());
    }
}

#[test]
fn checked_intermediates_precede_a_zero_factor() {
    let (result, id) = analyze_ranges(
        Limits::default(),
        &[(i32::MAX, i32::MAX)],
        |input, slots| {
            let one = number(input, 1)?;
            let zero = number(input, 0)?;
            let overflow = input.expression(Expression::Binary(BinaryOp::Add, slots[0], one))?;
            Ok(vec![input.expression(Expression::Binary(
                BinaryOp::Mul,
                zero,
                overflow,
            ))?])
        },
    );
    assert!(result.rule(id).unwrap().integer(0).is_none());
}

#[test]
fn unsupported_operations_decline_with_bounded_operands() {
    for kind in 0..8 {
        let (result, id) = analyze_ranges(Limits::default(), &[(2, 2)], |input, slots| {
            let value = slots[0];
            let node = match kind {
                0 => Expression::Absolute(value),
                1 => Expression::Unary(UnaryOp::BitwiseNot, value),
                2 => Expression::Binary(BinaryOp::Mul, value, value),
                3 => Expression::Binary(BinaryOp::Div, value, value),
                4 => Expression::Binary(BinaryOp::Pow, value, value),
                5 => Expression::Binary(BinaryOp::BitAnd, value, value),
                6 => Expression::Binary(BinaryOp::BitOr, value, value),
                _ => Expression::Binary(BinaryOp::BitXor, value, value),
            };
            Ok(vec![input.expression(node)?])
        });
        assert!(
            result.rule(id).unwrap().integer(0).is_none(),
            "operator case {kind}"
        );
    }
}

#[test]
fn literal_operations_preserve_signed_bounds() {
    let (result, id) = analyze_ranges(Limits::default(), &[(-7, 5)], |input, slots| {
        let factor = number(input, -2)?;
        Ok(vec![
            input.expression(Expression::Unary(UnaryOp::Negate, slots[0]))?,
            input.expression(Expression::Binary(BinaryOp::Mul, slots[0], factor))?,
            input.expression(Expression::Binary(BinaryOp::Div, slots[0], factor))?,
        ])
    });
    let rule = result.rule(id).unwrap();
    for (query, expected) in [(-5, 7), (-10, 14), (-2, 3)].into_iter().enumerate() {
        assert_eq!(rule.integer(query).unwrap().range().bounds(), expected);
    }
}

fn constant_queries(limits: Limits) -> (Analysis<Key, u8>, RuleId) {
    analyze_ranges(limits, &[], |input, _| {
        let first = number(input, 2)?;
        let second = number(input, 3)?;
        Ok(vec![first, second])
    })
}

#[test]
fn a_final_certificate_work_stop_clears_earlier_results() {
    let (complete, _) = constant_queries(Limits::default());
    let exact = complete.statistics().work;
    let (accepted, id) = constant_queries(Limits {
        max_work: exact,
        ..Limits::default()
    });
    assert_eq!(accepted.status(), Status::FixedPoint);
    assert_eq!(accepted.rule(id).unwrap().integer_query_count(), 2);
    let (stopped, id) = constant_queries(Limits {
        max_work: exact - 1,
        ..Limits::default()
    });
    assert!(matches!(
        stopped.status(),
        Status::Stopped(Stop {
            resource: crate::Resource::Work,
            ..
        })
    ));
    assert_eq!(stopped.statistics().work, exact - 1);
    assert_eq!(stopped.context(), Some(27));
    assert!(stopped.rule(id).is_none());
    assert_eq!(stopped.arguments().count(), 0);
}

#[test]
fn retained_certificate_cells_have_an_inclusive_link_limit() {
    let (complete, _) = constant_queries(Limits::default());
    let exact = complete.statistics().links;
    let (accepted, id) = constant_queries(Limits {
        max_links: exact,
        ..Limits::default()
    });
    assert_eq!(accepted.rule(id).unwrap().integer_query_count(), 2);
    let (stopped, id) = constant_queries(Limits {
        max_links: exact - 1,
        ..Limits::default()
    });
    assert!(matches!(
        stopped.status(),
        Status::Stopped(Stop {
            resource: crate::Resource::Links,
            ..
        })
    ));
    assert!(stopped.rule(id).is_none());
    assert_eq!(stopped.context(), Some(27));
}

#[test]
fn an_invalid_query_preserves_the_rule_context() {
    let failure = super::super::analyze(&Reader, Limits::default(), |input| {
        input.context(8_u8);
        let query = input.expression(Expression::Slot(1))?;
        input.rule(Rule {
            slots: 1,
            integer_queries: vec![query],
            ..Rule::default()
        })?;
        Ok(())
    })
    .unwrap_err();
    assert!(matches!(failure.error, Error::InvalidInput));
    assert_eq!(failure.context, Some(8));
}

#[test]
fn malformed_nodes_are_errors_not_declines() {
    let budget = test_budget();
    let input = Input::<_, ()>::new(&Reader, &budget);
    assert!(matches!(
        expression(&input, ExpressionId(0), &[]),
        Err(Error::InvalidInput)
    ));
}

#[test]
fn each_postorder_frame_is_charged() {
    let budget = test_budget();
    let mut input = Input::<_, ()>::new(&Reader, &budget);
    let mut root = input.expression(Expression::Slot(0)).unwrap();
    for _ in 0..3 {
        root = input
            .expression(Expression::Unary(UnaryOp::Negate, root))
            .unwrap();
    }
    let bounds = [Bound {
        integrality: Integrality::Integer,
        interval: Interval::Bounded { low: 1, high: 2 },
    }];
    let before = budget.statistics().work;
    assert_eq!(
        expression(&input, root, &bounds).unwrap().unwrap().bounds(),
        (-2, -1)
    );
    assert_eq!(budget.statistics().work - before, 7);
    leave_work(&budget, 6);
    assert_work_stop(&expression(&input, root, &bounds), &budget);
}

#[test]
fn empty_requests_add_no_work_or_links() {
    let budget = test_budget();
    let input = Input::<_, ()>::new(&Reader, &budget);
    let before = budget.statistics();
    assert!(queries(&input, &[], &[]).unwrap().is_empty());
    assert_eq!(budget.statistics(), before);
}

#[test]
fn all_positive_occurrences_constrain_complete_bindings() {
    let mut id = None;
    let result = super::super::analyze(&Reader, Limits::default(), |input| {
        let mut rule = Rule {
            slots: 1,
            ..Rule::default()
        };
        for (low, high) in [(-1, 1), (1, 2)] {
            let predicate = input.predicate(1)?;
            let argument = input.argument(predicate, 0)?;
            input.seed_interval(argument, low, high)?;
            rule.bindings.push(Binding::Positive { slot: 0, argument });
        }
        let slot = input.expression(Expression::Slot(0))?;
        let one = number(input, 1)?;
        rule.integer_queries
            .push(input.expression(Expression::Binary(BinaryOp::Mod, one, slot))?);
        id = Some(input.rule(rule)?);
        Ok(())
    })
    .unwrap();
    let rule = result.rule(id.unwrap()).unwrap();
    assert_eq!(
        rule.domains()[0].interval,
        Interval::Bounded { low: 1, high: 1 }
    );
    assert_eq!(rule.integer(0).unwrap().range().bounds(), (0, 0));
}

#[test]
fn a_shared_expression_uses_each_original_rule_scope() {
    let mut ids = Vec::new();
    let result: Analysis<Key, u8> = super::super::analyze(&Reader, Limits::default(), |input| {
        let slot = input.expression(Expression::Slot(0))?;
        for (low, high) in [(1, 2), (8, 9)] {
            let predicate = input.predicate(1)?;
            let argument = input.argument(predicate, 0)?;
            input.seed_interval(argument, low, high)?;
            ids.push(input.rule(Rule {
                slots: 1,
                bindings: vec![Binding::Positive { slot: 0, argument }],
                integer_queries: vec![slot],
                ..Rule::default()
            })?);
        }
        Ok(())
    })
    .unwrap();
    let first = result.rule(ids[0]).unwrap().integer(0).unwrap();
    let second = result.rule(ids[1]).unwrap().integer(0).unwrap();
    assert_eq!(first.expression(), second.expression());
    assert_eq!(first.range().bounds(), (1, 2));
    assert_eq!(second.range().bounds(), (8, 9));
}
