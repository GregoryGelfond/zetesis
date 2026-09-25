use super::*;
use crate::formula_support::testing::Fixture;
use themelios_base::{
    source::SourceId,
    span::{ByteOffset, Span},
};

fn location() -> Location {
    Location {
        source: SourceId::new(97),
        span: Span::empty(ByteOffset::new(0)),
    }
}

fn probe(
    values: &[Option<usize>],
    computation: &Computation<'_, '_>,
    counters: &mut Counters,
) -> Buffer<Option<usize>> {
    let limits = FormulaLimits::default();
    let mut result = Buffer::new(computation, &limits, counters, location()).unwrap();
    for &value in values {
        result
            .push(value, computation, &limits, counters, location())
            .unwrap();
    }
    result
}

fn prepared_map(
    computation: &Computation<'_, '_>,
    counters: &mut Counters,
) -> CoordinateMap<usize, usize> {
    let limits = FormulaLimits::default();
    let mut map = CoordinateMap::new(computation, &limits, counters, location()).unwrap();
    for (key, value) in [(3, 30), (1, 10)] {
        map.insert(
            key,
            value,
            None,
            Context::new(computation, &limits, counters, location()),
        )
        .unwrap();
    }
    map
}

#[test]
fn terminal_entries_transfer_the_existing_lease() {
    Fixture::default().with(location(), |_, computation, counters| {
        let map = prepared_map(computation, counters);
        let pointer = map.entries.as_ptr();
        let capacity = map.entries.capacity();
        let bytes = map.lease.bytes();
        let limits = FormulaLimits::default();
        let observer = computation.lease();
        let available = computation
            .allowance(&observer, &limits, location())
            .unwrap();
        let entries = map.into_entries();
        assert_eq!(entries.map.entries.as_ptr(), pointer);
        assert_eq!(entries.map.entries.capacity(), capacity);
        assert_eq!(entries.map.lease.bytes(), bytes);
        assert_eq!(
            computation
                .allowance(&observer, &limits, location())
                .unwrap(),
            available
        );
        drop(entries);
        assert_eq!(
            computation
                .allowance(&observer, &limits, location())
                .unwrap(),
            available + bytes
        );
    });
}

#[test]
fn coordinate_map_replaces_without_changing_key_order() {
    Fixture::default().with(location(), |_, computation, counters| {
        let limits = FormulaLimits::default();
        let mut map = prepared_map(computation, counters);
        assert_eq!(
            map.insert(
                1,
                11,
                None,
                Context::new(&*computation, &limits, counters, location())
            )
            .unwrap(),
            Some(10)
        );
        assert_eq!(
            map.insert(
                2,
                20,
                None,
                Context::new(&*computation, &limits, counters, location())
            )
            .unwrap(),
            None
        );
        assert_eq!(map.slice(), &[(1, 11), (2, 20), (3, 30)]);
    });
}

#[test]
fn coordinate_map_work_refusal_preserves_entries_for_retry() {
    let needed = Fixture::default().with(location(), |_, computation, counters| {
        let mut map = prepared_map(computation, counters);
        let before = counters.accounting.work;
        map.insert(
            2,
            20,
            None,
            Context::new(
                &*computation,
                &FormulaLimits::default(),
                counters,
                location(),
            ),
        )
        .unwrap();
        counters.accounting.work - before
    });
    for cutoff in 0..needed {
        Fixture::default().with(location(), |_, computation, counters| {
            let mut map = prepared_map(computation, counters);
            let limits = FormulaLimits {
                max_work: counters.accounting.work + cutoff,
                ..Default::default()
            };
            assert!(matches!(
                map.insert(
                    2,
                    20,
                    None,
                    Context::new(computation, &limits, counters, location())
                ),
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::Work,
                    ..
                })
            ));
            assert_eq!(map.slice(), &[(1, 10), (3, 30)]);
            map.insert(
                2,
                20,
                None,
                Context::new(
                    &*computation,
                    &FormulaLimits::default(),
                    counters,
                    location(),
                ),
            )
            .unwrap();
            assert_eq!(map.slice(), &[(1, 10), (2, 20), (3, 30)]);
        });
    }
}

#[test]
fn coordinate_map_replacement_requires_publication_work() {
    Fixture::default().with(location(), |_, computation, counters| {
        let mut map = prepared_map(computation, counters);
        // Searching for the middle key costs one permit; replacement needs its
        // own permit even though it neither allocates nor shifts another entry.
        let limits = FormulaLimits {
            max_work: counters.accounting.work + 1,
            ..Default::default()
        };
        assert!(
            map.insert(
                3,
                31,
                None,
                Context::new(computation, &limits, counters, location())
            )
            .is_err()
        );
        assert_eq!(map.slice(), &[(1, 10), (3, 30)]);
    });
}

#[test]
fn coordinate_map_storage_refusal_preserves_entries_for_retry() {
    let mut refused = 0;
    for extra in 0..96 {
        Fixture::default().with(location(), |_, computation, counters| {
            let mut map = prepared_map(computation, counters);
            let limits = FormulaLimits::default();
            let observer = computation.lease();
            let current = limits.max_support_bytes
                - computation
                    .allowance(&observer, &limits, location())
                    .unwrap();
            let bounded = FormulaLimits {
                max_support_bytes: current + extra,
                ..limits
            };
            if map
                .insert(
                    2,
                    20,
                    None,
                    Context::new(&*computation, &bounded, counters, location()),
                )
                .is_err()
            {
                refused += 1;
                assert_eq!(map.slice(), &[(1, 10), (3, 30)]);
                map.insert(
                    2,
                    20,
                    None,
                    Context::new(&*computation, &limits, counters, location()),
                )
                .unwrap();
                assert_eq!(map.slice(), &[(1, 10), (2, 20), (3, 30)]);
            }
        });
    }
    assert!(refused > 0);
}

struct Payload(usize);

fn prepared_contexts(
    computation: &Computation<'_, '_>,
    counters: &mut Counters,
) -> (Contexts<Payload>, Buffer<Option<usize>>) {
    let limits = FormulaLimits::default();
    let mut contexts = Contexts::new(computation, &limits, counters, location()).unwrap();
    for (value, expected) in [(3, 0), (1, 1)] {
        let key = probe(&[Some(value), None], computation, counters);
        assert_eq!(
            contexts
                .insert(
                    5,
                    &key,
                    Payload(value),
                    Context::new(computation, &limits, counters, location())
                )
                .unwrap(),
            expected
        );
    }
    let key = probe(&[Some(2), None, Some(0)], computation, counters);
    (contexts, key)
}

fn retry_context(
    mut contexts: Contexts<Payload>,
    key: &Buffer<Option<usize>>,
    computation: &Computation<'_, '_>,
    counters: &mut Counters,
) {
    let limits = FormulaLimits::default();
    assert_eq!(
        contexts
            .insert(
                5,
                key,
                Payload(2),
                Context::new(computation, &limits, counters, location())
            )
            .unwrap(),
        2
    );
    assert_eq!(
        contexts
            .find(5, key, computation, &limits, counters, location())
            .unwrap(),
        Some(2)
    );
    assert_eq!(contexts.get(0).unwrap().0, 3);
    assert_eq!(contexts.get(1).unwrap().0, 1);
    assert_eq!(contexts.get(2).unwrap().0, 2);
}

#[test]
fn context_keys_distinguish_absence_arity_and_aggregate() {
    Fixture::default().with(location(), |_, computation, counters| {
        let limits = FormulaLimits::default();
        let mut contexts = Contexts::new(computation, &limits, counters, location()).unwrap();
        let keys: [(usize, &[Option<usize>]); 5] = [
            (1, &[]),
            (1, &[None]),
            (1, &[Some(0)]),
            (1, &[Some(0), None]),
            (2, &[Some(0)]),
        ];
        for (expected, (aggregate, values)) in keys.into_iter().enumerate() {
            let key = probe(values, computation, counters);
            assert_eq!(
                contexts
                    .insert(
                        aggregate,
                        &key,
                        Payload(expected),
                        Context::new(&*computation, &limits, counters, location())
                    )
                    .unwrap(),
                expected
            );
        }
        for (expected, (aggregate, values)) in keys.into_iter().enumerate() {
            let key = probe(values, computation, counters);
            assert_eq!(
                contexts
                    .find(aggregate, &key, computation, &limits, counters, location())
                    .unwrap(),
                Some(expected)
            );
        }
    });
}

#[test]
fn duplicate_context_keeps_original_value() {
    Fixture::default().with(location(), |_, computation, counters| {
        let limits = FormulaLimits::default();
        let (mut contexts, key) = prepared_contexts(computation, counters);
        assert_eq!(
            contexts
                .insert(
                    5,
                    &key,
                    Payload(7),
                    Context::new(&*computation, &limits, counters, location())
                )
                .unwrap(),
            2
        );
        let bytes = contexts.key_bytes();
        assert_eq!(
            contexts
                .insert(
                    5,
                    &key,
                    Payload(9),
                    Context::new(&*computation, &limits, counters, location())
                )
                .unwrap(),
            2
        );
        assert_eq!(contexts.get(2).unwrap().0, 7);
        assert_eq!(contexts.key_bytes(), bytes);
    });
}

#[test]
fn context_search_stops_before_visiting_unpermitted_suffix() {
    Fixture::default().with(location(), |_, computation, counters| {
        let limits = FormulaLimits::default();
        let key = probe(&[Some(7); 128], computation, counters);
        let mut contexts = Contexts::new(computation, &limits, counters, location()).unwrap();
        contexts.insert(1,
&key,
Payload(0),
Context::new(&*computation, &limits, counters, location())).unwrap();
        let before = counters.accounting.work;
        let bounded = FormulaLimits { max_work: before + 2, ..limits };
        assert!(matches!(contexts.find(1, &key, computation, &bounded, counters, location()),
            Err(FormulaFailure::Limit { resource: FormulaResource::Work, observed, .. }) if observed == u128::from(before) + 3));
        assert_eq!(counters.accounting.work, before + 2);
    });
}

#[test]
fn context_work_refusal_publishes_no_partial_row_or_pool() {
    let needed = Fixture::default().with(location(), |_, computation, counters| {
        let (mut contexts, key) = prepared_contexts(computation, counters);
        let before = counters.accounting.work;
        contexts
            .insert(
                5,
                &key,
                Payload(2),
                Context::new(
                    &*computation,
                    &FormulaLimits::default(),
                    counters,
                    location(),
                ),
            )
            .unwrap();
        counters.accounting.work - before
    });
    for cutoff in 0..needed {
        Fixture::default().with(location(), |_, computation, counters| {
            let (mut contexts, key) = prepared_contexts(computation, counters);
            let limits = FormulaLimits {
                max_work: counters.accounting.work + cutoff,
                ..Default::default()
            };
            assert!(matches!(
                contexts.insert(
                    5,
                    &key,
                    Payload(2),
                    Context::new(&*computation, &limits, counters, location())
                ),
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::Work,
                    ..
                })
            ));
            assert_eq!(contexts.rows.len(), 2);
            assert_eq!(contexts.outer, [Some(3), None, Some(1), None]);
            assert_eq!(contexts.order, [1, 0]);
            retry_context(contexts, &key, computation, counters);
        });
    }
}

#[test]
fn context_storage_refusal_preserves_published_coordinates() {
    let mut refused = 0;
    for extra in 0..256 {
        Fixture::default().with(location(), |_, computation, counters| {
            let (mut contexts, key) = prepared_contexts(computation, counters);
            let limits = FormulaLimits::default();
            let observer = computation.lease();
            let current = limits.max_support_bytes
                - computation
                    .allowance(&observer, &limits, location())
                    .unwrap();
            let bounded = FormulaLimits {
                max_support_bytes: current + extra,
                ..limits
            };
            if contexts
                .insert(
                    5,
                    &key,
                    Payload(2),
                    Context::new(&*computation, &bounded, counters, location()),
                )
                .is_err()
            {
                refused += 1;
                assert_eq!(contexts.rows.len(), 2);
                assert_eq!(contexts.outer, [Some(3), None, Some(1), None]);
                assert_eq!(contexts.order, [1, 0]);
                retry_context(contexts, &key, computation, counters);
            }
        });
    }
    assert!(refused > 0);
}

#[test]
fn context_key_ceiling_is_checked_before_any_reserve() {
    Fixture::default().with(location(), |_, computation, counters| {
        let limits = FormulaLimits::default();
        let key = probe(&[None, Some(0)], computation, counters);
        let mut contexts = Contexts::new(computation, &limits, counters, location()).unwrap();
        let required = key_bytes(1, key.len(), 1);
        let bounded = FormulaLimits { max_aggregate_cache_key_bytes: usize::try_from(required - 1).unwrap(), ..limits };
        assert!(matches!(contexts.insert(1,
&key,
Payload(0),
Context::new(&*computation, &bounded, counters, location())),
            Err(FormulaFailure::Limit { resource: FormulaResource::AggregateCacheKeys, observed, .. }) if observed == required));
        assert_eq!(contexts.key_bytes(), 0);
        assert_eq!(contexts.rows.len(), 0);
    });
}

#[test]
fn context_spare_capacity_stays_charged_after_work_refusal() {
    Fixture::default().with(location(), |_, computation, counters| {
        let limits = FormulaLimits::default();
        let key = probe(&[Some(7); 8], computation, counters);
        let mut contexts = Contexts::new(computation, &limits, counters, location()).unwrap();
        // Each initially empty backing vector needs one reserve permit. Stop
        // after those three permits, before the single publication permit batch.
        let bounded = FormulaLimits {
            max_work: counters.accounting.work + 3,
            ..limits
        };
        assert!(
            contexts
                .insert(
                    1,
                    &key,
                    Payload(0),
                    Context::new(&*computation, &bounded, counters, location())
                )
                .is_err()
        );
        assert_eq!(contexts.rows.len(), 0);
        assert!(contexts.key_bytes() > 0);
        assert_eq!(
            contexts.lease.bytes(),
            contexts.storage_without(0, location()).unwrap()
        );
        let before = contexts.lease.bytes();
        contexts
            .insert(
                1,
                &key,
                Payload(0),
                Context::new(&*computation, &limits, counters, location()),
            )
            .unwrap();
        assert_eq!(contexts.lease.bytes(), before);
    });
}

#[test]
fn repeated_context_growth_uses_geometric_capacity() {
    Fixture::default().with(location(), |_, computation, counters| {
        let limits = FormulaLimits::default();
        let key = probe(&[], computation, counters);
        let mut contexts = Contexts::new(computation, &limits, counters, location()).unwrap();
        let mut growths = 0;
        for aggregate in 0..65 {
            let before = contexts.rows.capacity();
            assert_eq!(
                contexts
                    .insert(
                        aggregate,
                        &key,
                        Payload(aggregate),
                        Context::new(&*computation, &limits, counters, location())
                    )
                    .unwrap(),
                aggregate
            );
            growths += usize::from(contexts.rows.capacity() > before);
        }
        assert!(growths <= 8);
        for aggregate in 0..65 {
            assert_eq!(
                contexts
                    .find(aggregate, &key, computation, &limits, counters, location())
                    .unwrap(),
                Some(aggregate)
            );
            assert_eq!(contexts.get(aggregate).unwrap().0, aggregate);
        }
        assert_eq!(
            contexts.lease.bytes(),
            contexts.storage_without(0, location()).unwrap()
        );
    });
}

#[test]
fn exact_context_growth_preserves_the_key_allowance() {
    Fixture::default().with(location(), |_, computation, counters| {
        let (mut contexts, key) = prepared_contexts(computation, counters);
        let required = key_bytes(3, 7, 3);
        let limits = FormulaLimits {
            max_aggregate_cache_key_bytes: usize::try_from(required).unwrap(),
            ..Default::default()
        };
        assert_eq!(
            contexts
                .insert(
                    5,
                    &key,
                    Payload(2),
                    Context::new(&*computation, &limits, counters, location())
                )
                .unwrap(),
            2
        );
        assert_eq!(contexts.key_bytes(), required);
        assert_eq!(
            contexts
                .find(5, &key, computation, &limits, counters, location())
                .unwrap(),
            Some(2)
        );
    });
}

#[test]
fn exact_context_growth_preserves_sibling_replacement_space() {
    Fixture::default().with(location(), |_, computation, counters| {
        let (mut contexts, key) = prepared_contexts(computation, counters);
        let limits = FormulaLimits::default();
        let outer = limits.max_support_bytes
            - computation
                .allowance(&contexts.lease, &limits, location())
                .unwrap();
        let header = size_of::<Contexts<Payload>>();
        let rows = size_of::<Row<Payload>>();
        let pool = size_of::<Option<usize>>();
        let index = size_of::<usize>();
        // Each exact growth sees the capacities already replaced, the old
        // capacity of its own buffer, and the still-retained later siblings.
        let peak = [
            header + 5 * rows + 4 * pool + 2 * index,
            header + 3 * rows + 11 * pool + 2 * index,
            header + 3 * rows + 7 * pool + 5 * index,
        ]
        .into_iter()
        .max()
        .unwrap();
        let bounded = FormulaLimits {
            max_support_bytes: outer + peak,
            ..limits
        };
        assert_eq!(
            contexts
                .insert(
                    5,
                    &key,
                    Payload(2),
                    Context::new(&*computation, &bounded, counters, location())
                )
                .unwrap(),
            2
        );
        assert_eq!(contexts.rows.capacity(), 3);
        assert_eq!(contexts.outer.capacity(), 7);
        assert_eq!(contexts.order.capacity(), 3);
        assert_eq!(
            contexts.lease.bytes(),
            header + 3 * rows + 7 * pool + 3 * index
        );
    });
}

#[test]
fn empty_coordinate_map_rejects_another_workspace() {
    let mut first = Fixture::default();
    let map = first.with(location(), |_, computation, counters| {
        CoordinateMap::<usize, usize>::new(
            computation,
            &FormulaLimits::default(),
            counters,
            location(),
        )
        .unwrap()
    });
    Fixture::default().with(location(), |_, computation, counters| {
        assert!(matches!(
            map.find(
                &1,
                computation,
                &FormulaLimits::default(),
                counters,
                location()
            ),
            Err(FormulaFailure::TermAssignment {
                error: AssignmentError::Read(zetesis_core::catalog::ReadError::ForeignCatalog),
                ..
            })
        ));
    });
}

#[test]
fn aggregate_buffer_capacity_remains_charged_until_last_shared_owner_drops() {
    Fixture::default().with(location(), |_, computation, counters| {
        let limits = FormulaLimits::default();
        let observer = computation.lease();
        let before = computation
            .allowance(&observer, &limits, location())
            .unwrap();
        let mut elements = Buffer::new(computation, &limits, counters, location()).unwrap();
        elements
            .push(
                zetesis_ferraris::AggregateElement {
                    weight: 1,
                    condition: 0,
                },
                computation,
                &limits,
                counters,
                location(),
            )
            .unwrap();
        let aggregate = super::super::GroundAggregate::Numeric(std::sync::Arc::new(elements));
        let retained = computation
            .allowance(&observer, &limits, location())
            .unwrap();
        assert!(retained < before);
        let shared = aggregate.clone();
        assert_eq!(
            computation
                .allowance(&observer, &limits, location())
                .unwrap(),
            retained
        );
        drop(aggregate);
        assert_eq!(
            computation
                .allowance(&observer, &limits, location())
                .unwrap(),
            retained
        );
        drop(shared);
        assert_eq!(
            computation
                .allowance(&observer, &limits, location())
                .unwrap(),
            before
        );
    });
}
