//! Authenticated objective read sets and compact eligibility coordinates.
//!
//! The signed-predicate read set contains every positive pattern and every atom
//! in a closed condition. Selection scans the complete catalog in source order;
//! hence its positions are strictly increasing. The index and atom-node prefix
//! share that selection, but only atom payloads retain original coordinates.
//! Formula child references address the compact prefix. Missing condition atoms
//! remain false through the common compiler, and bounds keep the full universe.

use std::{cmp::Ordering, mem::size_of};

use zetesis_algebra::{Cancellation, MeterBytes, MeterBytesError};
use zetesis_core::catalog::PredicateRef;
use zetesis_core::{AtomIndex, AtomIndexError, SelectedAtomIndexErrorKind};
use zetesis_ferraris::{FormulaNodes, NodeView};
use zetesis_objective::{ConditionNode, ObjectiveProgram};

use super::{
    ObjectiveBoundError, ObjectiveBoundErrorKind as Kind, ObjectiveBoundResource as Resource,
    ObjectiveBoundStatistics, ObjectivePlan, ObjectivePlanLimits, Work, join,
};

/// A borrow of one admitted source's complete atom meanings, theory and objectives.
///
/// Only admitted formula and streamed-core owners mint this handle. Their source
/// interner and source selection establish unique original atom positions; an
/// arbitrary caller-supplied catalog cannot claim that invariant. The handle
/// neither owns copied payload nor authorizes a restriction of reduct membership.
#[derive(Clone, Copy, Debug)]
pub struct ObjectiveSource<'a> {
    compiled: &'a crate::formula::Compiled,
}
impl<'a> ObjectiveSource<'a> {
    pub(crate) fn from_compiled(compiled: &'a crate::formula::Compiled) -> Self {
        Self { compiled }
    }
}

mod attempt;
pub use attempt::{
    ObjectiveProjectionAttempt, ObjectiveProjectionError, ObjectiveProjectionErrorKind,
    ObjectiveProjectionReceipt,
};

fn existing(error: ObjectiveBoundError) -> ObjectiveProjectionErrorKind {
    ObjectiveProjectionErrorKind::Existing(error.kind())
}

/// This owner covers the two vector headers and its own fields. The borrowed
/// source, existing formula DAG and common compiler retain their own contracts.
/// Requested growth is checked before allocation; actual capacity is observed
/// afterwards. Dropping one vector releases its payload before index creation.
struct Storage {
    limit: u128,
    peak: u128,
    live: u128,
}
impl Storage {
    fn new(limit: u128) -> Self {
        Self {
            limit,
            peak: 0,
            live: 0,
        }
    }
    fn bytes(error: MeterBytesError) -> ObjectiveProjectionErrorKind {
        match error {
            MeterBytesError::Limit { observed, limit } => ObjectiveProjectionErrorKind::Bytes {
                required: observed,
                limit,
            },
            MeterBytesError::Allocation => ObjectiveProjectionErrorKind::Existing(Kind::Allocation),
            MeterBytesError::Overflow => ObjectiveProjectionErrorKind::Existing(Kind::Overflow),
        }
    }
    fn observe(&mut self, total: u128) -> Result<(), ObjectiveProjectionErrorKind> {
        self.live = total;
        MeterBytes::observe_peak(total, self.limit, &mut self.peak).map_err(Self::bytes)
    }
    fn add(&self, bytes: u128) -> Result<u128, ObjectiveProjectionErrorKind> {
        self.live
            .checked_add(bytes)
            .ok_or(ObjectiveProjectionErrorKind::Existing(Kind::Overflow))
    }
    fn reserve<T: Copy>(
        &mut self,
        values: &mut Vec<T>,
        ceiling: usize,
        work: &mut Work<'_>,
    ) -> Result<(), ObjectiveProjectionErrorKind> {
        if values.len() < values.capacity() {
            return Ok(());
        }
        let next = values
            .len()
            .checked_add(1)
            .ok_or_else(|| existing(work.error(Kind::Overflow)))?;
        let capacity = values.capacity().saturating_mul(2).max(next).min(ceiling);
        // Explicit replacement gives the byte receipt a real overlap boundary,
        // independent of whether an allocator could grow the old buffer in place.
        let mut replacement = Vec::new();
        self.vector_header::<T>()?;
        let requested = self.add(capacity as u128 * size_of::<T>() as u128)?;
        MeterBytes::check(requested, self.limit).map_err(Self::bytes)?;
        work.tick().map_err(existing)?;
        replacement
            .try_reserve_exact(capacity)
            .map_err(|_| existing(work.error(Kind::Allocation)))?;
        let actual = self.add(replacement.capacity() as u128 * size_of::<T>() as u128)?;
        self.observe(actual)?;
        for &value in values.iter() {
            work.tick().map_err(existing)?;
            replacement.push(value);
        }
        self.release(std::mem::replace(values, replacement))
    }
    fn release<T>(&mut self, values: Vec<T>) -> Result<(), ObjectiveProjectionErrorKind> {
        let bytes =
            size_of::<Vec<T>>() as u128 + values.capacity() as u128 * size_of::<T>() as u128;
        drop(values);
        self.live = self
            .live
            .checked_sub(bytes)
            .ok_or(ObjectiveProjectionErrorKind::Existing(Kind::Overflow))?;
        Ok(())
    }
    fn vector_header<T>(&mut self) -> Result<(), ObjectiveProjectionErrorKind> {
        let total = self.add(size_of::<Vec<T>>() as u128)?;
        self.observe(total)
    }
    fn record_index_peak(&mut self, peak: u128) -> Result<(), ObjectiveProjectionErrorKind> {
        let total = self.add(peak)?;
        self.peak = self.peak.max(total);
        Ok(())
    }
    fn index_peak(&mut self, peak: u128) -> Result<(), ObjectiveProjectionErrorKind> {
        let total = self.add(peak)?;
        MeterBytes::observe_peak(total, self.limit, &mut self.peak).map_err(Self::bytes)
    }
}

pub(super) fn compile<'a>(
    source: ObjectiveSource<'a>,
    limits: ObjectivePlanLimits,
    max_projection_bytes: u128,
    cancellation: &'a Cancellation,
) -> ObjectiveProjectionAttempt {
    let mut work = Work::new(limits, cancellation);
    // Preserve ordinary preparation's first control observation before storage.
    if let Err(error) = work.tick() {
        return ObjectiveProjectionAttempt::refused(error, 0);
    }
    let mut storage = Storage::new(max_projection_bytes);
    let prepared = storage
        .observe(size_of::<Storage>() as u128)
        .and_then(|()| prepare(source, &mut storage, &mut work));
    let (index, nodes, count) = match prepared {
        Ok(prepared) => prepared,
        Err(kind) => {
            // Every projection helper borrows these same owners. Capture once,
            // after its typed refusal, without polling or admission again.
            return ObjectiveProjectionAttempt::new(
                Err(kind),
                work.template,
                work.statistics(),
                storage.peak,
            );
        }
    };
    let result = join::complete(
        &source.compiled.theory,
        &source.compiled.objectives,
        work,
        index,
        nodes,
        join::AtomPrefix::Selected(count),
    );
    match result {
        Ok(plan) => {
            let statistics = plan.statistics();
            ObjectiveProjectionAttempt::new(Ok(plan), None, statistics, storage.peak)
        }
        Err(error) => ObjectiveProjectionAttempt::refused(error, storage.peak),
    }
}

fn prepare<'a>(
    source: ObjectiveSource<'a>,
    storage: &mut Storage,
    work: &mut Work<'_>,
) -> Result<(AtomIndex<'a>, FormulaNodes, usize), ObjectiveProjectionErrorKind> {
    let atoms = source.compiled.atoms.atoms();
    if atoms.len() != source.compiled.theory.atom_count() {
        return Err(existing(work.error(Kind::AtomCatalog)));
    }
    let signatures = read_set(&source.compiled.objectives, storage, work)?;
    let mut positions = Vec::new();
    storage.vector_header::<usize>()?;
    for position in 0..atoms.len() {
        work.tick().map_err(existing)?;
        let atom = atoms
            .at(position)
            .expect("bounded authenticated catalog position");
        if locate(&signatures, atom.predicate(), work)
            .map_err(existing)?
            .is_ok()
        {
            if positions.len() >= work.limits.max_atoms {
                return Err(existing(work.limit(Resource::Atoms)));
            }
            storage.reserve(&mut positions, work.limits.max_atoms, work)?;
            work.tick().map_err(existing)?;
            positions.push(position);
        }
    }
    storage.release(signatures)?;
    let remaining = storage
        .limit
        .checked_sub(storage.live)
        .ok_or(ObjectiveProjectionErrorKind::Existing(Kind::Overflow))?;
    let prepared =
        AtomIndex::from_catalog_selection_with(atoms, &positions, remaining, || work.tick());
    let index = match prepared {
        Ok(index) => {
            storage.index_peak(index.preparation_peak_bytes())?;
            index
        }
        Err(error) => {
            // Preserve the index's established callback refusal, even if its
            // actual owner header was observed before a smaller byte check.
            storage.record_index_peak(error.preparation_peak_bytes())?;
            return Err(match error.into_kind() {
                SelectedAtomIndexErrorKind::Index(AtomIndexError::Stopped(error)) => {
                    existing(error)
                }
                SelectedAtomIndexErrorKind::Index(AtomIndexError::Allocation) => {
                    existing(work.error(Kind::Allocation))
                }
                SelectedAtomIndexErrorKind::Index(AtomIndexError::Duplicate { .. })
                | SelectedAtomIndexErrorKind::Position { .. }
                | SelectedAtomIndexErrorKind::Order { .. } => {
                    existing(work.error(Kind::AtomCatalog))
                }
                SelectedAtomIndexErrorKind::Bytes { required, .. } => {
                    let required = storage.add(required)?;
                    Storage::bytes(MeterBytesError::Limit {
                        observed: required,
                        limit: storage.limit,
                    })
                }
            });
        }
    };
    let mut nodes = FormulaNodes::default();
    for &position in &positions {
        work.node(&mut nodes, NodeView::Atom(position))
            .map_err(existing)?;
    }
    let count = positions.len();
    storage.release(positions)?;
    Ok((index, nodes, count))
}

fn read_set<'a>(
    objectives: &'a ObjectiveProgram,
    storage: &mut Storage,
    work: &mut Work<'_>,
) -> Result<Vec<PredicateRef<'a>>, ObjectiveProjectionErrorKind> {
    let mut signatures = Vec::new();
    storage.vector_header::<PredicateRef<'_>>()?;
    for (index, template) in objectives.templates().iter().enumerate() {
        work.template = Some(index);
        work.tick().map_err(existing)?;
        for pattern in 0..template.positive().len() {
            work.tick().map_err(existing)?;
            let predicate = template
                .positive()
                .at(pattern)
                .expect("bounded positive pattern")
                .predicate();
            insert(&mut signatures, predicate, storage, work)?;
        }
        let condition = template.condition().nodes();
        for node in 0..condition.len() {
            work.tick().map_err(existing)?;
            if let ConditionNode::Atom(atom) = condition.at(node).expect("bounded closed condition")
            {
                insert(&mut signatures, atom.predicate(), storage, work)?;
            }
        }
    }
    work.template = None;
    Ok(signatures)
}

fn insert<'a>(
    signatures: &mut Vec<PredicateRef<'a>>,
    predicate: PredicateRef<'a>,
    storage: &mut Storage,
    work: &mut Work<'_>,
) -> Result<(), ObjectiveProjectionErrorKind> {
    let Err(position) = locate(signatures, predicate, work).map_err(existing)? else {
        return Ok(());
    };
    storage.reserve(signatures, usize::MAX, work)?;
    // Each admitted shift executes before the next poll; an interrupted vector
    // is private scratch and is never published as a completed read set.
    work.tick().map_err(existing)?;
    signatures.push(predicate);
    for index in (position..signatures.len() - 1).rev() {
        work.tick().map_err(existing)?;
        signatures[index + 1] = signatures[index];
    }
    work.tick().map_err(existing)?;
    signatures[position] = predicate;
    Ok(())
}

fn locate(
    signatures: &[PredicateRef<'_>],
    predicate: PredicateRef<'_>,
    work: &mut Work<'_>,
) -> Result<Result<usize, usize>, ObjectiveBoundError> {
    let mut start = 0;
    let mut end = signatures.len();
    while start < end {
        let middle = start + (end - start) / 2;
        match signatures[middle].compare_ref_with(predicate, || work.tick())? {
            Ordering::Less => start = middle + 1,
            Ordering::Equal => return Ok(Ok(middle)),
            Ordering::Greater => end = middle,
        }
    }
    Ok(Err(start))
}

#[cfg(test)]
mod tests;
