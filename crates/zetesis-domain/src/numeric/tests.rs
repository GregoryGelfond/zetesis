//! Bounds, scope, conservative operators and complete-stop publication.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Key {
    Number(i32),
    Symbol(&'static str),
}
pub(super) struct Reader;
impl Values for Reader {
    type Key = Key;
    type Error = &'static str;
    fn classify(
        &self,
        key: &Key,
        _: &Limits,
        budget: &Budget,
    ) -> Result<Classification, Error<Self::Error>> {
        budget.work(1)?;
        Ok(Classification {
            number: match key {
                Key::Number(value) => Some(*value),
                Key::Symbol(_) => None,
            },
            retain: true,
        })
    }
    fn compare(
        &self,
        left: &Key,
        right: &Key,
        budget: &Budget,
    ) -> Result<Ordering, Error<Self::Error>> {
        budget.work(1)?;
        Ok(left.cmp(right))
    }
}
fn argument(input: &mut Input<'_, Reader, u8>) -> Result<ArgumentId, Error<&'static str>> {
    let predicate = input.predicate(1)?;
    input.argument(predicate, 0)
}
fn number(
    input: &mut Input<'_, Reader, u8>,
    number: i32,
) -> Result<ExpressionId, Error<&'static str>> {
    let value = input.constant(Key::Number(number))?;
    input.expression(Expression::Constant(value))
}
fn bounded(low: i32, high: i32) -> Bound {
    Bound {
        integrality: Integrality::Integer,
        interval: Interval::Bounded { low, high },
    }
}

#[test]
fn guards_do_not_refine_complete_binding_domains() {
    let mut ids = None;
    let result = analyze(&Reader, Limits::default(), |input| {
        input.context(7);
        let source = argument(input)?;
        let target = argument(input)?;
        input.seed_interval(source, 1, 10)?;
        let slot = input.expression(Expression::Slot(0))?;
        let three = number(input, 3)?;
        let rule = input.rule(Rule {
            slots: 1,
            bindings: vec![Binding::Positive {
                slot: 0,
                argument: source,
            }],
            guards: vec![Guard::Compare {
                left: slot,
                relation: Relation::Lt,
                right: three,
            }],
            heads: vec![Head {
                argument: target,
                value: slot,
            }],
            integer_queries: vec![],
        })?;
        ids = Some((target, rule));
        Ok(())
    })
    .unwrap();
    let (target, rule) = ids.unwrap();
    assert_eq!(result.status(), Status::FixedPoint);
    assert_eq!(result.argument(target), Some(bounded(1, 2)));
    assert_eq!(result.rule(rule).unwrap().domains(), &[bounded(1, 10)]);
    assert_eq!(result.rule(rule).unwrap().produced(), &[bounded(1, 2)]);
}

#[test]
fn comparisons_never_invent_integer_operands() {
    let mut target = None;
    let result = analyze(&Reader, Limits::default(), |input| {
        input.context(1);
        let source = argument(input)?;
        let output = argument(input)?;
        target = Some(output);
        for key in [Key::Number(0), Key::Symbol("z")] {
            let key = input.constant(key)?;
            input.seed_value(source, key)?;
        }
        let slot = input.expression(Expression::Slot(0))?;
        let two = number(input, 2)?;
        input.rule(Rule {
            slots: 1,
            bindings: vec![Binding::Positive {
                slot: 0,
                argument: source,
            }],
            guards: vec![Guard::Compare {
                left: slot,
                relation: Relation::Gt,
                right: two,
            }],
            heads: vec![Head {
                argument: output,
                value: slot,
            }],
            integer_queries: vec![],
        })?;
        Ok(())
    })
    .unwrap();
    let target = target.unwrap();
    assert_eq!(
        result.argument(target).unwrap().integrality,
        Integrality::Mixed
    );
    assert_eq!(
        result.values(target).unwrap().copied().collect::<Vec<_>>(),
        vec![Key::Number(0), Key::Symbol("z")]
    );
}

#[test]
fn recursive_guard_caps_survive_widening() {
    let mut target = None;
    let result = analyze(&Reader, Limits::default(), |input| {
        input.context(3);
        let output = argument(input)?;
        target = Some(output);
        let zero = input.constant(Key::Number(0))?;
        input.seed_value(output, zero)?;
        let slot = input.expression(Expression::Slot(0))?;
        let one = number(input, 1)?;
        let cap = number(input, 100)?;
        let next = input.expression(Expression::Binary(BinaryOp::Add, slot, one))?;
        input.rule(Rule {
            slots: 1,
            bindings: vec![Binding::Positive {
                slot: 0,
                argument: output,
            }],
            guards: vec![Guard::Compare {
                left: next,
                relation: Relation::Le,
                right: cap,
            }],
            heads: vec![Head {
                argument: output,
                value: next,
            }],
            integer_queries: vec![],
        })?;
        Ok(())
    })
    .unwrap();
    assert_eq!(result.argument(target.unwrap()), Some(bounded(0, 100)));
    assert!(
        result.statistics().rounds < 20,
        "thresholds avoid enumerating the full interval"
    );
}

#[test]
fn unproved_operations_keep_unbounded_results() {
    for kind in 0..9 {
        let mut target = None;
        let result = analyze(&Reader, Limits::default(), |input| {
            input.context(4);
            let source = argument(input)?;
            let output = argument(input)?;
            target = Some(output);
            input.seed_interval(source, 1, 2)?;
            let slot = input.expression(Expression::Slot(0))?;
            let expression = match kind {
                0 => Expression::Absolute(slot),
                1 => Expression::Unary(UnaryOp::BitwiseNot, slot),
                2 => Expression::Binary(BinaryOp::Mul, slot, slot),
                3 => Expression::Binary(BinaryOp::Div, slot, slot),
                4 => Expression::Binary(BinaryOp::Mod, slot, slot),
                5 => Expression::Binary(BinaryOp::Pow, slot, slot),
                6 => Expression::Binary(BinaryOp::BitAnd, slot, slot),
                7 => Expression::Binary(BinaryOp::BitOr, slot, slot),
                _ => Expression::Binary(BinaryOp::BitXor, slot, slot),
            };
            let value = input.expression(expression)?;
            input.rule(Rule {
                slots: 1,
                bindings: vec![Binding::Positive {
                    slot: 0,
                    argument: source,
                }],
                guards: vec![],
                heads: vec![Head {
                    argument: output,
                    value,
                }],
                integer_queries: vec![],
            })?;
            Ok(())
        })
        .unwrap();
        assert_eq!(
            result.argument(target.unwrap()),
            Some(Bound {
                integrality: Integrality::Integer,
                interval: Interval::Unbounded
            }),
            "operator case {kind}"
        );
    }
}

#[test]
fn membership_refines_only_produced_values() {
    let mut rule = None;
    let result = analyze(&Reader, Limits::default(), |input| {
        input.context(5);
        let source = argument(input)?;
        input.seed_interval(source, 0, 10)?;
        let low = number(input, 1)?;
        let high = number(input, 3)?;
        rule = Some(input.rule(Rule {
            slots: 1,
            bindings: vec![Binding::Positive {
                slot: 0,
                argument: source,
            }],
            guards: vec![Guard::Membership { slot: 0, low, high }],
            heads: vec![],
            integer_queries: vec![],
        })?);
        Ok(())
    })
    .unwrap();
    let local = result.rule(rule.unwrap()).unwrap();
    assert_eq!(local.domains(), &[bounded(0, 10)]);
    assert_eq!(local.produced(), &[bounded(1, 3)]);
}

fn seeded(limits: Limits) -> Analysis<Key, u8> {
    analyze(&Reader, limits, |input| {
        input.context(9);
        let output = argument(input)?;
        let value = input.constant(Key::Number(7))?;
        input.seed_value(output, value)?;
        Ok(())
    })
    .unwrap()
}
#[test]
fn work_stop_clears_every_completed_bound() {
    let complete = seeded(Limits::default());
    let exact = complete.statistics().work;
    assert_eq!(
        seeded(Limits {
            max_work: exact,
            ..Limits::default()
        })
        .status(),
        Status::FixedPoint
    );
    let stopped = seeded(Limits {
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
    assert_eq!(stopped.arguments().count(), 0);
    assert_eq!(stopped.context(), Some(9));
}

#[test]
fn owner_failures_never_become_unknown_keys() {
    struct Broken;
    impl Values for Broken {
        type Key = ();
        type Error = &'static str;
        fn classify(
            &self,
            (): &(),
            _: &Limits,
            budget: &Budget,
        ) -> Result<Classification, Error<Self::Error>> {
            budget.work(2)?;
            Err(Error::Read("foreign owner"))
        }
        fn compare(&self, (): &(), (): &(), _: &Budget) -> Result<Ordering, Error<Self::Error>> {
            unreachable!()
        }
    }
    let failure = analyze(&Broken, Limits::default(), |input| {
        input.context(11_u8);
        input.constant(())?;
        Ok(())
    })
    .unwrap_err();
    assert!(matches!(failure.error, Error::Read("foreign owner")));
    assert_eq!(failure.statistics.work, 3);
    assert_eq!(failure.context, Some(11));
}

#[test]
fn opaque_summaries_never_establish_subtraction_correlation() {
    for reuse in [false, true] {
        for wrapped in [false, true] {
            let mut output = None;
            let result = analyze(&Reader, Limits::default(), |input| {
                let target = argument(input)?;
                output = Some(target);
                let first = input.expression(Expression::Unknown {
                    integer: true,
                    reason: Widening::HeadTerm,
                })?;
                let second = if reuse {
                    first
                } else {
                    let second = input.expression(Expression::Unknown {
                        integer: true,
                        reason: Widening::HeadTerm,
                    })?;
                    assert_ne!(
                        first, second,
                        "equal summaries do not identify equal expressions"
                    );
                    second
                };
                let one = number(input, 1)?;
                let left = if wrapped {
                    input.expression(Expression::Binary(BinaryOp::Add, first, one))?
                } else {
                    first
                };
                let right = if wrapped {
                    input.expression(Expression::Binary(BinaryOp::Add, second, one))?
                } else {
                    second
                };
                let value = input.expression(Expression::Binary(BinaryOp::Sub, left, right))?;
                input.rule(Rule {
                    heads: vec![Head {
                        argument: target,
                        value,
                    }],
                    ..Rule::default()
                })?;
                Ok(())
            })
            .unwrap();
            assert_eq!(
                result.argument(output.unwrap()),
                Some(Bound {
                    integrality: Integrality::Integer,
                    interval: Interval::Unbounded
                }),
                "reuse={reuse}, wrapped={wrapped}"
            );
        }
    }
}

#[test]
fn opaque_guard_results_are_not_reused_in_heads() {
    for wrapped in [false, true] {
        let mut output = None;
        let result = analyze(&Reader, Limits::default(), |input| {
            let target = argument(input)?;
            output = Some(target);
            let unknown = input.expression(Expression::Unknown {
                integer: true,
                reason: Widening::HeadTerm,
            })?;
            let one = number(input, 1)?;
            let cap = number(input, 3)?;
            let value = if wrapped {
                input.expression(Expression::Binary(BinaryOp::Add, unknown, one))?
            } else {
                unknown
            };
            input.rule(Rule {
                guards: vec![Guard::Compare {
                    left: value,
                    relation: Relation::Le,
                    right: cap,
                }],
                heads: vec![Head {
                    argument: target,
                    value,
                }],
                ..Rule::default()
            })?;
            Ok(())
        })
        .unwrap();
        assert_eq!(
            result.argument(output.unwrap()),
            Some(Bound {
                integrality: Integrality::Integer,
                interval: Interval::Unbounded
            })
        );
    }
}

#[test]
fn imprecise_producer_is_not_hidden_by_precise_facts() {
    let mut output = None;
    let result = analyze(&Reader, Limits::default(), |input| {
        let target = argument(input)?;
        output = Some(target);
        input.context(1);
        let value = input.constant(Key::Number(7))?;
        input.seed_value(target, value)?;
        input.context(2);
        let value = input.expression(Expression::Unknown {
            integer: false,
            reason: Widening::HeadTerm,
        })?;
        input.rule(Rule {
            heads: vec![Head {
                argument: target,
                value,
            }],
            ..Rule::default()
        })?;
        Ok(())
    })
    .unwrap();
    let output = output.unwrap();
    assert_eq!(result.argument(output), Some(Bound::TOP));
    assert!(result.values(output).is_none());
    assert_eq!(result.producers(output), Some(&[1, 2][..]));
}

pub(super) fn test_budget() -> Budget {
    Budget {
        limits: Limits::default(),
        statistics: Cell::new(Statistics::default()),
    }
}
pub(super) fn leave_work(budget: &Budget, remaining: u64) {
    budget
        .work(budget.limits.max_work - budget.statistics().work - remaining)
        .unwrap();
}
pub(super) fn assert_work_stop<T, E>(result: &Result<T, Error<E>>, budget: &Budget) {
    assert!(matches!(
        result,
        Err(Error::Limit(Stop {
            resource: crate::Resource::Work,
            ..
        }))
    ));
    assert_eq!(budget.statistics().work, budget.limits.max_work);
}
