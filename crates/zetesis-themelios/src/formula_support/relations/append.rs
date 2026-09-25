//! A round's private discovery coordinates stay attached to their append authority.

use super::{
    AtomAppender, AtomKey, AtomRef, AtomicUsize, CatalogRead, Counters, Failure, FormulaFailure,
    FormulaLimits, FormulaResource, Location, Memory, Ordering, TermRef, atom_failure,
    atom_interner, ceiling, failure, owner_limits, record_owner_peak, size_of,
};
use crate::formula_support::GroundingWork;

mod assigned;
pub(in crate::formula_support) use assigned::AssignedAtom;
pub(crate) use assigned::{SourceAtom, SourceScope};

pub(super) struct Base {
    pub(super) bytes: usize,
    pub(super) owner: u128,
    pub(super) pending: usize,
    pub(super) supported: usize,
}

pub(crate) struct SupportAppend<'a> {
    owner: AtomAppender<'a>,
    pending: &'a mut Vec<usize>,
    supported: &'a mut Vec<u64>,
    scope: &'a SourceScope,
    growth: &'a AtomicUsize,
    base: Base,
}

impl<'a> SupportAppend<'a> {
    pub(in crate::formula_support) fn read(&self) -> CatalogRead<'_> {
        self.owner.read()
    }

    pub(in crate::formula_support) fn import(
        &mut self,
        value: TermRef<'_>,
        workspace: usize,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<zetesis_core::catalog::TermKey, FormulaFailure> {
        let outer = self.outer_bytes(workspace);
        self.owner.restart_storage_peak();
        let checked = owner_limits(limits, outer, location)?;
        // The source value already passed its ingress policy. This boundary
        // bounds retained canonical storage rather than imposing a second,
        // unrelated depth or flat-value allocation policy.
        let term_limits = zetesis_core::catalog::Limits {
            max_nodes: usize::MAX,
            max_depth: usize::MAX,
            max_bytes: usize::MAX,
        };
        let result = self
            .owner
            .import_term_with(value, term_limits, checked, || {
                counters.work(limits, location)
            });
        let refreshed = self.refresh(workspace, limits, counters, location);
        let key = result.map_err(|error| atom_failure(error, limits, outer, location))?;
        refreshed?;
        Ok(key)
    }

    pub(in crate::formula_support) fn construct(
        &mut self,
        descriptor: zetesis_core::ValueNodeRef<'_>,
        values: zetesis_core::catalog::AssignmentSlice<'_>,
        children: &[usize],
        term_limits: zetesis_core::catalog::Limits,
        workspace: usize,
        work: GroundingWork<'_>,
    ) -> Result<zetesis_core::catalog::TermKey, FormulaFailure> {
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        let outer = self.outer_bytes(workspace);
        self.owner.restart_storage_peak();
        let checked = owner_limits(limits, outer, location)?;
        let result = self.owner.construct_term_with(
            descriptor,
            values,
            children,
            term_limits,
            checked,
            || counters.work(limits, location),
        );
        let refreshed = self.refresh(workspace, limits, counters, location);
        let key = result.map_err(|error| match error {
            atom_interner::AssignedFailure::Assignment(error) => {
                crate::formula_binding::assignment(error, location)
            }
            atom_interner::AssignedFailure::Interner(error) => {
                atom_failure(error, limits, outer, location)
            }
        })?;
        refreshed?;
        Ok(key)
    }
    pub(super) fn new(
        owner: AtomAppender<'a>,
        pending: &'a mut Vec<usize>,
        supported: &'a mut Vec<u64>,
        scope: &'a SourceScope,
        growth: &'a AtomicUsize,
        base: Base,
    ) -> Self {
        Self {
            owner,
            pending,
            supported,
            scope,
            growth,
            base,
        }
    }

    fn outer_bytes(&self, workspace: usize) -> u128 {
        self.base.bytes as u128
            - self.base.owner
            - self.base.pending as u128
            - self.base.supported as u128
            + (self.pending.capacity() * size_of::<usize>()) as u128
            + (self.supported.capacity() * size_of::<u64>()) as u128
            + workspace as u128
    }

    fn refresh(
        &self,
        workspace: usize,
        limits: &FormulaLimits,
        counters: &Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        let current = self.owner.storage_bytes() + self.outer_bytes(workspace);
        record_owner_peak(
            self.owner.storage_peak_bytes(),
            self.outer_bytes(workspace),
            counters,
        );
        let owner_growth = self.owner.storage_bytes() - self.base.owner;
        let pending_growth = self.pending.capacity() * size_of::<usize>() - self.base.pending;
        let supported_growth = self.supported.capacity() * size_of::<u64>() - self.base.supported;
        self.growth.store(
            usize::try_from(owner_growth)
                .ok()
                .and_then(|growth| growth.checked_add(pending_growth))
                .and_then(|growth| growth.checked_add(supported_growth))
                .ok_or_else(|| failure(Failure::Overflow, location))?,
            Ordering::Relaxed,
        );
        ceiling(
            FormulaResource::SupportBytes,
            current,
            limits.max_support_bytes as u128,
            location,
        )
    }

    pub(in crate::formula_support) fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub(in crate::formula_support) fn atoms(&self) -> impl Iterator<Item = AtomRef<'_>> {
        self.pending
            .iter()
            .map(|&id| self.owner.get(id).expect("round-local discovery"))
    }

    pub(in crate::formula_support) fn contains(
        &self,
        key: AtomKey<'_>,
        workspace: usize,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<bool, FormulaFailure> {
        let outer = self.outer_bytes(workspace);
        let position = self
            .owner
            .find_key_with(key, owner_limits(limits, outer, location)?, || {
                counters.work(limits, location)
            })
            .map_err(|error| atom_failure(error, limits, outer, location))?;
        counters.work(limits, location)?;
        Ok(position.is_some_and(|position| self.is_supported(position)))
    }

    #[cfg(test)]
    pub(in crate::formula_support) fn atom(
        &mut self,
        atom: AtomRef<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        let atom = self.discover_ref(atom, 0, limits, counters, location)?;
        self.retain(&atom, 0, limits, counters, location)
    }

    /// Checked in-place sorting preserves typed atom publication order without
    /// copying payload or allocating a sorting workspace.
    pub(in crate::formula_support) fn order(
        &mut self,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        let owner = &self.owner;
        crate::formula_support::sort::by(
            self.pending,
            GroundingWork::new(limits, counters, location),
            |left, right, work| {
                let left = owner.get(*left).expect("round-local discovery");
                let right = owner.get(*right).expect("round-local discovery");
                left.compare_ref_with(right, || work.counters.work(work.limits, work.location))
            },
        )
    }
}
