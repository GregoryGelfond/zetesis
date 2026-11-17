//! Component evidence comes from actual admitted source and its schedule.

use std::mem::size_of;
use std::num::NonZeroUsize;

use super::{Counters, Footprint, FormulaLimits, Inventory, ProgramSite, Registration, State};
use crate::formula_ir::Prepared;
use crate::formula_support::{SupportCatalog, testing::prepare};
use crate::{FormulaFailure, FormulaResource};
use zetesis_algebra::{Cancellation, Stop};

const INDEPENDENT: &str = "a(N) :- N = #count{X : e(X)}. b(N) :- N = #count{X : f(X)}.";

fn maximum(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}

/// Build the real component schedule and ask it to settle a no-growth round.
/// The fixtures have no fact blocks: empty prerequisite relations are complete.
fn ready<'source>(
    program: &'source Prepared,
    catalog: &mut SupportCatalog,
    location: ProgramSite,
) -> Registration<'source> {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let view = catalog
        .component_view(&limits, &mut counters, location)
        .unwrap()
        .unwrap();
    let mut registration = Registration::prepare(
        program.rules_without_facts(),
        Some(view),
        0,
        &limits,
        &mut counters,
        location,
    )
    .unwrap();
    registration
        .schedule(program, view, 0, (&limits, &mut counters, location))
        .unwrap();
    let (_relations, delta) = catalog.split(&limits, &mut counters, location).unwrap();
    assert!(
        registration
            .mark_ready(&delta, &limits, &mut counters, location)
            .unwrap()
    );
    registration
}

fn inventory<'registration, 'source>(
    registration: &'registration Registration<'source>,
    catalog: &SupportCatalog,
    location: ProgramSite,
) -> Inventory<'registration, 'source> {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let view = catalog
        .component_view(&limits, &mut counters, location)
        .unwrap()
        .unwrap();
    Inventory::prepare(
        registration,
        view,
        registration.bytes as u128 + catalog.bytes(location).unwrap() as u128,
        (&limits, &mut counters, location),
    )
    .unwrap()
    .unwrap()
}

fn widths(inventory: &Inventory<'_, '_>, location: ProgramSite) -> Vec<usize> {
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let mut result = Vec::new();
    while let Some(wave) = inventory
        .next(maximum(14), (&limits, &mut counters, location))
        .unwrap()
    {
        let selected = wave.producers.to_vec();
        result.push(selected.len());
        drop(wave);
        for producer in selected {
            assert_eq!(inventory.schedule.take_ready(), Some(producer));
        }
    }
    result
}

#[test]
fn independent_completed_inputs_share_one_ready_wave() {
    let mut owner = prepare(INDEPENDENT);
    let registration = ready(&owner.program, &mut owner.catalog, owner.location);
    let inventory = inventory(&registration, &owner.catalog, owner.location);
    assert_eq!(inventory.schedule.ready.borrow().len(), 2);
    assert_eq!(widths(&inventory, owner.location), [2]);
}

#[test]
fn a_shared_published_read_is_not_a_write_conflict() {
    let mut owner = prepare("a(N) :- N = #count{X : e(X)}. b(N) :- N = #count{X : e(X)}.");
    let registration = ready(&owner.program, &mut owner.catalog, owner.location);
    let inventory = inventory(&registration, &owner.catalog, owner.location);
    assert_eq!(widths(&inventory, owner.location), [2]);
}

#[test]
fn a_negative_ready_chain_stays_serial() {
    let mut owner = prepare("a :- not z. b :- not a. c :- not b.");
    let registration = ready(&owner.program, &mut owner.catalog, owner.location);
    let inventory = inventory(&registration, &owner.catalog, owner.location);
    assert_eq!(&*inventory.schedule.ready.borrow(), &[0, 1, 2]);
    // The ordinary certified resolver records only positive inputs. These
    // exact intersections establish that the extra negative reads survived.
    for pair in [(0, 1), (1, 2)] {
        let left = &inventory.footprints[pair.0];
        let right = &inventory.footprints[pair.1];
        assert!(
            inventory.components[left.writes.clone()]
                .iter()
                .any(|component| { inventory.components[right.reads.clone()].contains(component) })
        );
    }
    assert_eq!(widths(&inventory, owner.location), [1, 1, 1]);
}

#[test]
fn a_multihead_producer_occupies_one_queue_entry() {
    let mut owner = prepare("a(N); b(N) :- N = #count{X : e(X)}. c(N) :- N = #count{X : f(X)}.");
    let registration = ready(&owner.program, &mut owner.catalog, owner.location);
    let inventory = inventory(&registration, &owner.catalog, owner.location);
    let producer = registration
        .rules
        .iter()
        .position(|rule| rule.head.disjuncts().count() == 2)
        .unwrap();
    assert_eq!(
        super::super::rule_heads(&registration.rules[producer]).count(),
        2
    );
    assert_eq!(inventory.footprints[producer].writes.len(), 2);
    assert_eq!(inventory.schedule.ready.borrow().len(), 2);
    assert_eq!(widths(&inventory, owner.location), [2]);
}

#[test]
fn every_multihead_write_participates_in_exclusion() {
    let mut owner = prepare("a(N); b(N) :- N = #count{X : e(X)}. b(N) :- N = #count{X : f(X)}.");
    let registration = ready(&owner.program, &mut owner.catalog, owner.location);
    let inventory = inventory(&registration, &owner.catalog, owner.location);
    let producer = registration
        .rules
        .iter()
        .position(|rule| rule.head.disjuncts().count() == 2)
        .unwrap();
    assert_eq!(
        super::super::rule_heads(&registration.rules[producer]).count(),
        2
    );
    assert_eq!(inventory.footprints[producer].writes.len(), 2);
    assert_eq!(widths(&inventory, owner.location), [1, 1]);
}

#[test]
fn a_recursive_ready_producer_remains_serial() {
    let mut owner = prepare("a(X) :- a(X), not z. b(N) :- N = #count{X : f(X)}.");
    let registration = ready(&owner.program, &mut owner.catalog, owner.location);
    let inventory = inventory(&registration, &owner.catalog, owner.location);
    assert_eq!(inventory.schedule.ready.borrow().len(), 2);
    assert_eq!(widths(&inventory, owner.location), [1, 1]);
}

#[test]
fn a_missing_source_component_is_not_an_empty_read_set() {
    let mut owner = prepare(INDEPENDENT);
    let mut registration = ready(&owner.program, &mut owner.catalog, owner.location);
    let schedule = registration.schedule.as_mut().unwrap();
    let removed = schedule
        .nodes
        .iter_mut()
        .find(|node| node.signature.name.as_str() == "e")
        .unwrap();
    removed.component = None;
    let inventory = inventory(&registration, &owner.catalog, owner.location);
    assert!(!inventory.footprints[0].complete);
    assert_eq!(widths(&inventory, owner.location), [1, 1]);
}

#[test]
fn an_unpublished_prerequisite_stays_serial() {
    let mut owner = prepare(INDEPENDENT);
    let registration = ready(&owner.program, &mut owner.catalog, owner.location);
    let inventory = inventory(&registration, &owner.catalog, owner.location);
    let prerequisite = inventory.components[inventory.footprints[0].reads.start];
    inventory.schedule.reached.borrow_mut()[prerequisite] = false;
    assert_eq!(widths(&inventory, owner.location), [1, 1]);
}

#[test]
fn worker_width_limits_preserve_queue_coverage() {
    let mut owner = prepare(INDEPENDENT);
    let registration = ready(&owner.program, &mut owner.catalog, owner.location);
    let inventory = inventory(&registration, &owner.catalog, owner.location);
    let before = inventory.schedule.ready.borrow().clone();
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let wave = inventory
        .next(maximum(1), (&limits, &mut counters, owner.location))
        .unwrap()
        .unwrap();
    assert_eq!(&*wave.producers, &before[..1]);
    drop(wave);
    assert_eq!(inventory.schedule.taken.get(), 0);
    assert_eq!(widths(&inventory, owner.location), [2]);
}

#[test]
fn every_refused_wave_check_preserves_the_schedule() {
    let mut owner = prepare(INDEPENDENT);
    let registration = ready(&owner.program, &mut owner.catalog, owner.location);
    let inventory = inventory(&registration, &owner.catalog, owner.location);
    let defaults = FormulaLimits::default();
    let mut counters = Counters::default();
    let wave = inventory
        .next(maximum(14), (&defaults, &mut counters, owner.location))
        .unwrap()
        .unwrap();
    assert_eq!(wave.producers.len(), 2);
    drop(wave);
    let required = counters.accounting.work;
    let before = inventory.schedule.ready.borrow().clone();
    assert!(required > 3);
    for allowance in 0..required {
        let limits = FormulaLimits {
            max_work: allowance,
            ..defaults
        };
        let mut counters = Counters::default();
        let result = inventory.next(maximum(14), (&limits, &mut counters, owner.location));
        assert!(matches!(result, Err(FormulaFailure::Limit {
            resource: FormulaResource::Work, limit, ..
        }) if limit == u128::from(allowance)));
        assert!(counters.accounting.work <= allowance);
        assert_eq!(&*inventory.schedule.ready.borrow(), &before);
        assert_eq!(inventory.schedule.taken.get(), 0);
        assert!(
            before
                .iter()
                .all(|&producer| inventory.schedule.state[producer].get() == State::Ready)
        );
    }
    let limits = FormulaLimits {
        max_work: required,
        ..defaults
    };
    assert_eq!(
        inventory
            .next(
                maximum(14),
                (&limits, &mut Counters::default(), owner.location)
            )
            .unwrap()
            .unwrap()
            .producers
            .len(),
        2
    );
}

#[test]
fn cancellation_cannot_consume_a_ready_producer() {
    let mut owner = prepare(INDEPENDENT);
    let registration = ready(&owner.program, &mut owner.catalog, owner.location);
    let inventory = inventory(&registration, &owner.catalog, owner.location);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut counters = Counters::default().with_cancellation(Some(&cancellation));
    let result = inventory.next(
        maximum(14),
        (&FormulaLimits::default(), &mut counters, owner.location),
    );
    assert!(matches!(
        result,
        Err(FormulaFailure::Interrupted {
            reason: Stop::Cancelled,
            ..
        })
    ));
    assert_eq!(counters.accounting.work, 0);
    assert_eq!(inventory.schedule.taken.get(), 0);
}

#[test]
fn inventory_storage_counts_retained_ranges_and_components() {
    let mut owner = prepare(INDEPENDENT);
    let registration = ready(&owner.program, &mut owner.catalog, owner.location);
    let inventory = inventory(&registration, &owner.catalog, owner.location);
    assert_eq!(
        inventory.bytes,
        size_of::<Inventory<'_, '_>>()
            + inventory.footprints.capacity() * size_of::<Footprint>()
            + inventory.components.capacity() * size_of::<usize>()
    );
    let defaults = FormulaLimits::default();
    let mut counters = Counters::default();
    let view = owner
        .catalog
        .component_view(&defaults, &mut counters, owner.location)
        .unwrap()
        .unwrap();
    let limits = FormulaLimits {
        max_support_bytes: size_of::<Inventory<'_, '_>>() - 1,
        ..defaults
    };
    assert!(matches!(
        Inventory::prepare(
            &registration,
            view,
            0,
            (&limits, &mut counters, owner.location)
        ),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes,
            ..
        })
    ));
}

#[test]
fn missing_component_schedule_declines_without_a_wave() {
    let owner = prepare("a(X) :- e(X).");
    let limits = FormulaLimits::default();
    let mut counters = Counters::default();
    let view = owner
        .catalog
        .component_view(&limits, &mut counters, owner.location)
        .unwrap()
        .unwrap();
    let registration = Registration::prepare(
        owner.program.rules_without_facts(),
        Some(view),
        0,
        &limits,
        &mut counters,
        owner.location,
    )
    .unwrap();
    assert!(
        Inventory::prepare(
            &registration,
            view,
            0,
            (&limits, &mut counters, owner.location)
        )
        .unwrap()
        .is_none()
    );
}
