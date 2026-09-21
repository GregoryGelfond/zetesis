//! Necessary region truth applied before binding and scalar evaluation.
//!
//! Rows still belong to complete possible support. This view only maps their
//! original occurrence positions to the admitted formula's dense IDs; it owns
//! no atom payload and does not establish a new support certificate. A source
//! row absent from the formula catalog remains eligible: scalar filters may
//! exclude it before it contributes any admitted atom occurrence.

use themelios_base::span::Location;
use themelios_program::program::DefaultNegation;
use zetesis_core::{
    AtomIndex, AtomRow, Predicate,
    relation::{Failure, Row},
};
use zetesis_cpu::regions::Region;

use crate::formula_ir::RuleIr;
use crate::formula_support::{CompletedSupport, Counters, RowFilter};
use crate::{FormulaFailure, FormulaLimits};

struct PredicateRows<'source> {
    /// The exact catalog predicate borrowed by every row of this relation.
    predicate: &'source Predicate,
    positions: Vec<Option<usize>>,
}

/// Prepared integer correspondence; no region decisions are retained.
pub(super) struct SourceRows<'source> {
    predicates: Vec<PredicateRows<'source>>,
}

impl<'source> SourceRows<'source> {
    pub(super) fn prepare(
        support: &mut CompletedSupport<'source>,
        index: &AtomIndex<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
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
        for (predicate, atoms) in support.source_atoms() {
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
                let found = index
                    .lookup()
                    .get_with(atom, || counters.work(limits, location))?;
                counters.work(limits, location)?;
                positions.push(found.map(AtomRow::position));
            }
            counters.work(limits, location)?;
            predicates.push(PredicateRows {
                predicate,
                positions,
            });
        }
        // Every reservation checked its actual capacity before any publication.
        support.retain_workspace(usize::try_from(bytes).expect("admitted support bytes fit usize"));
        Ok(Self { predicates })
    }
}

fn reserve<T>(
    values: &mut Vec<T>,
    count: usize,
    bytes: &mut u128,
    support: &CompletedSupport<'_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: Location,
) -> Result<(), FormulaFailure> {
    // Each vector is reserved once before it is filled; no growth overlap or
    // hidden tuple copies. Check allocator slack before retaining the buffer.
    support.admit_workspace(
        *bytes + count as u128 * size_of::<T>() as u128,
        limits,
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
    support.admit_workspace(actual, limits, location)?;
    *bytes = actual;
    Ok(())
}

/// Borrowed region decisions over prepared source and formula identities.
pub(super) struct Selection<'a, 'source> {
    pub(super) rows: &'a SourceRows<'source>,
    pub(super) index: &'a AtomIndex<'source>,
    pub(super) region: &'a Region,
}

impl Selection<'_, '_> {
    /// A necessary predicate-level condition, independent of bindings. Matching
    /// arguments and every scalar condition remain the ordinary join/body work.
    pub(super) fn possible(
        &self,
        rule: &RuleIr,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<bool, FormulaFailure> {
        for literal in &rule.body {
            let Some((negation, pattern)) = super::literal_atom(literal) else {
                continue;
            };
            let rows = self
                .index
                .lookup()
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
        location: Location,
    ) -> Result<bool, FormulaFailure> {
        // SourceRows and Join borrow the same completed catalogs. Pointer
        // identity names a relation here, not equality between ASP predicates.
        // The descriptor scan is bounded by that owner's predicate count and
        // charges before each comparison. It does not inspect atom/name payload.
        for source in &self.rows.predicates {
            counters.work(limits, location)?;
            if std::ptr::eq(source.predicate, row.predicate()) {
                counters.work(limits, location)?;
                let position = source.positions.get(row.position()).ok_or(
                    FormulaFailure::SupportRelation {
                        error: Failure::Owner,
                        location,
                    },
                )?;
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
        prepare_formula(
            "{p(1);p(2);-p(1)}. :-p(X),not -p(X).".into(),
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
        match prepared
            .completed
            .admit_workspace(0, &limits, prepared.source.location)
        {
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
        prepared
            .prepare_index(owner.atom_catalog(), &mut counters)
            .unwrap();
        let original_limit = prepared.limits.max_support_bytes;
        let retained = workspace_bytes(prepared);
        // Admit the outer descriptors, then refuse an inner row-ID buffer.
        // The index must survive; temporary map capacity must not be retained.
        prepared.limits.max_support_bytes = usize::try_from(retained).unwrap()
            + size_of::<SourceRows<'_>>()
            + prepared.completed.source_atoms().count() * size_of::<PredicateRows<'_>>();
        let mut previous = None;
        for _ in 0..2 {
            let before = counters.work;
            let cause = prepared
                .prepare_selection(owner.atom_catalog(), &mut counters)
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
                counters.work > before,
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
            .prepare_selection(owner.atom_catalog(), &mut counters)
            .unwrap();
        let rows = prepared.rows.as_ref().unwrap();
        let map_bytes = size_of::<SourceRows<'_>>() as u128
            + rows.predicates.capacity() as u128 * size_of::<PredicateRows<'_>>() as u128
            + rows
                .predicates
                .iter()
                .map(|p| p.positions.capacity() as u128 * size_of::<Option<usize>>() as u128)
                .sum::<u128>();
        assert_eq!(workspace_bytes(prepared), retained + map_bytes);
        let work = counters.work;
        prepared
            .prepare_selection(owner.atom_catalog(), &mut counters)
            .unwrap();
        assert_eq!(counters.work, work);
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
        let index = AtomIndex::new_with(&[], || Ok::<_, FormulaFailure>(())).unwrap();
        let rows = SourceRows::prepare(
            &mut prepared.completed,
            &index,
            &prepared.limits,
            &mut counters,
            prepared.source.location,
        )
        .unwrap();
        let region = Region::all_open(0);
        let selection = Selection {
            rows: &rows,
            index: &index,
            region: &region,
        };
        let mut visited = 0;
        for (predicate, atoms) in prepared.completed.source_atoms() {
            let relation = Relation::from_atoms(predicate, atoms, Limits::default()).unwrap();
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
        let mut atoms = owner.atom_catalog().atoms().to_vec();
        atoms.sort_unstable_by(|a, b| b.cmp(a));
        assert!(atoms.windows(2).all(|pair| pair[0] > pair[1]));
        let index = AtomIndex::new_with(&atoms, || Ok::<_, FormulaFailure>(())).unwrap();
        let mut checker = owner.checker(ConstraintCheckLimits::default()).unwrap();
        let prepared = checker.prepared.as_mut().unwrap();
        let mut counters = Counters::default();
        let rows = SourceRows::prepare(
            &mut prepared.completed,
            &index,
            &prepared.limits,
            &mut counters,
            prepared.source.location,
        )
        .unwrap();
        for held in 0..atoms.len() {
            let mut region = Region::all_open(atoms.len());
            assert!(region.hold(held));
            let selection = Selection {
                rows: &rows,
                index: &index,
                region: &region,
            };
            for (predicate, source) in prepared.completed.source_atoms() {
                let relation = Relation::from_atoms(predicate, source, Limits::default()).unwrap();
                for (position, atom) in source.iter().enumerate() {
                    let permitted = selection
                        .permits(
                            relation.row(position).unwrap(),
                            &prepared.limits,
                            &mut counters,
                            prepared.source.location,
                        )
                        .unwrap();
                    assert_eq!(permitted, atom == &atoms[held]);
                }
            }
        }
    }
}
