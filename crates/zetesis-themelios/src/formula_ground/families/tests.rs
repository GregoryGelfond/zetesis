//! Exact ownership and requested-versus-observed shared header boundaries.
use super::*;
use crate::formula_support::testing::Fixture;
use crate::formula_support::{Accounting, Counters, SupportCatalog};
use crate::grounding_observer::Profile;
use crate::test_support::location;
use crate::{FormulaLimits, GroundingObserver, GroundingOutcome, GroundingPhase};
use zetesis_core::catalog::{AssignmentError, ReadError};

#[derive(Default)]
struct Peak(std::cell::Cell<Option<u64>>);
impl GroundingObserver for Peak {
    fn enter(&self) {}
    fn exit(&self) {}
    fn details_enabled(&self) -> bool {
        true
    }
    fn phase_exit(
        &self,
        _: GroundingPhase,
        _: Option<ProgramSite>,
        _: GroundingOutcome,
        work: crate::GroundingWork,
    ) {
        self.0.set(work.support_peak_bytes);
    }
}

fn live_bytes(computation: &Computation<'_, '_>) -> usize {
    let limits = FormulaLimits::default();
    let lease = computation.lease();
    limits.max_support_bytes - computation.allowance(&lease, &limits, location()).unwrap()
}

#[test]
fn workspace_header_records_only_allocated_bytes_at_exact_boundary() {
    for short in [false, true] {
        Fixture::default().with(location(), |_support, computation, _| {
            let baseline = live_bytes(computation);
            let prefix = size_of::<TermTable>() + size_of::<Contexts<Family<'_>>>();
            let complete = size_of::<Workspace<'_, '_>>() + 2 * size_of::<usize>();
            assert!(complete > prefix);
            let limits = FormulaLimits {
                max_support_bytes: baseline + complete - usize::from(short),
                ..Default::default()
            };
            let observer = Peak::default();
            let profile = Profile::new(Some(&observer));
            let mut counters = Counters::resume(Accounting::default(), profile.work());
            let result = profile.phase(GroundingPhase::RuleInstantiation, None, || {
                Workspace::new(Context::new(
                    computation,
                    &limits,
                    &mut counters,
                    location(),
                ))
            });
            if short {
                assert!(matches!(result, Err(FormulaFailure::Limit {
                    resource: FormulaResource::SupportBytes, limit, observed, ..
                }) if limit == (baseline + complete - 1) as u128
                    && observed == (baseline + complete) as u128));
                assert_eq!(observer.0.get(), Some((baseline + prefix) as u64));
                assert_eq!(live_bytes(computation), baseline);
                assert_eq!(counters.accounting.work, 0);
            } else {
                let owner = result.unwrap_or_else(|error| panic!("{error}"));
                assert_eq!(observer.0.get(), Some((baseline + complete) as u64));
                assert_eq!(live_bytes(computation), baseline + complete);
                assert_eq!(counters.accounting.work, 1);
                drop(owner);
                assert_eq!(live_bytes(computation), baseline);
            }
        });
    }
}

#[test]
fn produced_header_admission_does_not_observe_future_capacity() {
    Fixture::default().with(location(), |_support, computation, counters| {
        let limits = FormulaLimits::default();
        let values = Binding::new(computation, &limits, counters, location()).unwrap();
        let baseline = live_bytes(computation);
        let bytes = size_of::<Produced>() - size_of::<Binding>() + 2 * size_of::<usize>();
        let lease = computation.lease();
        let observer = Peak::default();
        let profile = Profile::new(Some(&observer));
        // A pure admission has no Counters argument; establish a named observed
        // baseline separately and show both requests leave it unchanged.
        let tracked = Counters::resume(Accounting::default(), profile.work());
        for short in [true, false] {
            let limits = FormulaLimits {
                max_support_bytes: baseline + bytes - usize::from(short),
                ..limits
            };
            let result = profile.phase(GroundingPhase::RuleInstantiation, None, || {
                computation
                    .storage_observed(&lease, 0, 0, &limits, &tracked, location())
                    .unwrap();
                admit_shared_owner(&lease, bytes, computation, &limits, location())
            });
            if short {
                assert!(matches!(result, Err(FormulaFailure::Limit {
                    resource: FormulaResource::SupportBytes, limit, observed, ..
                }) if limit == (baseline + bytes - 1) as u128
                    && observed == (baseline + bytes) as u128));
            } else {
                result.unwrap();
            }
            assert_eq!(observer.0.get(), Some(baseline as u64));
            assert_eq!(lease.bytes(), 0);
            assert_eq!(live_bytes(computation), baseline);
            assert_eq!(tracked.accounting.work, 0);
        }
        drop(values);
    });
}

#[test]
fn empty_key_rejects_foreign_vocabulary_even_with_identical_support() {
    let limits = FormulaLimits::default();
    let mut first = SupportCatalog::default();
    let mut second = SupportCatalog::default();
    let mut counters = Counters::default();
    let (relations, mut own_append) = first.split(&limits, &mut counters, location()).unwrap();
    let (_foreign_relations, mut foreign_append) =
        second.split(&limits, &mut counters, location()).unwrap();
    let support = Support::indexed(&relations, &limits, &counters, location()).unwrap();
    let own = Computation::new(&mut own_append, &support);
    let foreign = Computation::new(&mut foreign_append, &support);
    let workspace = Workspace::new(Context::new(&own, &limits, &mut counters, location())).unwrap();
    let binding = Binding::new(&foreign, &limits, &mut counters, location()).unwrap();
    // Both the Support pointer and the incoming Binding/current reader agree.
    // Only authentication of the retained term owner detects this mismatch.
    workspace.authenticate(&support, location()).unwrap();
    binding
        .slots()
        .prefix(0)
        .unwrap()
        .bind_with(foreign.read(), || Ok::<(), FormulaFailure>(()))
        .unwrap();
    let result = workspace.key(
        &[],
        &binding,
        Context::new(&foreign, &limits, &mut counters, location()),
    );
    assert!(matches!(
        result,
        Err(FormulaFailure::TermAssignment {
            error: AssignmentError::Read(ReadError::ForeignCatalog),
            ..
        })
    ));
    assert_eq!(workspace.rows.borrow().len(), 0);
    assert_eq!(workspace.cached_elements.get(), 0);
    assert_eq!(workspace.cached_roots.get(), 0);
    let own_binding = Binding::new(&own, &limits, &mut counters, location()).unwrap();
    let key = workspace
        .key(
            &[],
            &own_binding,
            Context::new(&own, &limits, &mut counters, location()),
        )
        .unwrap();
    assert!(key.slice().is_empty());
}

/// Each test gets a real completed catalog/query and its exact source IR.
/// The closure cannot retain a query, appender, frame or source beyond this owner.
fn with_completed_family(
    run: impl for<'query, 'source, 'call> FnOnce(
        &'query CompletedQueries<'source>,
        &'source AggregateIr,
        usize,
        &'call mut Budget,
        Context<'call, &'call mut Computation<'query, 'source>>,
    ),
) {
    // A chosen atom: a fact's would be certain, and its tuple required.
    let preparation = crate::formula_support::testing::prepare("{p}. q(N) :- N=#count{1:p}.");
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
    let mut computation = Computation::new(&mut append, queries.support());
    let rule = program
        .rules_without_facts()
        .iter()
        .find(|rule| rule.bindings.is_some())
        .unwrap();
    let aggregate = rule
        .body
        .iter()
        .find_map(|literal| match literal {
            crate::formula_ir::LiteralIr::Aggregate(aggregate) => Some(aggregate),
            _ => None,
        })
        .unwrap();
    assert!(aggregate.family_inputs.is_empty());
    assert_eq!(aggregate.elements.len(), 1);
    assert!(
        !aggregate.elements[0].required,
        "an atom condition is interpretation-dependent"
    );
    run(
        &queries,
        aggregate,
        rule.body_variables,
        &mut budget,
        Context::new(&mut computation, &limits, &mut counters, location),
    );
}

fn assert_optional_unit_count(values: &Binding, computation: &Computation<'_, '_>) {
    // The candidate carrier includes the empty and singleton selection. The
    // original atom condition and equality still decide the emitted formula.
    let actual: Vec<_> = (0..values.len())
        .map(|slot| {
            let key = values.slots().key(slot).unwrap().unwrap();
            match computation.read().term(&key).unwrap().descriptor() {
                zetesis_core::ValueNodeRef::Number(number) => number,
                other => panic!("a numeric count proposal, not {other:?}"),
            }
        })
        .collect();
    assert_eq!(actual, [0, 1]);
}

#[test]
fn independent_empty_frames_reuse_one_completed_carrier() {
    with_completed_family(|queries, aggregate, variables, budget, context| {
        let Context { computation, work } = context;
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        let workspace =
            Workspace::new(Context::new(&*computation, limits, counters, location)).unwrap();
        workspace.enable(queries, location).unwrap();
        let mut first = Binding::new(computation, limits, counters, location).unwrap();
        first
            .extend_scope(variables, computation, limits, counters, location)
            .unwrap();
        let mut second = Binding::new(computation, limits, counters, location).unwrap();
        second
            .extend_scope(variables, computation, limits, counters, location)
            .unwrap();
        let (first, second, events) = testing::scoped(true, || {
            let first = workspace
                .values(
                    Ir::Source(aggregate),
                    &first,
                    queries.support(),
                    budget,
                    Context::new(&mut *computation, limits, counters, location),
                )
                .unwrap();
            let second = workspace
                .values(
                    Ir::Source(aggregate),
                    &second,
                    queries.support(),
                    budget,
                    Context::new(&mut *computation, limits, counters, location),
                )
                .unwrap();
            (first, second, testing::counts())
        });
        assert_eq!(events.produced, 1);
        assert_eq!(
            events.reused, 1,
            "a registry hit, not an existing Cursor rewind"
        );
        assert_eq!(
            events.carrier_borrowed, 0,
            "this isolates the producer-side registry"
        );
        let (Carrier::Shared(first), Carrier::Shared(second)) = (&first, &second) else {
            panic!("the enabled completed workspace returns shared carriers")
        };
        assert!(Rc::ptr_eq(first, second));
        assert_optional_unit_count(&first.values, computation);
        assert_eq!(workspace.rows.borrow().len(), 1);
        assert_eq!(
            workspace.cached_elements.get(),
            0,
            "no native eligibility was emitted"
        );
        assert_eq!(workspace.cached_roots.get(), 0);
    });
}

#[test]
fn completed_carrier_retention_checks_row_ceiling_before_native_publication() {
    for row_limit in [0, 1] {
        with_completed_family(|queries, aggregate, variables, budget, context| {
            let Context { computation, work } = context;
            let GroundingWork {
                limits,
                counters,
                location,
            } = work;
            let bounded = FormulaLimits {
                max_aggregate_cache_rows: row_limit,
                ..*limits
            };
            let workspace =
                Workspace::new(Context::new(&*computation, &bounded, counters, location)).unwrap();
            workspace.enable(queries, location).unwrap();
            let mut frame = Binding::new(computation, &bounded, counters, location).unwrap();
            frame
                .extend_scope(variables, computation, &bounded, counters, location)
                .unwrap();
            let (result, events) = testing::scoped(true, || {
                let result = workspace.values(
                    Ir::Source(aggregate),
                    &frame,
                    queries.support(),
                    budget,
                    Context::new(&mut *computation, &bounded, counters, location),
                );
                (result, testing::counts())
            });
            if row_limit == 0 {
                assert!(matches!(
                    result,
                    Err(FormulaFailure::Limit {
                        resource: FormulaResource::AggregateCacheRows,
                        limit: 0,
                        observed: 1,
                        ..
                    })
                ));
                assert_eq!(workspace.rows.borrow().len(), 0);
                assert_eq!(
                    events.produced, 0,
                    "a refused draft is not a completed stage"
                );
            } else {
                let result = result.unwrap();
                assert_optional_unit_count(&result, computation);
                assert_eq!(workspace.rows.borrow().len(), 1);
                let rows = workspace.rows.borrow();
                let family = rows.get(0).unwrap();
                assert!(family.produced.is_some());
                assert!(family.native.is_none());
                assert_eq!(events.produced, 1);
            }
            assert_eq!(events.reused, 0);
            assert_eq!(events.carrier_borrowed, 0);
            assert_eq!(workspace.cached_elements.get(), 0);
            assert_eq!(workspace.cached_roots.get(), 0);
        });
    }
}

mod native_stages;
