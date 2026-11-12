//! A later refusal preserves exactly the completed stages of one actual Builder.
use super::*;
use crate::formula_ground::{Builder, GroundAggregate, Purpose};
use std::sync::Arc;

fn assert_stages(
    owner: &Workspace<'_, '_>,
    produced: &Rc<Produced<'_>>,
    native: bool,
    roots: Option<usize>,
) -> Option<GroundAggregate> {
    let rows = owner.rows.borrow();
    assert_eq!(rows.len(), 1);
    let family = rows.get(0).unwrap();
    assert!(Rc::ptr_eq(family.produced.as_ref().unwrap(), produced));
    assert_eq!(owner.cached_elements.get(), usize::from(native));
    assert_eq!(owner.cached_roots.get(), roots.unwrap_or(0));
    if native {
        let cached = family.native.as_ref().unwrap();
        assert_eq!(cached.elements.len(), 1);
        assert_eq!(cached.required.slice(), [false]);
        assert_eq!(cached.roots.as_ref().map(|values| values.len()), roots);
        Some(cached.elements.clone())
    } else {
        assert!(family.native.is_none());
        assert_eq!(roots, None);
        None
    }
}

fn refuse_native<'source>(
    builder: &mut Builder<'_, '_, 'source>,
    aggregate: &'source AggregateIr,
    frame: &Binding,
    support: &Support<'source>,
    location: ProgramSite,
    produced: &Rc<Produced<'source>>,
    resource: FormulaResource,
) -> (Option<GroundAggregate>, u64) {
    let roots_refusal = resource == FormulaResource::AggregateCacheRoots;
    let before = builder.counters.accounting.work;
    let error = builder
        .assignment_aggregate(
            Ir::Source(aggregate),
            aggregate.binding.unwrap(),
            frame,
            support,
            location,
            None,
        )
        .unwrap_err();
    let expected = if roots_refusal { 2 } else { 1 };
    assert!(matches!(error, FormulaFailure::Limit {
        resource: actual, limit: 0, observed, ..
    } if actual == resource && observed == expected));
    let refused_work = builder.counters.accounting.work;
    assert!(
        refused_work > before,
        "completed native construction remains performed work"
    );
    assert_eq!(testing::counts().consumed, 1);
    (
        assert_stages(&builder.families, produced, roots_refusal, None),
        refused_work,
    )
}

fn assert_same_elements(retained: GroundAggregate, completed: GroundAggregate) {
    let (GroundAggregate::Numeric(retained), GroundAggregate::Numeric(completed)) =
        (retained, completed)
    else {
        panic!("the count fixture has numeric elements")
    };
    assert!(
        Arc::ptr_eq(&retained, &completed),
        "root refusal keeps actual native storage"
    );
}

fn refused_stage(resource: FormulaResource) {
    with_completed_family(|queries, aggregate, variables, budget, context| {
        let Context { computation, work } = context;
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        let roots_refusal = resource == FormulaResource::AggregateCacheRoots;
        let bounded = FormulaLimits {
            max_aggregate_cache_elements: usize::from(roots_refusal),
            max_aggregate_cache_roots: 0,
            ..*limits
        };
        let inclusive = FormulaLimits {
            max_aggregate_cache_elements: 1,
            max_aggregate_cache_roots: 2,
            ..*limits
        };
        let target = aggregate.binding.unwrap();
        let mut frame = Binding::new(computation, &bounded, counters, location).unwrap();
        frame
            .extend_scope(variables, computation, &bounded, counters, location)
            .unwrap();
        let zero = computation.number(0, &bounded, counters, location).unwrap();
        frame
            .set(target, &zero, &bounded, counters, location)
            .unwrap();
        let mut builder = Builder::empty(
            computation,
            &bounded,
            budget,
            counters,
            Purpose::Validation,
            None,
            location,
        )
        .unwrap();
        builder.initialize(location).unwrap();
        let owner = Rc::clone(&builder.families);
        owner.enable(queries, location).unwrap();
        testing::scoped(true, || {
            let carrier = owner
                .values(
                    Ir::Source(aggregate),
                    &frame,
                    queries.support(),
                    builder.budget,
                    Context::new(
                        &mut *builder.computation,
                        builder.limits,
                        builder.counters,
                        location,
                    ),
                )
                .unwrap();
            let Carrier::Shared(produced) = carrier else {
                panic!("completed shared carrier")
            };
            assert_optional_unit_count(&produced.values, builder.computation);
            assert!(
                produced.replay.is_some(),
                "this exercises the certified native route"
            );
            assert_stages(&owner, &produced, false, None);
            let (retained, refused_work) = refuse_native(
                &mut builder,
                aggregate,
                &frame,
                queries.support(),
                location,
                &produced,
                resource,
            );
            // Resume this same owner with an inclusive limit, without refunding
            // work or rolling back real interned terms from the refused draft.
            builder.limits = &inclusive;
            builder
                .assignment_aggregate(
                    Ir::Source(aggregate),
                    target,
                    &frame,
                    queries.support(),
                    location,
                    None,
                )
                .unwrap();
            assert!(builder.counters.accounting.work > refused_work);
            let completed = assert_stages(&owner, &produced, true, Some(2)).unwrap();
            if let Some(retained) = retained {
                assert_same_elements(retained, completed);
            }
            assert_eq!(testing::counts().produced, 1);
            assert_eq!(
                testing::counts().consumed,
                if roots_refusal { 1 } else { 2 }
            );
        });
    });
}

#[test]
fn native_element_refusal_keeps_produced_and_publishes_no_native_stage() {
    refused_stage(FormulaResource::AggregateCacheElements);
}

#[test]
fn native_root_refusal_keeps_the_completed_element_owner() {
    refused_stage(FormulaResource::AggregateCacheRoots);
}

#[test]
fn owned_local_plan_retains_carrier_without_replay() {
    with_completed_family(|queries, aggregate, variables, budget, context| {
        let Context { computation, work } = context;
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        let owner =
            Workspace::new(Context::new(&*computation, limits, counters, location)).unwrap();
        owner.enable(queries, location).unwrap();
        let mut frame = Binding::new(computation, limits, counters, location).unwrap();
        frame
            .extend_scope(variables, computation, limits, counters, location)
            .unwrap();
        crate::formula_support::testing::without_local_plans(|| {
            testing::scoped(true, || {
                let carrier = owner
                    .values(
                        Ir::Source(aggregate),
                        &frame,
                        queries.support(),
                        budget,
                        Context::new(&mut *computation, limits, counters, location),
                    )
                    .unwrap();
                let Carrier::Shared(produced) = carrier else {
                    panic!("completed shared carrier")
                };
                assert_optional_unit_count(&produced.values, computation);
                assert!(
                    produced.replay.is_none(),
                    "an owned plan cannot certify later replay"
                );
                assert_stages(&owner, &produced, false, None);
                assert_eq!(testing::counts().produced, 1);
                assert_eq!(testing::counts().consumed, 0);
            });
        });
    });
}
