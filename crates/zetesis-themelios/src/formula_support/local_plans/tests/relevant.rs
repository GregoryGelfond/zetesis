use super::*;

fn same_plan(left: &order::Plan<'_>, right: &order::Plan<'_>, literals: usize) {
    assert_eq!(left.pivot, right.pivot);
    assert_eq!(
        left.patterns.iter().map(|p| p.source).collect::<Vec<_>>(),
        right.patterns.iter().map(|p| p.source).collect::<Vec<_>>()
    );
    for literal in 0..literals {
        assert_eq!(
            left.decisions.decides(literal),
            right.decisions.decides(literal)
        );
    }
    for depth in 0..=left.patterns.len() {
        assert_eq!(
            left.decisions.decided_at(depth).collect::<Vec<_>>(),
            right.decisions.decided_at(depth).collect::<Vec<_>>()
        );
        assert_eq!(
            left.decisions.certifies(depth),
            right.decisions.certifies(depth)
        );
    }
}

fn row(
    binding: &Binding<'_>,
    computation: &Computation<'_, '_>,
) -> Vec<Option<zetesis_core::Value>> {
    (0..binding.len())
        .map(|slot| {
            binding.slots().key(slot).unwrap().map(|key| {
                computation
                    .read()
                    .term(&key)
                    .unwrap()
                    .to_value(zetesis_core::ValueLimits::default())
                    .unwrap()
            })
        })
        .collect()
}

#[test]
fn ignored_outer_slot_reuses_plan_with_the_current_binding() {
    testing::with_completed_source(
        "d(1). h(X,Y) :- d(X), Y=1.",
        |prepared, support, computation, counters| {
            let rule = prepared
                .rules_without_facts()
                .iter()
                .find(|rule| rule.variables == 2)
                .unwrap();
            let literals = &rule.body[..1];
            let location = rule.location;
            let limits = FormulaLimits::default();
            let mut budget = testing::budget();
            let first = testing::binding(
                &[Some(zetesis_core::Value::Number(1)), None],
                computation,
                counters,
                location,
            );
            let current = testing::binding(
                &[
                    Some(zetesis_core::Value::Number(1)),
                    Some(zetesis_core::Value::Number(7)),
                ],
                computation,
                counters,
                location,
            );
            let table = Table::new(support.workspace());
            let initial = table
                .plan(
                    literals,
                    &first,
                    rule.variables,
                    support,
                    &mut budget,
                    Context::new(computation, &limits, counters, location),
                )
                .unwrap()
                .unwrap();
            assert_eq!(
                table.state.borrow().entries[0].bound,
                vec![Some(true), None]
            );
            let selected = table
                .plan(
                    literals,
                    &current,
                    rule.variables,
                    support,
                    &mut budget,
                    Context::new(computation, &limits, counters, location),
                )
                .unwrap()
                .unwrap();
            assert!(Rc::ptr_eq(&initial, &selected));
            let mut shared = Join::with_plan(
                literals,
                &current,
                rule.variables,
                support,
                Plan::Shared(selected),
                None,
                Context::new(computation, &limits, counters, location),
            )
            .unwrap();
            let mut fresh = Join::new(
                literals,
                &current,
                rule.variables,
                support,
                &mut budget,
                Context::new(computation, &limits, counters, location),
            )
            .unwrap();
            same_plan(&shared.plan, &fresh.plan, literals.len());
            let mut rows = 0;
            loop {
                let left = shared
                    .next(computation, &limits, &mut budget, counters, location)
                    .unwrap()
                    .map(|b| row(&b, computation));
                let right = fresh
                    .next(computation, &limits, &mut budget, counters, location)
                    .unwrap()
                    .map(|b| row(&b, computation));
                assert_eq!(left, right);
                let Some(left) = left else { break };
                assert_eq!(
                    left,
                    vec![
                        Some(zetesis_core::Value::Number(1)),
                        Some(zetesis_core::Value::Number(7))
                    ]
                );
                rows += 1;
            }
            assert_eq!(rows, 1);
        },
    );
}

#[test]
fn original_shapes_preserve_fresh_order_and_every_decision() {
    for source in [
        "d(1). d(2). h(X,Y) :- d(X), d(Y), X<Y.",
        "d(f(1)). d(f(2)). h(X) :- d(f(X)).",
        "d(1). {p(1)}. h(X) :- d(X), not p(X), X=1..2.",
        "d(1). {p(1)}. h(X,N) :- d(X), N=#count{Y:d(Y),p(Y)}.",
        "d(1). {p(1)}. h(X) :- d(X), p(Y):d(Y).",
    ] {
        testing::with_completed_source(source, |prepared, support, computation, counters| {
            let rule = prepared.rules_without_facts().last().unwrap();
            let location = rule.location;
            let limits = FormulaLimits::default();
            let variables = rule.variables + 1;
            let mut values = vec![Some(zetesis_core::Value::Number(1)); variables];
            values[rule.variables] = None;
            let first = testing::binding(&values, computation, counters, location);
            values[rule.variables] = Some(zetesis_core::Value::Number(7));
            let current = testing::binding(&values, computation, counters, location);
            let mut budget = testing::budget();
            let table = Table::new(support.workspace());
            let initial = table
                .plan(
                    &rule.body,
                    &first,
                    variables,
                    support,
                    &mut budget,
                    Context::new(computation, &limits, counters, location),
                )
                .unwrap()
                .unwrap();
            assert_eq!(table.state.borrow().entries[0].bound[rule.variables], None);
            let reused = table
                .plan(
                    &rule.body,
                    &current,
                    variables,
                    support,
                    &mut budget,
                    Context::new(computation, &limits, counters, location),
                )
                .unwrap()
                .unwrap();
            assert!(Rc::ptr_eq(&initial, &reused), "{source}");
            let fresh = order::Plan::new(
                &rule.body,
                &current,
                variables,
                order::SourceRows {
                    relations: support,
                    pivot: None,
                },
                &mut budget,
                location,
                None,
            )
            .unwrap();
            same_plan(&reused, &fresh, rule.body.len());
        });
    }
}

#[test]
fn comparison_readiness_remains_required() {
    testing::with_completed_source(
        "d(1). h(X,Y) :- d(X), X<2, Y=1.",
        |prepared, support, computation, counters| {
            let rule = prepared
                .rules_without_facts()
                .iter()
                .find(|rule| rule.variables == 2)
                .unwrap();
            let literal = rule
                .body
                .iter()
                .find(|l| matches!(l, crate::formula_ir::LiteralIr::Compare(_)))
                .unwrap();
            let literals = std::slice::from_ref(literal);
            let location = rule.location;
            let limits = FormulaLimits::default();
            let mut budget = testing::budget();
            let first = testing::binding(&[None, None], computation, counters, location);
            let current = testing::binding(
                &[Some(zetesis_core::Value::Number(1)), None],
                computation,
                counters,
                location,
            );
            let table = Table::new(support.workspace());
            let plan = table
                .plan(
                    literals,
                    &first,
                    rule.variables,
                    support,
                    &mut budget,
                    Context::new(computation, &limits, counters, location),
                )
                .unwrap()
                .unwrap();
            assert_eq!(
                table.state.borrow().entries[0].bound,
                vec![Some(false), None]
            );
            assert!(!plan.decisions.decides(0));
            assert!(
                table
                    .plan(
                        literals,
                        &current,
                        rule.variables,
                        support,
                        &mut budget,
                        Context::new(computation, &limits, counters, location)
                    )
                    .unwrap()
                    .is_none()
            );
            let fresh = order::Plan::new(
                literals,
                &current,
                rule.variables,
                order::SourceRows {
                    relations: support,
                    pivot: None,
                },
                &mut budget,
                location,
                None,
            )
            .unwrap();
            assert!(fresh.decisions.decides(0));
            assert_eq!(table.state.borrow().entries.len(), 1);
        },
    );
}

#[test]
fn footprint_has_exact_refusal_and_storage_conservation() {
    testing::with_completed_source(
        "d(1). h(X,Y) :- d(X), Y=1.",
        |prepared, support, computation, counters| {
            assert_eq!(size_of::<Option<bool>>(), size_of::<u8>());
            let rule = prepared
                .rules_without_facts()
                .iter()
                .find(|rule| rule.variables == 2)
                .unwrap();
            let literals = &rule.body[..1];
            let location = rule.location;
            let limits = FormulaLimits::default();
            let prefix = testing::binding(
                &[Some(zetesis_core::Value::Number(1)), None],
                computation,
                counters,
                location,
            );
            let mut attempt = |limit| {
                let before = support.live_bytes();
                let table = Table::new(support.workspace());
                let mut budget = Budget::new(
                    crate::ExpansionLimits {
                        max_term_work: limit,
                        ..Default::default()
                    },
                    usize::MAX,
                );
                let result = table.plan(
                    literals,
                    &prefix,
                    rule.variables,
                    support,
                    &mut budget,
                    Context::new(computation, &limits, counters, location),
                );
                let published = table.state.borrow().entries.len();
                let charged = budget.usage().term_work;
                if let Ok(Some(plan)) = &result {
                    let state = table.state.borrow();
                    let expected = usize::try_from(plan.retained_bytes()).unwrap()
                        + shared_header(location).unwrap()
                        + state.entries[0].bound.capacity() * size_of::<Option<bool>>()
                        + state.entries.capacity() * size_of::<Entry<'_>>();
                    assert_eq!(state.lease.bytes(), expected);
                    assert_eq!(support.live_bytes(), before + expected);
                }
                let outcome = result
                    .map(|plan| assert!(plan.is_some()))
                    .map_err(|error| format!("{error:?}"));
                drop(table);
                assert_eq!(support.live_bytes(), before);
                (outcome, charged, published)
            };
            let (result, full, published) =
                attempt(crate::ExpansionLimits::default().max_term_work);
            result.unwrap();
            assert_eq!(published, 1);
            for limit in 0..=full {
                let first = attempt(limit);
                let repeated = attempt(limit);
                assert_eq!(first, repeated, "fixed owner and allowance {limit}");
                assert!(first.1 <= limit);
                assert_eq!(first.2, usize::from(limit == full));
                assert_eq!(first.0.is_ok(), limit == full);
            }
        },
    );
}

#[test]
fn projected_hit_preserves_cancellation_and_owner() {
    testing::with_completed_source(
        "d(1). h(X,Y) :- d(X), Y=1.",
        |prepared, support, computation, counters| {
            let rule = prepared
                .rules_without_facts()
                .iter()
                .find(|rule| rule.variables == 2)
                .unwrap();
            let literals = &rule.body[..1];
            let location = rule.location;
            let limits = FormulaLimits::default();
            let prefix = testing::binding(
                &[Some(zetesis_core::Value::Number(1)), None],
                computation,
                counters,
                location,
            );
            let current = testing::binding(
                &[
                    Some(zetesis_core::Value::Number(1)),
                    Some(zetesis_core::Value::Number(7)),
                ],
                computation,
                counters,
                location,
            );
            let table = Table::new(support.workspace());
            let mut budget = testing::budget();
            let plan = table
                .plan(
                    literals,
                    &prefix,
                    rule.variables,
                    support,
                    &mut budget,
                    Context::new(computation, &limits, counters, location),
                )
                .unwrap()
                .unwrap();
            let retained = support.live_bytes();
            let cancellation = zetesis_algebra::Cancellation::default();
            cancellation.cancel();
            let mut stopped = testing::budget().with_cancellation(Some(cancellation));
            let result = table.plan(
                literals,
                &current,
                rule.variables,
                support,
                &mut stopped,
                Context::new(computation, &limits, counters, location),
            );
            assert!(matches!(
                result,
                Err(FormulaFailure::Expansion(
                    crate::ExpansionFailure::Interrupted { .. }
                ))
            ));
            assert_eq!(stopped.usage().term_work, 0);
            assert_eq!(table.state.borrow().entries.len(), 1);
            assert_eq!(support.live_bytes(), retained);
            let mut available = testing::budget();
            let resumed = table
                .plan(
                    literals,
                    &current,
                    rule.variables,
                    support,
                    &mut available,
                    Context::new(computation, &limits, counters, location),
                )
                .unwrap()
                .unwrap();
            assert!(Rc::ptr_eq(&plan, &resumed));
        },
    );
}

#[test]
fn structural_private_whole_value_and_nested_slots_are_both_required() {
    testing::with_completed_source(
        "d(f(1)). h(X) :- d(f(X)).",
        |prepared, support, computation, counters| {
            let rule = prepared.rules_without_facts().last().unwrap();
            let location = rule.location;
            let limits = FormulaLimits::default();
            let values = vec![Some(zetesis_core::Value::Number(1)); rule.variables];
            let prefix = testing::binding(&values, computation, counters, location);
            let mut budget = testing::budget();
            let table = Table::new(support.workspace());
            let plan = table
                .plan(
                    &rule.body,
                    &prefix,
                    rule.variables,
                    support,
                    &mut budget,
                    Context::new(computation, &limits, counters, location),
                )
                .unwrap()
                .unwrap();
            let pattern = plan
                .patterns
                .iter()
                .find_map(|occurrence| match occurrence.pattern {
                    crate::formula_support::PositivePattern::Structural(pattern) => Some(pattern),
                    crate::formula_support::PositivePattern::Flat(_) => None,
                })
                .expect("fixture must use a structural pattern");
            let whole: Vec<_> = pattern.atom().terms().variables().collect();
            let nested: Vec<_> = pattern
                .slots()
                .filter(|slot| !whole.contains(slot))
                .collect();
            assert_eq!(whole.len(), 1);
            assert_eq!(nested.len(), 1);
            for &slot in whole.iter().chain(&nested) {
                assert_eq!(table.state.borrow().entries[0].bound[slot], Some(true));
                let mut changed = values.clone();
                changed[slot] = None;
                let current = testing::binding(&changed, computation, counters, location);
                assert!(
                    table
                        .plan(
                            &rule.body,
                            &current,
                            rule.variables,
                            support,
                            &mut budget,
                            Context::new(computation, &limits, counters, location)
                        )
                        .unwrap()
                        .is_none()
                );
                let fresh = order::Plan::new(
                    &rule.body,
                    &current,
                    rule.variables,
                    order::SourceRows {
                        relations: support,
                        pivot: None,
                    },
                    &mut budget,
                    location,
                    None,
                );
                if whole.contains(&slot) {
                    assert!(
                        matches!(fresh,Err(FormulaFailure::UnsafeVariable{variable,..}) if variable==slot)
                    );
                } else {
                    assert!(fresh.is_ok());
                }
            }
        },
    );
}
