//! Prepared scopes preserve exact traversal, emitted graphs and failure boundaries.

use super::testing as route;
use super::*;
use crate::formula_support::{Counters, testing};
use crate::{ExpansionFailure, ExpansionLimits, ExpansionResource, FormulaLimits, FormulaResource};
use zetesis_core::{Value, ValueNodeRef};

const SOURCE: &str = "p(1,2). p(1,3). p(2,4). h(X,Y) :- p(X,Y), X<Y.";

fn rows(
    mut join: Join<'_, '_>,
    computation: &mut Computation<'_, '_>,
    counters: &mut Counters,
    location: ProgramSite,
) -> Vec<Vec<Option<i32>>> {
    let mut result = Vec::new();
    let mut budget = testing::budget();
    while let Some(row) = join
        .next(
            computation,
            &FormulaLimits::default(),
            &mut budget,
            counters,
            location,
        )
        .unwrap()
    {
        result.push(
            (0..row.len())
                .map(|slot| {
                    if !row.is_bound(slot, location).unwrap() {
                        return None;
                    }
                    let ValueNodeRef::Number(value) = row
                        .read(slot, computation.read(), location)
                        .unwrap()
                        .descriptor()
                    else {
                        panic!("numeric fixture")
                    };
                    Some(value)
                })
                .collect(),
        );
    }
    result
}

#[test]
fn prepared_cursors_preserve_complete_bindings() {
    testing::with_completed_source(SOURCE, |program, support, computation, counters| {
        let rule = &program.rules_without_facts()[0];
        assert_eq!(rule.variables, 2);
        let location = rule.location;
        let limits = FormulaLimits::default();
        let fixed = testing::binding(
            &[Some(Value::Number(1)), None, None],
            computation,
            counters,
            location,
        );
        let used = [0, 1];
        let scope = ComponentScope {
            literals: &rule.body,
            used: &used,
        };
        let ((), counts) = route::scoped(true, || {
            let mut plans = ComponentPlans::new(
                1,
                3,
                &fixed,
                support,
                Context::new(computation, &limits, counters, location),
            )
            .unwrap();
            for (value, expected) in [
                (
                    1,
                    vec![vec![Some(1), Some(2), None], vec![Some(1), Some(3), None]],
                ),
                (2, vec![vec![Some(2), Some(4), None]]),
                (9, vec![]),
                (
                    1,
                    vec![vec![Some(1), Some(2), None], vec![Some(1), Some(3), None]],
                ),
            ] {
                let fixed = testing::binding(
                    &[Some(Value::Number(value)), None, None],
                    computation,
                    counters,
                    location,
                );
                let mut budget = testing::budget();
                let original = Join::component(
                    scope.literals,
                    3,
                    scope.used,
                    &fixed,
                    support,
                    &mut budget,
                    Context::new(computation, &limits, counters, location),
                )
                .unwrap();
                let original = rows(original, computation, counters, location);
                assert_eq!(original, expected);
                let prepared = plans
                    .join(
                        0,
                        scope,
                        &fixed,
                        support,
                        &mut budget,
                        Context::new(computation, &limits, counters, location),
                    )
                    .unwrap();
                assert!(matches!(prepared.plan, JoinPlan::Borrowed(_)));
                assert_eq!(rows(prepared, computation, counters, location), original);
            }
        });
        assert_eq!(
            counts,
            route::Counts {
                preparations: 1,
                cursors: 4
            }
        );
    });
}

#[test]
fn changed_fixed_shape_is_refused_before_traversal() {
    testing::with_completed_source(SOURCE, |program, support, computation, counters| {
        let rule = &program.rules_without_facts()[0];
        let location = rule.location;
        let limits = FormulaLimits::default();
        let fixed = testing::binding(
            &[Some(Value::Number(1)), None],
            computation,
            counters,
            location,
        );
        let wrong_shape = testing::binding(
            &[None, Some(Value::Number(2))],
            computation,
            counters,
            location,
        );
        let wrong_length =
            testing::binding(&[Some(Value::Number(1))], computation, counters, location);
        let mut plans = ComponentPlans::new(
            1,
            2,
            &fixed,
            support,
            Context::new(computation, &limits, counters, location),
        )
        .unwrap();
        let scope = ComponentScope {
            literals: &rule.body,
            used: &[0, 1],
        };
        let mut budget = testing::budget();
        for fixed in [&wrong_shape, &wrong_length] {
            assert!(matches!(
                plans.join(
                    0,
                    scope,
                    fixed,
                    support,
                    &mut budget,
                    Context::new(computation, &limits, counters, location),
                ),
                Err(FormulaFailure::SupportRelation {
                    error: zetesis_core::relation::Failure::Owner,
                    ..
                })
            ));
        }
        let correct = plans
            .join(
                0,
                scope,
                &fixed,
                support,
                &mut budget,
                Context::new(computation, &limits, counters, location),
            )
            .unwrap();
        assert_eq!(rows(correct, computation, counters, location).len(), 2);
    });
}

#[test]
fn another_component_cannot_reuse_a_prepared_slot() {
    testing::with_completed_source(SOURCE, |program, support, computation, counters| {
        let rule = &program.rules_without_facts()[0];
        let location = rule.location;
        let limits = FormulaLimits::default();
        let fixed = testing::binding(
            &[Some(Value::Number(1)), None],
            computation,
            counters,
            location,
        );
        let mut plans = ComponentPlans::new(
            1,
            2,
            &fixed,
            support,
            Context::new(computation, &limits, counters, location),
        )
        .unwrap();
        let scope = ComponentScope {
            literals: &rule.body,
            used: &[0, 1],
        };
        let mut budget = testing::budget();
        drop(
            plans
                .join(
                    0,
                    scope,
                    &fixed,
                    support,
                    &mut budget,
                    Context::new(computation, &limits, counters, location),
                )
                .unwrap(),
        );
        let other = ComponentScope {
            literals: &rule.body[..1],
            used: scope.used,
        };
        assert!(matches!(
            plans.join(
                0,
                other,
                &fixed,
                support,
                &mut budget,
                Context::new(computation, &limits, counters, location),
            ),
            Err(FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Owner,
                ..
            })
        ));
    });
}

#[test]
fn another_support_owner_is_refused() {
    // Both query owners borrow the same completed source directly. Reborrowing
    // one Support through the other would give them different source lifetimes.
    let preparation = testing::prepare(SOURCE);
    let program = preparation.program;
    let limits = preparation.limits;
    let location = preparation.location;
    let mut budget = preparation.budget;
    let mut counters = Counters::resume(
        preparation.accounting,
        crate::grounding_observer::Work::default(),
    );
    let mut catalog = crate::formula_support::build(
        preparation.catalog,
        &program,
        None,
        &limits,
        &mut budget,
        &mut counters,
        location,
    )
    .unwrap();
    let (completed, mut append) = catalog.split(&limits, &mut counters, location).unwrap();
    let queries = completed
        .queries(crate::JoinStrategy::Indexed, &limits, &counters, location)
        .unwrap();
    let other = completed
        .queries(crate::JoinStrategy::Indexed, &limits, &counters, location)
        .unwrap();
    let support = queries.support();
    let mut computation = Computation::new(&mut append, support);
    let rule = &program.rules_without_facts()[0];
    let location = rule.location;
    let fixed = testing::binding(
        &[Some(Value::Number(1)), None],
        &mut computation,
        &mut counters,
        location,
    );
    let mut plans = ComponentPlans::new(
        1,
        2,
        &fixed,
        support,
        Context::new(&computation, &limits, &mut counters, location),
    )
    .unwrap();
    let scope = ComponentScope {
        literals: &rule.body,
        used: &[0, 1],
    };
    assert!(matches!(
        plans.join(
            0,
            scope,
            &fixed,
            other.support(),
            &mut testing::budget(),
            Context::new(&computation, &limits, &mut counters, location),
        ),
        Err(FormulaFailure::SupportRelation {
            error: zetesis_core::relation::Failure::Owner,
            ..
        })
    ));
    assert!(plans.slots.slice()[0].is_none());
}

#[test]
fn plan_payload_is_shared_and_released_with_its_owner() {
    testing::with_completed_source(SOURCE, |program, support, computation, counters| {
        let rule = &program.rules_without_facts()[0];
        let location = rule.location;
        let limits = FormulaLimits::default();
        let fixed = testing::binding(
            &[Some(Value::Number(1)), None],
            computation,
            counters,
            location,
        );
        let before = support.live_bytes();
        let mut plans = ComponentPlans::new(
            1,
            2,
            &fixed,
            support,
            Context::new(computation, &limits, counters, location),
        )
        .unwrap();
        let scope = ComponentScope {
            literals: &rule.body,
            used: &[0, 1],
        };
        let mut budget = testing::budget();
        drop(
            plans
                .join(
                    0,
                    scope,
                    &fixed,
                    support,
                    &mut budget,
                    Context::new(computation, &limits, counters, location),
                )
                .unwrap(),
        );
        let bytes = support.live_bytes();
        let metadata = plans.slots.slice()[0].as_ref().unwrap();
        let expected = size_of::<ComponentPlans<'_, '_>>()
            + plans.slots.capacity() * size_of::<Option<Prepared<'_>>>()
            + plans.bound.capacity() * size_of::<bool>()
            + usize::try_from(metadata.plan.retained_bytes()).unwrap();
        assert_eq!(bytes, before + expected);
        for _ in 0..3 {
            drop(
                plans
                    .join(
                        0,
                        scope,
                        &fixed,
                        support,
                        &mut budget,
                        Context::new(computation, &limits, counters, location),
                    )
                    .unwrap(),
            );
            assert_eq!(support.live_bytes(), bytes);
        }
        drop(plans);
        assert_eq!(support.live_bytes(), before);
    });
}

#[test]
fn refused_preparation_leaves_no_published_plan() {
    testing::with_completed_source(SOURCE, |program, support, computation, counters| {
        let rule = &program.rules_without_facts()[0];
        let location = rule.location;
        let limits = FormulaLimits::default();
        let fixed = testing::binding(
            &[Some(Value::Number(1)), None],
            computation,
            counters,
            location,
        );
        let scope = ComponentScope {
            literals: &rule.body,
            used: &[0, 1],
        };
        let mut attempt = |allowance| {
            let before = support.live_bytes();
            let mut plans = ComponentPlans::new(
                1,
                2,
                &fixed,
                support,
                Context::new(computation, &limits, counters, location),
            )
            .unwrap();
            let mut budget = Budget::new(
                ExpansionLimits {
                    max_term_work: allowance,
                    ..Default::default()
                },
                usize::MAX,
            );
            let result = plans
                .join(
                    0,
                    scope,
                    &fixed,
                    support,
                    &mut budget,
                    Context::new(computation, &limits, counters, location),
                )
                .map(drop);
            let published = plans.slots.slice()[0].is_some();
            let work = budget.usage().term_work;
            drop(plans);
            assert_eq!(support.live_bytes(), before);
            (result, work, published)
        };
        let (result, work, published) = attempt(ExpansionLimits::default().max_term_work);
        result.unwrap();
        assert!(published);
        for allowance in 0..work {
            let (result, _, published) = attempt(allowance);
            assert!(matches!(
                result,
                Err(FormulaFailure::Expansion(ExpansionFailure::Limit {
                    resource: ExpansionResource::TermWork,
                    ..
                }))
            ));
            assert!(!published);
        }
    });
}

#[test]
fn a_retained_plan_obeys_a_lowered_storage_limit() {
    testing::with_completed_source(SOURCE, |program, support, computation, counters| {
        let rule = &program.rules_without_facts()[0];
        let location = rule.location;
        let limits = FormulaLimits::default();
        let fixed = testing::binding(
            &[Some(Value::Number(1)), None],
            computation,
            counters,
            location,
        );
        let mut plans = ComponentPlans::new(
            1,
            2,
            &fixed,
            support,
            Context::new(computation, &limits, counters, location),
        )
        .unwrap();
        let scope = ComponentScope {
            literals: &rule.body,
            used: &[0, 1],
        };
        let mut budget = testing::budget();
        drop(
            plans
                .join(
                    0,
                    scope,
                    &fixed,
                    support,
                    &mut budget,
                    Context::new(computation, &limits, counters, location),
                )
                .unwrap(),
        );
        let limited = FormulaLimits {
            max_support_bytes: support.live_bytes() - 1,
            ..limits
        };
        assert!(matches!(
            plans.join(
                0,
                scope,
                &fixed,
                support,
                &mut budget,
                Context::new(computation, &limited, counters, location),
            ),
            Err(FormulaFailure::Limit {
                resource: FormulaResource::SupportBytes,
                ..
            })
        ));
    });
}

#[test]
fn a_retained_plan_observes_cancellation() {
    testing::with_completed_source(SOURCE, |program, support, computation, counters| {
        let rule = &program.rules_without_facts()[0];
        let location = rule.location;
        let limits = FormulaLimits::default();
        let fixed = testing::binding(
            &[Some(Value::Number(1)), None],
            computation,
            counters,
            location,
        );
        let mut plans = ComponentPlans::new(
            1,
            2,
            &fixed,
            support,
            Context::new(computation, &limits, counters, location),
        )
        .unwrap();
        let scope = ComponentScope {
            literals: &rule.body,
            used: &[0, 1],
        };
        let mut budget = testing::budget();
        drop(
            plans
                .join(
                    0,
                    scope,
                    &fixed,
                    support,
                    &mut budget,
                    Context::new(computation, &limits, counters, location),
                )
                .unwrap(),
        );
        let cancellation = zetesis_cpu::Cancellation::default();
        cancellation.cancel();
        counters.attach_test_control(&cancellation);
        let failure = plans
            .join(
                0,
                scope,
                &fixed,
                support,
                &mut budget,
                Context::new(computation, &limits, counters, location),
            )
            .err()
            .expect("cancelled cursor construction");
        assert!(failure.interruption().is_some());
        counters.detach_test_control();
    });
}

#[test]
fn sharing_keeps_exact_formula_graphs_and_source_origins() {
    for source in [
        "d(1..3). {p(X,Y):d(Y);q(X,Y):d(Y)} :- d(X). h(X) :- p(X,_),q(X,_).",
        "{p(1,a);p(1,b);q(2,a);q(2,b)}. h(X,X+1) :- p(X,_),q(X+1,_).",
        "{p(1,a);p(1,b);q(2,a);q(2,b);a}. h(X,X+1) :- p(X,_),q(X+1,_),not a.",
        "{p(1,a);q(2,a);a}. h(X,X+1) :- p(X,_),q(X+1,_). p(1,b) :- h(1,2),a.",
        "{p(1,a);q(2,a);a}. h(X,X+1) :- p(X,_),q(X+1,_). h(1,9) :- a.",
        "{p(1,a);q(3,a);a}. h(X,X+1) :- p(X,_),q(X+1,_). h(1,2) :- a.",
    ] {
        let compile =
            || crate::formula_ground::ground(testing::prepare(source), None, None).unwrap();
        let (prepared, counts) = route::scoped(true, compile);
        let (ordinary, skipped) = route::scoped(false, compile);
        assert!(counts.preparations > 0, "prepared route: {source}");
        assert_eq!(skipped, route::Counts::default());
        assert_eq!(
            crate::test_support::compiled_atoms(&prepared),
            crate::test_support::compiled_atoms(&ordinary)
        );
        assert_eq!(
            prepared.theory.nodes(),
            ordinary.theory.nodes(),
            "nodes: {source}"
        );
        assert_eq!(
            prepared.theory.operands(),
            ordinary.theory.operands(),
            "operands: {source}"
        );
        assert_eq!(
            prepared.theory.roots(),
            ordinary.theory.roots(),
            "roots: {source}"
        );
        assert_eq!(prepared.origins, ordinary.origins, "origins: {source}");
        assert_eq!(prepared.warnings, ordinary.warnings, "warnings: {source}");
    }
}

#[test]
fn repeated_heads_share_preflight_and_emission_plans() {
    let source = "d(1..8). {p(X,Y):d(Y);q(X,Y):d(Y)} :- d(X). h(X) :- p(X,_),q(X,_).";
    let compile = || crate::formula_ground::ground(testing::prepare(source), None, None).unwrap();
    let (prepared, counts) = route::scoped(true, compile);
    let (ordinary, _) = route::scoped(false, compile);
    assert!(counts.preparations > 0);
    assert!(counts.cursors >= 4 * counts.preparations, "{counts:?}");
    assert!(prepared.expansion.term_work < ordinary.expansion.term_work);
}

#[test]
fn rejected_arithmetic_does_not_prepare_component_plans() {
    let source = "{p(1,a);q(2,a);a}. h(X,X+1) :- p(X,_),q(X+1,_). h(z,2) :- a.";
    let (_, counts) = route::scoped(true, || {
        crate::formula_ground::ground(testing::prepare(source), None, None).unwrap()
    });
    assert_eq!(counts, route::Counts::default());
}

#[test]
fn the_reference_control_restores_after_unwind() {
    assert!(route::enabled());
    let failure = std::panic::catch_unwind(|| {
        route::scoped(false, || {
            assert!(!route::enabled());
            assert!(std::thread::spawn(route::enabled).join().unwrap());
            panic!("restore the previous reference control");
        })
    });
    assert!(failure.is_err());
    assert!(route::enabled());
}
