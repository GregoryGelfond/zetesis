//! A round's private discovery coordinates stay attached to their append authority.

use super::{
    AtomAppender, AtomRef, AtomicUsize, CatalogRead, Counters, Failure, FormulaFailure,
    FormulaLimits, FormulaResource, Memory, Ordering, ProgramSite, TermRef, atom_failure,
    atom_interner, ceiling, failure, owner_limits, record_owner_peak, size_of,
};
use crate::formula_support::GroundingWork;
use zetesis_algebra::words::Words;
use zetesis_core::catalog::PredicateRef;

mod assigned;
pub(crate) use assigned::{FactRows, SourceAtom, SourceScope};

#[cfg(test)]
mod order_tests;

pub(super) struct Base {
    pub(super) bytes: usize,
    pub(super) owner: u128,
    pub(super) pending: usize,
    pub(super) supported: usize,
}

/// Each recorded fact rule's index and its head's atom position.
pub(in crate::formula_support) type HeadRows = Vec<(usize, usize)>;

pub(crate) struct SupportAppend<'a> {
    owner: AtomAppender<'a>,
    pending: &'a mut Vec<usize>,
    /// Open only in support's first round, with a reserved cell per fact row.
    fact_rows: Option<&'a mut Vec<usize>>,
    /// A run of fact rules' heads prepared at the owner as one fact block.
    head_block: Option<assigned::FactRows>,
    /// Open only in support's first round without fact blocks: each recorded
    /// fact rule's index and head atom position.
    head_rows: Option<&'a mut HeadRows>,
    /// The fact rule whose head the open head block admits next.
    head_rule: Option<usize>,
    supported: &'a mut Words<u64>,
    scope: &'a SourceScope,
    growth: &'a AtomicUsize,
    base: Base,
}

impl<'a> SupportAppend<'a> {
    /// Only live split views carry this exact growth owner. Safe exclusive
    /// catalog borrowing prevents a second epoch while either view survives.
    /// A pointer match alone would not certify an arbitrary stored snapshot.
    pub(in crate::formula_support) fn belongs_to(&self, relations: &super::Relations<'_>) -> bool {
        relations
            .growth
            .is_some_and(|growth| std::ptr::eq(growth, self.growth))
    }

    pub(in crate::formula_support) fn read(&self) -> CatalogRead<'_> {
        self.owner.read()
    }

    pub(in crate::formula_support) fn term_lookup(&self) -> atom_interner::TermLookup<'_> {
        self.owner.term_lookup()
    }

    pub(in crate::formula_support) fn import(
        &mut self,
        value: TermRef<'_>,
        workspace: usize,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
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
        (pending, fact_rows, head_rows): (
            &'a mut Vec<usize>,
            Option<&'a mut Vec<usize>>,
            Option<&'a mut HeadRows>,
        ),
        supported: &'a mut Words<u64>,
        scope: &'a SourceScope,
        growth: &'a AtomicUsize,
        base: Base,
    ) -> Self {
        Self {
            owner,
            pending,
            fact_rows,
            head_block: None,
            head_rows,
            head_rule: None,
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
        location: ProgramSite,
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

    /// Record a fact row's atom in the cell reserved for it. One unit.
    pub(in crate::formula_support) fn record_fact(
        &mut self,
        atom: &SourceAtom,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        counters.work(limits, location)?;
        let Some(rows) = self.fact_rows.as_deref_mut() else {
            return Err(failure(Failure::Owner, location));
        };
        if !self.scope.same(&atom.scope) || rows.len() == rows.capacity() {
            return Err(failure(Failure::Owner, location));
        }
        rows.push(atom.position);
        Ok(())
    }

    pub(in crate::formula_support) fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub(in crate::formula_support) fn atoms(&self) -> impl Iterator<Item = AtomRef<'_>> {
        self.pending
            .iter()
            .map(|&id| self.owner.get(id).expect("round-local discovery"))
    }

    #[cfg(test)]
    pub(in crate::formula_support) fn atom(
        &mut self,
        atom: AtomRef<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        let atom = self.discover_ref(atom, 0, limits, counters, location)?;
        self.retain(&atom, 0, limits, counters, location)
    }

    /// Put the round's discoveries in typed atom order. Each atom is read
    /// once into a key; the natural merge sort then compares keys only.
    pub(in crate::formula_support) fn order(
        &mut self,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
        executor: Option<&crate::formula_support::Executor>,
    ) -> Result<(), FormulaFailure> {
        // Admitted keys, scratch and run-start buffers for a natural merge
        // sort: discoveries mostly arrive in typed order, so it mostly
        // confirms runs, and the merges of one pass are independent, so the
        // executor's workers share each pass.
        let workspace = counters.accounting.workspace.bytes();
        let bytes = usize::try_from(self.owner.storage_bytes() + self.outer_bytes(0))
            .map_err(|_| failure(Failure::Overflow, location))?;
        let mut memory = Memory::new(bytes, workspace as u128, limits, counters, location);
        let owner = &self.owner;
        let mut keys = Vec::new();
        let mut terms = Vec::new();
        let mut scratch = Vec::new();
        let mut runs = Vec::new();
        let result = (|| {
            memory.reserve(&mut keys, self.pending.len())?;
            memory.reserve(&mut scratch, self.pending.len())?;
            memory.reserve(&mut runs, self.pending.len() + 1)?;
            // Each atom's row, arity and columns are read once, with the
            // permits a typed atom comparison pays for reading them; the
            // comparisons then read only these keys.
            for &position in self.pending.iter() {
                counters.charge_work(2, limits, location)?;
                let atom = owner.get(position).expect("round-local discovery");
                let predicate = atom.predicate();
                let arity = predicate.arity();
                counters.charge_work(arity as u128, limits, location)?;
                memory.reserve(&mut terms, arity)?;
                let start = terms.len();
                terms.extend((0..arity).map(|column| {
                    atom.values()
                        .at(column)
                        .expect("an atom has a value per column")
                }));
                keys.push(Key {
                    position,
                    predicate,
                    start,
                });
            }
            let terms = &terms;
            crate::formula_support::sort::merge_by(
                &mut keys,
                &mut scratch,
                &mut runs,
                executor.and_then(crate::formula_support::Executor::pool),
                GroundingWork::new(limits, counters, location),
                |left, right, charged| left.compare(right, terms, charged),
            )?;
            // One permit per position written back in typed order.
            counters.charge_work(keys.len() as u128, limits, location)?;
            for (cell, key) in self.pending.iter_mut().zip(&keys) {
                *cell = key.position;
            }
            Ok(())
        })();
        // Release exactly what this receipt recorded; the buffers drop here.
        let admitted = memory.bytes - bytes;
        memory.release(admitted);
        drop((keys, scratch, terms, runs));
        result
    }
}

/// A pending atom's position with its predicate and the start of its
/// columns in the sort's term buffer, read once before sorting.
#[derive(Clone, Copy)]
struct Key<'a> {
    position: usize,
    predicate: PredicateRef<'a>,
    start: usize,
}

impl Key<'_> {
    /// The typed atom order: predicates, then columns left to right, each
    /// compared as a typed atom comparison compares them. Distinct positions
    /// are distinct atoms, so no identity check precedes the fields.
    fn compare(
        &self,
        other: &Self,
        terms: &[TermRef<'_>],
        charged: &mut u64,
    ) -> std::cmp::Ordering {
        let mut before = || {
            *charged += 1;
            Ok::<_, std::convert::Infallible>(())
        };
        let order: Result<_, std::convert::Infallible> = (|| {
            let predicate = self
                .predicate
                .compare_ref_with(other.predicate, &mut before)?;
            if !predicate.is_eq() {
                return Ok(predicate);
            }
            for column in 0..self.predicate.arity() {
                let order = terms[self.start + column]
                    .compare_ref_with(terms[other.start + column], &mut before)?;
                if !order.is_eq() {
                    return Ok(order);
                }
            }
            Ok(std::cmp::Ordering::Equal)
        })();
        match order {
            Ok(order) => order,
            Err(never) => match never {},
        }
    }
}
