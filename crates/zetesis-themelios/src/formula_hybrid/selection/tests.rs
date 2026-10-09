mod evidence;
mod predicates;
use super::*;
use crate::{
    AdmissionOptions, ConstraintCheckCause, ConstraintCheckLimits, ExpansionLimits,
    FormulaResource, HybridFormula, prepare_formula,
};
use zetesis_core::relation::{Limits, Relation};

fn owner() -> HybridFormula {
    owner_of("{p(1);p(2);-p(1)}. :-p(X),not -p(X).")
}

fn owner_of(source: &str) -> HybridFormula {
    prepare_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
    .ground_hybrid()
    .unwrap()
}

fn workspace_bytes(prepared: &super::super::PreparedConstraints<'_>) -> u128 {
    let limits = FormulaLimits {
        max_support_bytes: 0,
        ..prepared.limits
    };
    match prepared.completed.admit_workspace(
        0,
        &limits,
        &Counters::default(),
        prepared.source.location,
    ) {
        Err(FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes,
            observed,
            limit: 0,
            ..
        }) => observed,
        other => panic!("expected a typed support-capacity receipt, got {other:?}"),
    }
}

#[test]
fn refused_row_map_preserves_retained_capacity_on_retry() {
    let owner = owner();
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let prepared = checker.prepared.as_mut().unwrap();
    let mut counters = Counters::default();
    prepared.prepare_index(owner.core(), &mut counters).unwrap();
    let original_limit = prepared.limits.max_support_bytes;
    let retained = workspace_bytes(prepared);
    // Admit the outer descriptors, then refuse an inner row-ID buffer.
    // The index must survive; temporary map capacity must not be retained.
    prepared.limits.max_support_bytes = usize::try_from(retained).unwrap()
        + size_of::<RowPositions>()
        + prepared.completed.source_atoms().count() * size_of::<Vec<Option<usize>>>();
    let mut previous = None;
    for _ in 0..2 {
        let before = counters.accounting.work;
        let cause = prepared
            .prepare_selection(owner.core(), &mut counters)
            .unwrap_err();
        let ConstraintCheckCause::Source(error) = cause else {
            panic!("expected source refusal")
        };
        let FormulaFailure::Limit {
            resource: FormulaResource::SupportBytes,
            observed,
            limit,
            ..
        } = *error
        else {
            panic!("expected support-byte refusal")
        };
        assert_eq!(limit, prepared.limits.max_support_bytes as u128);
        assert!(observed > limit);
        if let Some(previous) = previous {
            assert_eq!(observed, previous);
        }
        previous = Some(observed);
        assert!(
            counters.accounting.work > before,
            "failed preparation retains accepted work"
        );
        assert!(prepared.index.is_some());
        assert!(prepared.rows.is_none());
        assert_eq!(workspace_bytes(prepared), retained);
    }
    // Only this private test changes the byte ceiling; public checker
    // limits remain fixed. The larger allowance permits a complete retry.
    prepared.limits.max_support_bytes = original_limit;
    prepared
        .prepare_selection(owner.core(), &mut counters)
        .unwrap();
    let rows = prepared.rows.as_ref().unwrap();
    let map_bytes = size_of::<SourceRows<'_>>() as u128
        + rows.predicates.capacity() as u128 * size_of::<PredicateRows<'_>>() as u128
        + owner.core().0.rows.get().unwrap().retained_bytes();
    assert_eq!(workspace_bytes(prepared), retained + map_bytes);
    let work = counters.accounting.work;
    prepared
        .prepare_selection(owner.core(), &mut counters)
        .unwrap();
    assert_eq!(counters.accounting.work, work);
    assert_eq!(workspace_bytes(prepared), retained + map_bytes);
}

#[test]
fn unmapped_rows_remain_eligible() {
    let owner = owner();
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let prepared = checker.prepared.as_mut().unwrap();
    let mut counters = Counters::default();
    // A pre-match support row need not have survived the scalar guards
    // that contributed occurrences to the admitted formula catalog.
    let index = zetesis_core::AtomIndex::new_with(&[], || Ok::<_, FormulaFailure>(())).unwrap();
    let positions = RowPositions::prepare(
        &prepared.completed,
        index.lookup(),
        &prepared.limits,
        &mut counters,
        prepared.source.location,
    )
    .unwrap();
    let rows = SourceRows::attach(
        &prepared.completed,
        &positions,
        &prepared.limits,
        &mut counters,
        prepared.source.location,
    )
    .unwrap();
    let region = Region::all_open(0);
    let selection = Selection {
        predicates: None,
        rows: &rows,
        index: index.lookup(),
        region: &region,
    };
    let mut visited = 0;
    for (predicate, atoms) in prepared.completed.source_atoms() {
        let relation = Relation::from_refs(predicate, atoms, Limits::default()).unwrap();
        let source = selection
            .resolve(
                atoms,
                &prepared.limits,
                &mut counters,
                prepared.source.location,
            )
            .unwrap();
        for position in 0..atoms.len() {
            assert!(
                selection
                    .permits(
                        source,
                        relation.row(position).unwrap(),
                        &prepared.limits,
                        &mut counters,
                        prepared.source.location
                    )
                    .unwrap()
            );
            visited += 1;
        }
    }
    assert!(visited > 0);
}

#[test]
fn row_selection_preserves_unsorted_dense_ids() {
    let owner = owner();
    let mut catalog_owner = zetesis_core::atom_interner::AtomInterner::new();
    let catalog_limits = zetesis_core::atom_interner::Limits {
        max_atoms: owner.atom_catalog().atoms().len(),
        max_bytes: 16 * 1024 * 1024,
    };
    let mut positions = Vec::new();
    for atom in owner.atom_catalog().atoms() {
        let position = catalog_owner
            .entry_atom_with(atom, catalog_limits, || Ok::<_, FormulaFailure>(()))
            .unwrap()
            .insert_with(catalog_limits, || Ok::<_, FormulaFailure>(()))
            .unwrap();
        positions.push(position);
    }
    positions.sort_unstable_by(|&a, &b| {
        catalog_owner
            .get(b)
            .unwrap()
            .cmp(&catalog_owner.get(a).unwrap())
    });
    // The independent catalog publishes real occurrence IDs in descending
    // semantic order; the index must recover those IDs after its own sort.
    let catalog = catalog_owner
        .publish_selection_with(&positions, catalog_limits, || Ok::<_, FormulaFailure>(()))
        .unwrap();
    let atoms = catalog.atoms();
    assert!(atoms.len() > 1);
    assert!(atoms.iter().zip(atoms.iter().skip(1)).all(|(a, b)| a > b));
    let index =
        zetesis_core::AtomIndex::from_catalog_with(atoms, || Ok::<_, FormulaFailure>(())).unwrap();
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let prepared = checker.prepared.as_mut().unwrap();
    let mut counters = Counters::default();
    let positions = RowPositions::prepare(
        &prepared.completed,
        index.lookup(),
        &prepared.limits,
        &mut counters,
        prepared.source.location,
    )
    .unwrap();
    let rows = SourceRows::attach(
        &prepared.completed,
        &positions,
        &prepared.limits,
        &mut counters,
        prepared.source.location,
    )
    .unwrap();
    let mut saw_supported = false;
    let mut saw_unsupported = false;
    for held in 0..atoms.len() {
        let mut region = Region::all_open(atoms.len());
        assert!(region.hold(held));
        let selection = Selection {
            predicates: None,
            rows: &rows,
            index: index.lookup(),
            region: &region,
        };
        let mut has_source_row = false;
        for (predicate, source) in prepared.completed.source_atoms() {
            let relation = Relation::from_refs(predicate, source, Limits::default()).unwrap();
            let resolved = selection
                .resolve(
                    source,
                    &prepared.limits,
                    &mut counters,
                    prepared.source.location,
                )
                .unwrap();
            for (position, atom) in source.iter().enumerate() {
                let permitted = selection
                    .permits(
                        resolved,
                        relation.row(position).unwrap(),
                        &prepared.limits,
                        &mut counters,
                        prepared.source.location,
                    )
                    .unwrap();
                let is_held_atom = atom == atoms.at(held).unwrap();
                assert_eq!(permitted, is_held_atom);
                has_source_row |= is_held_atom;
            }
        }
        saw_supported |= has_source_row;
        saw_unsupported |= !has_source_row;
    }
    // The negative occurrence -p(2) is admitted by the constraint, but no
    // source rule supports it. Its dense ID must not select another row.
    assert!(saw_supported);
    assert!(saw_unsupported);
}

/// The work one checker charges to reach the core's index.
fn index_charge(owner: &HybridFormula) -> (u64, *const zetesis_core::CatalogIndex) {
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let prepared = checker.prepared.as_mut().unwrap();
    let mut counters = Counters::default();
    prepared.prepare_index(owner.core(), &mut counters).unwrap();
    (
        counters.accounting.work,
        std::ptr::from_ref(prepared.index.unwrap()),
    )
}

#[test]
fn a_later_checker_borrows_the_core_index_for_one_unit() {
    // Two cores of different sizes: the first checker's build grows with
    // the atom count, every later checker's borrow does not.
    for source in [
        "{p(1..2)}. :-p(X),p(Y),X<Y.",
        "{p(1..200)}. :-p(X),p(Y),X+Y=7,X<Y.",
    ] {
        let owner = owner_of(source);
        let (built, first) = index_charge(&owner);
        let (borrowed, second) = index_charge(&owner);
        assert!(built > 1, "{source}");
        assert_eq!(borrowed, 1, "{source}");
        assert!(std::ptr::eq(first, second), "{source}");
    }
}

#[test]
fn a_borrowing_checker_counts_the_index_in_its_ledger() {
    let owner = owner();
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let prepared = checker.prepared.as_mut().unwrap();
    let before = workspace_bytes(prepared);
    index_charge(&owner);
    prepared
        .prepare_index(owner.core(), &mut Counters::default())
        .unwrap();
    assert_eq!(
        workspace_bytes(prepared),
        before + prepared.index.unwrap().retained_bytes()
    );
}

#[test]
fn checkers_racing_on_first_use_share_one_published_index() {
    let owner = owner_of("{p(1..50)}. :-p(X),p(Y),X+Y=7,X<Y.");
    let indexes: Vec<usize> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..8)
            .map(|_| scope.spawn(|| index_charge(&owner).1 as usize))
            .collect();
        workers.into_iter().map(|w| w.join().unwrap()).collect()
    });
    assert!(indexes.iter().all(|&index| index == indexes[0]));
}

#[test]
fn a_refused_index_build_publishes_nothing() {
    let owner = owner();
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let prepared = checker.prepared.as_mut().unwrap();
    prepared.limits.max_support_bytes = 0;
    assert!(
        prepared
            .prepare_index(owner.core(), &mut Counters::default())
            .is_err()
    );
    assert!(prepared.index.is_none());
    assert!(owner.core().0.index.get().is_none());
}

/// The work one checker charges to map the core's source rows, after its
/// index is in place.
fn rows_charge(owner: &HybridFormula) -> u64 {
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let prepared = checker.prepared.as_mut().unwrap();
    let mut counters = Counters::default();
    prepared.prepare_index(owner.core(), &mut counters).unwrap();
    let before = counters.accounting.work;
    prepared
        .prepare_selection(owner.core(), &mut counters)
        .unwrap();
    counters.accounting.work - before
}

#[test]
fn a_later_checker_borrows_the_core_row_positions() {
    // One source predicate read in either core; the larger has 100 times
    // the rows. A later checker pays per predicate, not per row.
    let small = owner_of("{p(1..2)}. :-p(X),p(Y),X<Y.");
    let large = owner_of("{p(1..200)}. :-p(X),p(Y),X+Y=7,X<Y.");
    let built = (rows_charge(&small), rows_charge(&large));
    let borrowed = (rows_charge(&small), rows_charge(&large));
    assert!(built.1 > borrowed.1, "a build maps every row");
    assert_eq!(borrowed.0, borrowed.1);
}

#[test]
fn resolved_rows_keep_original_occurrence_positions() {
    let owner = owner();
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let prepared = checker.prepared.as_mut().unwrap();
    let mut counters = Counters::default();
    prepared
        .prepare_selection(owner.core(), &mut counters)
        .unwrap();
    let rows = prepared.rows.as_ref().unwrap();
    let source = rows
        .predicates
        .iter()
        .find(|source| source.atoms.len() > 1)
        .unwrap();
    let atoms = source.atoms;
    let predicate = atoms.at(0).unwrap().predicate();
    let order = [1, 0, 1];
    let relation =
        Relation::from_catalog_refs(predicate, atoms, &order, Limits::default()).unwrap();
    let mut region = Region::all_open(owner.atom_catalog().atoms().len());
    assert!(region.hold(source.positions[0].unwrap()));
    let selection = Selection {
        predicates: None,
        rows,
        index: prepared.index.unwrap().lookup(),
        region: &region,
    };
    let source = selection
        .resolve(
            atoms,
            &prepared.limits,
            &mut counters,
            prepared.source.location,
        )
        .unwrap();
    let before = counters.accounting.work;
    let selected: Vec<_> = (0..order.len())
        .map(|position| {
            let row = relation.row(position).unwrap();
            assert_eq!(row.source_index(), order[position]);
            selection
                .permits(
                    source,
                    row,
                    &prepared.limits,
                    &mut counters,
                    prepared.source.location,
                )
                .unwrap()
        })
        .collect();
    assert_eq!(selected, [false, true, false]);
    assert_eq!(counters.accounting.work - before, order.len() as u64);
}

#[test]
fn equal_foreign_occurrences_refuse_resolution() {
    let owner = owner();
    let other = self::owner();
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let prepared = checker.prepared.as_mut().unwrap();
    let mut counters = Counters::default();
    prepared
        .prepare_selection(owner.core(), &mut counters)
        .unwrap();
    let mut foreign = other.checker(ConstraintCheckLimits::default()).unwrap();
    let foreign = foreign.prepared.as_mut().unwrap();
    let (_, atoms) = foreign.completed.source_atoms().next().unwrap();
    let rows = prepared.rows.as_ref().unwrap();
    assert!(
        rows.predicates
            .iter()
            .any(|source| source.atoms.iter().eq(atoms.iter()))
    );
    let region = Region::all_open(owner.atom_catalog().atoms().len());
    let selection = Selection {
        predicates: None,
        rows,
        index: prepared.index.unwrap().lookup(),
        region: &region,
    };
    let before = counters.accounting.work;
    assert!(matches!(
        selection.resolve(
            atoms,
            &prepared.limits,
            &mut counters,
            prepared.source.location
        ),
        Err(FormulaFailure::SupportRelation {
            error: Failure::Owner,
            ..
        })
    ));
    assert_eq!(
        counters.accounting.work - before,
        rows.predicates.len() as u64
    );
}

#[test]
fn source_resolution_retains_refused_work_prefixes() {
    let owner = owner();
    let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
    let prepared = checker.prepared.as_mut().unwrap();
    prepared
        .prepare_selection(owner.core(), &mut Counters::default())
        .unwrap();
    let rows = prepared.rows.as_ref().unwrap();
    let last = rows.predicates.len() - 1;
    let atoms = rows.predicates[last].atoms;
    let region = Region::all_open(owner.atom_catalog().atoms().len());
    let selection = Selection {
        predicates: None,
        rows,
        index: prepared.index.unwrap().lookup(),
        region: &region,
    };
    for quota in 0..=last {
        let mut counters = Counters::default();
        let limits = FormulaLimits {
            max_work: quota as u64,
            ..prepared.limits
        };
        assert!(matches!(
            selection.resolve(atoms, &limits, &mut counters, prepared.source.location),
            Err(FormulaFailure::Limit { resource: FormulaResource::Work, observed, limit, .. })
                if observed == quota as u128 + 1 && limit == quota as u128
        ));
        assert_eq!(counters.accounting.work, quota as u64);
    }
    let mut counters = Counters::default();
    let limits = FormulaLimits {
        max_work: (last + 1) as u64,
        ..prepared.limits
    };
    assert_eq!(
        selection
            .resolve(atoms, &limits, &mut counters, prepared.source.location)
            .unwrap(),
        last
    );
    assert_eq!(counters.accounting.work, (last + 1) as u64);
    let cancellation = zetesis_cpu::Cancellation::default();
    cancellation.cancel();
    let mut counters = Counters::default().with_cancellation(Some(&cancellation));
    assert!(matches!(
        selection.resolve(atoms, &limits, &mut counters, prepared.source.location),
        Err(FormulaFailure::Interrupted {
            reason: zetesis_cpu::Stop::Cancelled,
            ..
        })
    ));
    assert_eq!(counters.accounting.work, 0);
}
