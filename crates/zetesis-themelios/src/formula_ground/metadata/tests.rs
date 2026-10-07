use proptest::prelude::*;
use themelios_base::source::SourceId;
use themelios_base::span::{ByteOffset, Span};

use super::*;
use crate::{ExpansionFailure, ExpansionLimits, FormulaResource};

fn location(source: u32, offset: u32) -> ProgramSite {
    ProgramSite::source(themelios_base::span::Location {
        source: SourceId::new(source),
        span: Span::empty(ByteOffset::new(offset)),
    })
}

fn prepared() -> (Metadata, Counters, FormulaLimits) {
    let mut metadata = Metadata::default();
    let mut counters = Counters::default();
    let limits = FormulaLimits::default();
    for source in 0..3 {
        metadata
            .atom(location(source, 0), &mut counters, &limits)
            .unwrap();
    }
    (metadata, counters, limits)
}

proptest! {
    #[test]
    fn shared_chains_preserve_each_atoms_events(events in prop::collection::vec((0_usize..3, 0_usize..30, 0_u32..4, 0_u32..20), 0..100)) {
        let (mut metadata, mut counters, limits) = prepared();
        let mut budget = Budget::new(ExpansionLimits::default(), 0);
        let mut producers = [Vec::new(), Vec::new(), Vec::new()];
        let mut origins: [Vec<ProgramSite>; 3] = std::array::from_fn(|atom| vec![location(u32::try_from(atom).unwrap(), 0)]);
        for (atom, antecedent, source, offset) in events {
            let origin = location(source, offset);
            metadata.producer(atom, antecedent, &mut counters, &limits, origin).unwrap();
            metadata.origin(atom, origin, &mut budget, &mut counters, &limits).unwrap();
            producers[atom].push(antecedent);
            origins[atom].push(origin);
            origins[atom].sort_unstable();
            origins[atom].dedup();
            for index in 0..3 {
                prop_assert_eq!(metadata.producers(index).collect::<Vec<_>>(), producers[index].clone());
                prop_assert_eq!(metadata.origins(index).collect::<Vec<_>>(), origins[index].clone());
                prop_assert_eq!(metadata.location(index), location(u32::try_from(index).unwrap(), 0));
            }
        }
    }
}

#[test]
fn iterator_length_tracks_its_unread_entries() {
    let (mut metadata, mut counters, limits) = prepared();
    for atom in [1, 0, 1, 2, 1] {
        metadata
            .producer(atom, atom + 7, &mut counters, &limits, location(0, 0))
            .unwrap();
    }
    let mut values = metadata.producers(1);
    for remaining in (1..=3).rev() {
        assert_eq!(values.len(), remaining);
        assert_eq!(values.next(), Some(8));
    }
    assert_eq!(values.len(), 0);
    assert_eq!(values.next(), None);
    assert_eq!(values.next(), None);
}

#[test]
fn duplicate_origin_needs_no_new_origin_allowance() {
    let (mut metadata, mut counters, limits) = prepared();
    let mut budget = Budget::new(
        ExpansionLimits {
            max_origin_locations: 0,
            ..ExpansionLimits::default()
        },
        0,
    );
    metadata
        .origin(1, location(1, 0), &mut budget, &mut counters, &limits)
        .unwrap();
    assert_eq!(metadata.origins(1).collect::<Vec<_>>(), [location(1, 0)]);
}

#[test]
fn duplicate_lookup_obeys_the_work_ceiling() {
    let (mut metadata, mut counters, mut limits) = prepared();
    let mut budget = Budget::new(
        ExpansionLimits {
            max_origin_locations: 0,
            ..ExpansionLimits::default()
        },
        0,
    );
    limits.max_work = counters.accounting.work;
    assert!(matches!(
        metadata.origin(1, location(1, 0), &mut budget, &mut counters, &limits),
        Err(FormulaFailure::Limit {
            resource: FormulaResource::Work,
            ..
        })
    ));
    assert_eq!(metadata.origins(1).collect::<Vec<_>>(), [location(1, 0)]);
}

#[test]
fn failed_metadata_records_the_published_atom() {
    // Sweep the actual small operation's finite work prefix. Interner lookup,
    // planning and copy admission precede metadata, so a literal one-tick
    // allowance no longer identifies the metadata failure boundary.
    let (complete_work, _) = metadata_publication_attempt(None);
    assert!((0..complete_work).any(|work| metadata_publication_attempt(Some(work)).1));
}

#[derive(Default)]
struct MetadataObserver(
    std::cell::RefCell<Option<(crate::GroundingOutcome, crate::GroundingWork)>>,
);
impl crate::GroundingObserver for MetadataObserver {
    fn enter(&self) {}
    fn exit(&self) {}
    fn details_enabled(&self) -> bool {
        true
    }
    fn phase_exit(
        &self,
        _phase: crate::GroundingPhase,
        _location: Option<ProgramSite>,
        outcome: crate::GroundingOutcome,
        work: crate::GroundingWork,
    ) {
        *self.0.borrow_mut() = Some((outcome, work));
    }
}

fn atom(name: &str) -> zetesis_core::Atom {
    zetesis_core::Atom::new(zetesis_core::Predicate::new(name, 0).unwrap(), vec![]).unwrap()
}

/// Reach a real metadata growth boundary without assuming Vec's growth policy.
fn fill_metadata_capacity(builder: &mut crate::formula_ground::Builder<'_, '_, '_>) {
    while builder.metadata.atoms.is_empty()
        || (builder.metadata.atoms.len() < builder.metadata.atoms.capacity()
            && builder.metadata.origins.len() < builder.metadata.origins.capacity())
    {
        let name = format!("p{}", builder.metadata.atoms.len());
        builder
            .atom_ref((&atom(&name)).into(), location(0, 0))
            .unwrap();
    }
}

fn metadata_publication_attempt(additional: Option<u64>) -> (u64, bool) {
    use crate::formula_ground::{Builder, Purpose};
    use crate::grounding_observer::Profile;
    use crate::{GroundingOutcome, GroundingPhase};

    let observer = MetadataObserver::default();
    let profile = Profile::new(Some(&observer));
    let limits = FormulaLimits::default();
    let mut budget = Budget::new(ExpansionLimits::default(), 0);
    let mut counters = Counters::resume(
        crate::formula_support::Accounting::default(),
        profile.work(),
    );
    let mut source = crate::formula_support::SupportCatalog::default();
    let (relations, mut append) = source
        .split(&limits, &mut counters, location(0, 0))
        .unwrap();
    let support =
        crate::formula_support::Support::indexed(&relations, &limits, &counters, location(0, 0))
            .unwrap();
    let mut computation = crate::formula_support::Computation::new(&mut append, &support);
    let mut builder = Builder::empty(
        &mut computation,
        &limits,
        &mut budget,
        &mut counters,
        Purpose::Theory,
        None,
        location(0, 0),
    )
    .unwrap();
    builder.initialize(location(0, 0)).unwrap();
    fill_metadata_capacity(&mut builder);
    let before = builder.metadata.atoms.len();
    let origins_before = builder.metadata.origins.len();
    let nodes_before = builder.nodes.view().len();
    assert_eq!(builder.catalog.len(), before);
    let relocation = growth(&builder.metadata.atoms) + growth(&builder.metadata.origins);
    assert!(relocation > 0);
    let started = builder.counters.accounting.work;
    let limited = FormulaLimits {
        max_work: additional.map_or(limits.max_work, |work| started.checked_add(work).unwrap()),
        ..limits
    };
    builder.limits = &limited;
    let result = profile.phase(GroundingPhase::RuleInstantiation, None, || {
        builder.atom_ref((&atom("e")).into(), location(0, 0))
    });
    let spent = builder.counters.accounting.work - started;
    if additional.is_none() {
        result.unwrap();
        assert_eq!(builder.catalog.len(), before + 1);
        assert_eq!(builder.metadata.atoms.len(), before + 1);
        return (spent, false);
    }
    // Replays inspect their own actual capacities; a completed shorter path is
    // not evidence of the required metadata refusal.
    let Err(error) = result else {
        return (spent, false);
    };
    assert!(matches!(
        error,
        FormulaFailure::Limit {
            resource: FormulaResource::Work,
            ..
        }
    ));
    if builder.catalog.len() == before || builder.metadata.atoms.len() != before {
        return (spent, false);
    }
    assert!(matches!(error, FormulaFailure::Limit {
        resource: FormulaResource::Work, observed, limit, location: found,
    } if observed == u128::from(builder.counters.accounting.work) + relocation
        && limit == u128::from(limited.max_work) && found == location(0, 0)));
    let (outcome, work) = observer.0.borrow().unwrap();
    assert_eq!(outcome, GroundingOutcome::Failed);
    assert_eq!(work.atoms_inserted, Some(1));
    assert_eq!(builder.catalog.len(), before + 1);
    assert_eq!(
        builder
            .catalog
            .get(
                before,
                builder.computation,
                &mut Counters::default(),
                &limits,
                location(0, 0)
            )
            .unwrap(),
        zetesis_core::catalog::AtomRef::from(&atom("e"))
    );
    assert_eq!(builder.metadata.atoms.len(), before);
    assert_eq!(builder.metadata.origins.len(), origins_before);
    assert_eq!(builder.nodes.view().len(), nodes_before);
    assert!(builder.roots.is_empty());
    assert!(builder.origins.is_empty());
    (spent, true)
}

#[test]
fn origin_allowance_precedes_new_entry_work() {
    let (mut metadata, mut counters, mut limits) = prepared();
    limits.max_work = counters.accounting.work;
    let mut budget = Budget::new(
        ExpansionLimits {
            max_origin_locations: 0,
            ..ExpansionLimits::default()
        },
        0,
    );
    let new = location(9, 2);
    assert!(
        matches!(metadata.origin(1, new, &mut budget, &mut counters, &limits), Err(FormulaFailure::Expansion(ExpansionFailure::Limit { resource: ExpansionResource::Origins, location: found, .. })) if found == new)
    );
    assert_eq!(metadata.origins(1).collect::<Vec<_>>(), [location(1, 0)]);
}

#[test]
fn origin_work_refusal_preserves_the_chain() {
    let (mut metadata, mut counters, mut limits) = prepared();
    let mut budget = Budget::new(ExpansionLimits::default(), 0);
    let new = location(9, 2);
    limits.max_work = counters.accounting.work + 1;
    assert!(
        matches!(metadata.origin(1, new, &mut budget, &mut counters, &limits), Err(FormulaFailure::Limit { resource: FormulaResource::Work, location: found, .. }) if found == new)
    );
    assert_eq!(metadata.origins(1).collect::<Vec<_>>(), [location(1, 0)]);
    limits.max_work += 1;
    metadata
        .origin(1, new, &mut budget, &mut counters, &limits)
        .unwrap();
    assert_eq!(
        metadata.origins(1).collect::<Vec<_>>(),
        [location(1, 0), new]
    );
}

#[test]
fn origin_copy_charges_traversal_and_payload() {
    let (metadata, mut counters, mut limits) = prepared();
    let initial = counters.accounting.work;
    limits.max_work = initial + 1;
    assert!(
        matches!(metadata.copy_origins(1, &mut counters, &limits), Err(FormulaFailure::Limit { resource: FormulaResource::Work, observed, limit, .. }) if observed == u128::from(initial + 2) && limit == u128::from(initial + 1))
    );
    limits.max_work += 1;
    assert_eq!(
        metadata.copy_origins(1, &mut counters, &limits).unwrap(),
        [location(1, 0)]
    );
    assert_eq!(counters.accounting.work, initial + 2);
}

#[test]
fn failed_capacity_reservation_is_located() {
    let origin = location(3, 4);
    let mut values = Vec::<ProgramSite>::new();
    let failure = reserve(&mut values, usize::MAX, origin).unwrap_err();
    assert!(
        matches!(failure, FormulaFailure::MetadataAllocation { location: found, .. } if found == origin)
    );
    assert_eq!(
        failure.diagnostics()[0].primary().location,
        origin.location().unwrap()
    );
    assert!(std::error::Error::source(&failure).is_some());
    assert!(failure.to_string().starts_with("formula metadata storage:"));
    assert_eq!(values.capacity(), 0);
}
