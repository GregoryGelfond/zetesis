//! Necessary region truth applied before binding and scalar evaluation.
//!
//! Rows still belong to complete possible support. This view only maps their
//! original occurrence positions to the admitted formula's dense IDs; it owns
//! no atom payload and does not establish a new support certificate. A source
//! row absent from the formula catalog remains eligible: scalar filters may
//! exclude it before it contributes any admitted atom occurrence.

use crate::ProgramSite;
use themelios_program::program::DefaultNegation;
use zetesis_core::{
    AtomLookup, AtomRow,
    catalog::Atoms,
    relation::{Failure, Row},
};
use zetesis_cpu::regions::Region;

use crate::formula_ir::RuleIr;
use crate::formula_support::{CompletedSupport, Counters, RowFilter};
use crate::{FormulaFailure, FormulaLimits};

/// Each kept source predicate's occurrence positions in the core's dense
/// catalog, in the order the core's streamed rows list those predicates. A
/// function of the immutable core alone: built once, by the first checker
/// that needs it, and shared by every checker of the core.
pub(super) struct RowPositions {
    predicates: Vec<Vec<Option<usize>>>,
    bytes: u128,
}

impl RowPositions {
    /// Map every source occurrence through `index`: one charged probe per
    /// kept support row. Each buffer is admitted against `support`'s
    /// workspace before it is filled; the caller retains the total.
    pub(super) fn prepare(
        support: &CompletedSupport<'_>,
        index: AtomLookup<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        let mut count = 0;
        for _ in support.source_atoms() {
            counters.work(limits, location)?;
            count += 1;
        }
        let mut predicates = Vec::new();
        let mut bytes = size_of::<Self>() as u128;
        reserve(
            &mut predicates,
            count,
            &mut bytes,
            support,
            limits,
            counters,
            location,
        )?;
        for (_, atoms) in support.source_atoms() {
            counters.work(limits, location)?;
            let mut positions = Vec::new();
            reserve(
                &mut positions,
                atoms.len(),
                &mut bytes,
                support,
                limits,
                counters,
                location,
            )?;
            for atom in atoms {
                let found = index.get_with(atom, || counters.work(limits, location))?;
                counters.work(limits, location)?;
                positions.push(found.map(AtomRow::position));
            }
            counters.work(limits, location)?;
            predicates.push(positions);
        }
        Ok(Self { predicates, bytes })
    }

    /// The header and every reserved buffer's capacity.
    pub(super) const fn retained_bytes(&self) -> u128 {
        self.bytes
    }
}

struct PredicateRows<'source> {
    /// The exact source occurrence mapping used by every row of this relation.
    atoms: Atoms<'source>,
    positions: &'source [Option<usize>],
}

/// A checker's integer correspondence: its own occurrence maps paired with
/// the core's shared positions. No region decisions are retained.
pub(super) struct SourceRows<'source> {
    predicates: Vec<PredicateRows<'source>>,
    bytes: u128,
}

impl<'source> SourceRows<'source> {
    /// Pair each kept predicate's occurrence map in `support` with the core's
    /// positions: O(predicates) work. The maps and the positions list the
    /// same predicates in the same order with the same lengths, since both
    /// come from the core's immutable streamed rows; a mismatch is refused.
    pub(super) fn attach(
        support: &CompletedSupport<'source>,
        positions: &'source RowPositions,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        let owner = || FormulaFailure::SupportRelation {
            error: Failure::Owner,
            location,
        };
        let mut count = 0;
        for _ in support.source_atoms() {
            counters.work(limits, location)?;
            count += 1;
        }
        if count != positions.predicates.len() {
            return Err(owner());
        }
        let mut predicates = Vec::new();
        let mut bytes = size_of::<Self>() as u128;
        reserve(
            &mut predicates,
            count,
            &mut bytes,
            support,
            limits,
            counters,
            location,
        )?;
        for ((_, atoms), positions) in support.source_atoms().zip(&positions.predicates) {
            counters.work(limits, location)?;
            if atoms.len() != positions.len() {
                return Err(owner());
            }
            predicates.push(PredicateRows { atoms, positions });
        }
        Ok(Self { predicates, bytes })
    }

    /// The header and the reserved pairing buffer's capacity, which the
    /// caller retains in its ledger; the shared positions are not included.
    pub(super) const fn retained_bytes(&self) -> u128 {
        self.bytes
    }
}

fn reserve<T>(
    values: &mut Vec<T>,
    count: usize,
    bytes: &mut u128,
    support: &CompletedSupport<'_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<(), FormulaFailure> {
    // Each vector is reserved once before it is filled; no growth overlap or
    // hidden tuple copies. Check allocator slack before retaining the buffer.
    support.admit_workspace(
        *bytes + count as u128 * size_of::<T>() as u128,
        limits,
        counters,
        location,
    )?;
    counters.work(limits, location)?;
    values
        .try_reserve_exact(count)
        .map_err(|_| FormulaFailure::SupportRelation {
            error: Failure::Allocation,
            location,
        })?;
    let actual = *bytes + values.capacity() as u128 * size_of::<T>() as u128;
    support.admit_workspace(actual, limits, counters, location)?;
    *bytes = actual;
    Ok(())
}

/// Borrowed region decisions over prepared source and formula identities.
pub(super) struct Selection<'a, 'source> {
    pub(super) rows: &'a SourceRows<'source>,
    pub(super) index: AtomLookup<'a, 'source>,
    pub(super) region: &'a Region,
}

impl Selection<'_, '_> {
    /// A necessary predicate-level condition, independent of bindings. Matching
    /// arguments and every scalar condition remain the ordinary join/body work.
    pub(super) fn possible(
        &self,
        rule: &RuleIr,
        support: &CompletedSupport<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<bool, FormulaFailure> {
        for literal in &rule.body {
            let Some((negation, pattern)) = super::literal_atom(literal) else {
                continue;
            };
            let components = support
                .components()
                .ok_or_else(|| crate::formula_support::components::missing(rule.location))?;
            let pattern = pattern.get(components, limits, counters, rule.location)?;
            let rows = self
                .index
                .predicate_with(pattern.predicate(), || counters.work(limits, rule.location))?;
            let mut possible = false;
            for row in rows {
                counters.work(limits, rule.location)?;
                possible = match negation {
                    DefaultNegation::Not => self.region.is_cut(row.position()),
                    DefaultNegation::None | DefaultNegation::NotNot => {
                        self.region.is_held(row.position())
                    }
                };
                if possible {
                    break;
                }
            }
            if !possible {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

impl RowFilter for Selection<'_, '_> {
    fn permits(
        &self,
        row: Row<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<bool, FormulaFailure> {
        // A row position is meaningful only in its original occurrence map.
        // Equal atom or predicate contents cannot establish that correspondence.
        for source in &self.rows.predicates {
            counters.work(limits, location)?;
            if let Some(occurrence) = row.occurrence_in(source.atoms) {
                counters.work(limits, location)?;
                let position =
                    source
                        .positions
                        .get(occurrence)
                        .ok_or(FormulaFailure::SupportRelation {
                            error: Failure::Owner,
                            location,
                        })?;
                return Ok(position.is_none_or(|position| self.region.is_held(position)));
            }
        }
        Err(FormulaFailure::SupportRelation {
            error: Failure::Owner,
            location,
        })
    }
}

#[cfg(test)]
mod tests {
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
            rows: &rows,
            index: index.lookup(),
            region: &region,
        };
        let mut visited = 0;
        for (predicate, atoms) in prepared.completed.source_atoms() {
            let relation = Relation::from_refs(predicate, atoms, Limits::default()).unwrap();
            for position in 0..atoms.len() {
                assert!(
                    selection
                        .permits(
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
            zetesis_core::AtomIndex::from_catalog_with(atoms, || Ok::<_, FormulaFailure>(()))
                .unwrap();
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
                rows: &rows,
                index: index.lookup(),
                region: &region,
            };
            let mut has_source_row = false;
            for (predicate, source) in prepared.completed.source_atoms() {
                let relation = Relation::from_refs(predicate, source, Limits::default()).unwrap();
                for (position, atom) in source.iter().enumerate() {
                    let permitted = selection
                        .permits(
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
}
