//! Immutable query dimensions and reference-free candidate workspaces.

use std::collections::BTreeMap;
use std::mem::size_of;

use zetesis_core::{Predicate, Program, SeedView, Value};

use super::{
    Check, Limits, Work, bounds,
    relations::{Catalogs, Layout, Layouts, PendingRows, storage},
    row_steps::RowSteps,
};
use crate::{Control, Stop};

/// Independent bounds for preparing queries for one exact admitted program.
/// Source program payload is already owned by `Program` and is not copied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparationLimits {
    /// Template and positive-pattern dimension inspections, and the
    /// argument-bound inference.
    pub max_work: u64,
    /// Named immutable preparation bytes, excluding the shared source program.
    pub max_bytes: usize,
    /// The most tuples a dense relation may index: a predicate whose bounded
    /// arguments admit more keeps its tree. Zero keeps every tree. A dense
    /// relation's words are charged to each candidate's closure bytes.
    pub max_dense_atoms: usize,
}

impl Default for PreparationLimits {
    fn default() -> Self {
        Self {
            max_work: Limits::default().max_work,
            max_bytes: Limits::default().max_closure_bytes,
            max_dense_atoms: 1 << 24,
        }
    }
}

/// Completed preparation receipt, separate from every candidate's work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparationStatistics {
    /// Actual charged dimension inspections.
    pub work: u64,
    /// Named retained preparation storage, excluding the source program.
    pub retained_bytes: usize,
    /// The program's predicates.
    pub predicates: usize,
    /// Predicates read as dense relations: every argument bounded and the
    /// product of widths within `PreparationLimits::max_dense_atoms`.
    pub dense_predicates: usize,
}

/// Prepared join dimensions for one exact immutable program instance.
///
/// This owner shares the admitted program and holds no candidate truth. It does
/// not enumerate a ground carrier. Its evaluator uses scalar delta rounds;
/// within each selected source occurrence tuples keep canonical storage order.
/// Preparation is linear in templates and positive-pattern occurrences. The
/// dimensions bound the assignment, cursor and undo buffers actually used by
/// [`Self::check_view`]. They are not a class certificate or semantic index.
pub struct PreparedQueries {
    program: Program,
    dimensions: Dimensions,
    rules: Rules,
    /// The dense layouts, for the predicates the argument bounds admit.
    layouts: Layouts,
    /// Where a template's innermost join may be taken a row at a time.
    row_steps: RowSteps,
    statistics: PreparationStatistics,
}

#[derive(Default)]
pub(super) struct Dimensions {
    variables: usize,
    depth: usize,
    width: usize,
    /// Templates with a positive body: the most an incremental round visits.
    rules: usize,
}

/// Which templates a round must revisit when a predicate gains rows: those
/// whose positive body names it. A template whose body names no predicate
/// with new rows cannot bind anew, so a round visits only this selection.
#[derive(Default)]
pub(super) struct Rules {
    by_predicate: BTreeMap<Predicate, Vec<usize>>,
}

impl Rules {
    /// Template indices whose positive body names `predicate`, ascending and
    /// without repetition.
    pub(super) fn naming(&self, predicate: &Predicate) -> &[usize] {
        self.by_predicate.get(predicate).map_or(&[], Vec::as_slice)
    }

    fn bytes(&self) -> usize {
        self.by_predicate
            .iter()
            .map(|(predicate, indices)| {
                size_of::<Predicate>()
                    + predicate.name().len()
                    + indices.capacity() * size_of::<usize>()
            })
            .sum()
    }
}

impl PreparedQueries {
    /// Inspect one program under independent finite preparation bounds.
    ///
    /// # Errors
    /// Returns the original cancellation/deadline, work or storage stop. Failure
    /// publishes no prepared owner and is not a candidate membership result.
    pub fn new(
        program: &Program,
        limits: PreparationLimits,
        control: &Control,
    ) -> Result<Self, Stop> {
        control.poll()?;
        let mut work = Work::source(control, limits.max_work);
        work.limits.max_closure_bytes = limits.max_bytes;
        Self::prepare(program, limits.max_dense_atoms, &mut work)
    }

    pub(super) fn prepare(
        program: &Program,
        max_dense_atoms: usize,
        work: &mut Work<'_>,
    ) -> Result<Self, Stop> {
        storage::admit(work, size_of::<Self>() as u128)?;
        storage::record(work, size_of::<Self>() as u128)?;
        let before = work.statistics.work;
        let mut dimensions = Dimensions::default();
        let mut rules = Rules::default();
        for (index, template) in program.templates().iter().enumerate() {
            work.tick()?;
            dimensions.variables = dimensions.variables.max(template.variable_count());
            dimensions.depth = dimensions.depth.max(template.positive().len());
            dimensions.rules += usize::from(!template.positive().is_empty());
            for pattern in template.positive() {
                work.tick()?;
                dimensions.width = dimensions.width.max(pattern.terms().len());
                let naming = rules
                    .by_predicate
                    .entry(pattern.predicate().clone())
                    .or_default();
                if naming.last() != Some(&index) {
                    naming.push(index);
                }
            }
        }
        let bounds = bounds::infer_with(program, max_dense_atoms, work)?;
        let mut layouts = Layouts::default();
        for predicate in program.predicates() {
            work.tick()?;
            if let Some(layout) = bounds
                .bounds(predicate)
                .and_then(|bounds| Layout::new(predicate, bounds, max_dense_atoms))
            {
                layouts.push(layout);
            }
        }
        let row_steps = RowSteps::plan(program, &layouts, work)?;
        let retained_bytes =
            (size_of::<Self>() as u128 + rules.bytes() as u128 + row_steps.bytes() as u128)
                .checked_add(layouts.bytes())
                .ok_or(Stop::StorageLimit)?;
        storage::admit(work, retained_bytes)?;
        storage::record(work, retained_bytes)?;
        Ok(Self {
            program: program.clone(),
            dimensions,
            rules,
            statistics: PreparationStatistics {
                work: work.statistics.work - before,
                retained_bytes: usize::try_from(retained_bytes).map_err(|_| Stop::StorageLimit)?,
                predicates: program.predicates().len(),
                dense_predicates: layouts.len(),
            },
            layouts,
            row_steps,
        })
    }

    /// Exact admitted source owner. Equal source text need not be this instance.
    #[must_use]
    pub const fn program(&self) -> &Program {
        &self.program
    }

    /// Actual completed preparation, which is not charged again per candidate.
    #[must_use]
    pub const fn statistics(&self) -> PreparationStatistics {
        self.statistics
    }

    /// Compute a frozen seed's complete reduct closure using reusable capacity.
    ///
    /// All candidate truth is empty initially. A completed call transfers atom
    /// payload to its returned `Check`; only empty catalog metadata, predicate
    /// names and reference-free join/old-new ID capacity remain. Frontiers and
    /// all logical ID lengths are reset before another candidate is evaluated.
    /// Assignment references live
    /// within one immutable round. A different program instance retires the old
    /// workspace before reuse. Retained capacity is admitted under the supplied
    /// limits, including when they are tighter than the preceding call.
    ///
    /// Preparation work is separate. One-shot [`super::check_view`] instead
    /// charges preparation and candidate work to its single `Limits::max_work`.
    /// Thus completed results agree, but exact resource cutoffs can differ.
    ///
    /// # Errors
    /// Returns the same typed candidate stops as [`super::check_view`]. An error
    /// discards the candidate's dirty workspace; no partial `Check` or failure
    /// statistics are published. Previously returned checks remain independent.
    pub fn check_view(
        &self,
        seed: SeedView<'_>,
        workspace: &mut ClosureWorkspace,
        limits: Limits,
        control: &Control,
    ) -> Result<Check, Stop> {
        control.poll()?;
        if !self.program.same_instance(seed.program()) {
            return Err(Stop::WrongProgram);
        }
        let mut work = Work::source(control, limits.max_work);
        work.limits = limits;
        self.check_with(seed, workspace, &mut work)
    }

    pub(super) fn check_with(
        &self,
        seed: SeedView<'_>,
        workspace: &mut ClosureWorkspace,
        work: &mut Work<'_>,
    ) -> Result<Check, Stop> {
        self.check_scheduled(seed, workspace, super::Schedule::Delta, work)
    }

    fn check_scheduled(
        &self,
        seed: SeedView<'_>,
        workspace: &mut ClosureWorkspace,
        schedule: super::Schedule,
        work: &mut Work<'_>,
    ) -> Result<Check, Stop> {
        if !workspace.clean
            || workspace
                .program
                .as_ref()
                .is_some_and(|old| !old.same_instance(&self.program))
        {
            *workspace = ClosureWorkspace::default();
        }
        workspace.clean = false;
        workspace.program = Some(self.program.clone());
        let result = self.evaluate(seed, workspace, schedule, work);
        if result.is_ok() {
            workspace.clean = true;
        } else {
            *workspace = ClosureWorkspace::default();
        }
        result
    }

    fn evaluate(
        &self,
        seed: SeedView<'_>,
        workspace: &mut ClosureWorkspace,
        schedule: super::Schedule,
        work: &mut Work<'_>,
    ) -> Result<Check, Stop> {
        let completed = self.closure_with(super::Gates::Frozen(seed), workspace, schedule, work)?;
        let seed_mismatch =
            !super::gate_agreement(&self.program, seed, completed.atoms.atoms(), work)?;
        work.statistics.derived_atoms = completed.atoms.atoms().len();
        work.control.poll()?;
        Ok(Check {
            program: self.program.clone(),
            closure: completed.atoms,
            constraint_violated: completed.constraint_violated,
            seed_mismatch,
            statistics: work.statistics,
        })
    }
    pub(super) fn closure_with(
        &self,
        gates: super::Gates<'_>,
        workspace: &mut ClosureWorkspace,
        schedule: super::Schedule,
        work: &mut Work<'_>,
    ) -> Result<super::CompletedClosure, Stop> {
        // The immutable preparation, its dense layouts included, serves every
        // candidate's closure, so each candidate admits it first.
        let retained = self.statistics.retained_bytes as u128;
        let base = workspace
            .catalogs
            .owned_bytes()
            .checked_add(ClosureWorkspace::headers())
            .and_then(|bytes| bytes.checked_add(retained))
            .ok_or(Stop::StorageLimit)?;
        workspace.buffers.prepare(&self.dimensions, base, work)?;
        let mut live = base
            .checked_add(workspace.buffers.bytes()?)
            .and_then(|bytes| bytes.checked_add(workspace.pending.bytes()))
            .ok_or(Stop::StorageLimit)?;
        workspace.pending.prepare(&self.layouts, &mut live, work)?;
        let overhead = ClosureWorkspace::headers()
            .checked_add(workspace.buffers.bytes()?)
            .and_then(|bytes| bytes.checked_add(workspace.pending.bytes()))
            .and_then(|bytes| bytes.checked_add(retained))
            .ok_or(Stop::StorageLimit)?;
        workspace.catalogs.set_overhead(overhead, work)?;
        if self.program.templates().is_empty() {
            return Ok(super::CompletedClosure {
                atoms: zetesis_core::Model::default(),
                constraint_violated: false,
            });
        }
        super::least_closure_with(
            &self.program,
            gates,
            &mut workspace.catalogs,
            super::RoundWorkspace {
                buffers: &mut workspace.buffers,
                dimensions: &self.dimensions,
                rules: &self.rules,
                layouts: &self.layouts,
                row_steps: &self.row_steps,
                pending: &mut workspace.pending,
                overhead,
            },
            schedule,
            work,
        )
    }
}

/// Reusable empty relation metadata, ordered ID views and join cursor capacity.
///
/// This owner never retains borrowed values or candidate truth after a completed
/// call. It can be moved between workers. No thread identity, global cache or
/// shared mutable result storage is involved. Failed or unwound candidates are
/// discarded before the next use; returned `Check` payload remains independent.
/// Final atom buffers are transferred to the result and cannot be reused here.
pub struct ClosureWorkspace {
    program: Option<Program>,
    catalogs: Catalogs,
    buffers: Buffers,
    pending: PendingRows,
    clean: bool,
}

impl Default for ClosureWorkspace {
    fn default() -> Self {
        Self {
            program: None,
            catalogs: Catalogs::default(),
            buffers: Buffers::default(),
            pending: PendingRows::default(),
            clean: true,
        }
    }
}

impl ClosureWorkspace {
    fn headers() -> u128 {
        (size_of::<Self>() - size_of::<Catalogs>()) as u128
    }

    /// Named retained capacity, including owner headers and empty predicate/index
    /// storage. Excludes shared source/preparation and final returned models,
    /// tree-container/allocator overhead and Arc counters. Not a process ceiling.
    /// This remains observable for conservative collective cache admission.
    ///
    /// # Errors
    /// Returns `StorageLimit` if the checked wide capacity sum overflows.
    pub fn retained_bytes(&self) -> Result<u128, Stop> {
        let buffers = self.buffers.bytes()?;
        self.catalogs
            .owned_bytes()
            .checked_add(Self::headers())
            .and_then(|bytes| bytes.checked_add(buffers))
            .and_then(|bytes| bytes.checked_add(self.pending.bytes()))
            .ok_or(Stop::StorageLimit)
    }
}

#[derive(Default)]
pub(super) struct Buffers {
    pub(super) cursors: Vec<Option<super::window::Window>>,
    /// The relation handle of each depth, resolved once per visit.
    pub(super) slots: Vec<super::relations::Slot>,
    pub(super) undo: Vec<Vec<usize>>,
    /// The templates one incremental round visits, in template order.
    pub(super) rules: Vec<usize>,
}

impl Buffers {
    pub(super) fn local(depth: usize) -> Self {
        Self {
            cursors: vec![None; depth],
            slots: vec![super::relations::Slot::Unresolved; depth],
            undo: vec![Vec::new(); depth],
            rules: Vec::new(),
        }
    }

    fn bytes(&self) -> Result<u128, Stop> {
        let headers = self.cursors.capacity() as u128
            * size_of::<Option<super::window::Window>>() as u128
            + self.slots.capacity() as u128 * size_of::<super::relations::Slot>() as u128
            + self.undo.capacity() as u128 * size_of::<Vec<usize>>() as u128
            + self.rules.capacity() as u128 * size_of::<usize>() as u128;
        self.undo.iter().try_fold(headers, |bytes, row| {
            bytes
                .checked_add(row.capacity() as u128 * size_of::<usize>() as u128)
                .ok_or(Stop::StorageLimit)
        })
    }

    fn prepare(
        &mut self,
        dimensions: &Dimensions,
        base: u128,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
        work.charge(self.undo.len())?;
        let mut live = base.checked_add(self.bytes()?).ok_or(Stop::StorageLimit)?;
        storage::admit(work, live)?;
        storage::record(work, live)?;
        reserve(&mut self.cursors, dimensions.depth, &mut live, work)?;
        reserve(&mut self.slots, dimensions.depth, &mut live, work)?;
        reserve(&mut self.undo, dimensions.depth, &mut live, work)?;
        reserve(&mut self.rules, dimensions.rules, &mut live, work)?;
        work.charge(dimensions.depth.saturating_sub(self.cursors.len()))?;
        self.cursors.resize_with(dimensions.depth, || None);
        self.slots
            .resize(dimensions.depth, super::relations::Slot::Unresolved);
        work.charge(dimensions.depth.saturating_sub(self.undo.len()))?;
        self.undo.resize_with(dimensions.depth, Vec::new);
        for row in &mut self.undo {
            work.tick()?;
            reserve(row, dimensions.width, &mut live, work)?;
        }
        Ok(())
    }
}

pub(super) fn assignment<'source>(
    dimensions: &Dimensions,
    base: u128,
    work: &mut Work<'_>,
) -> Result<(Vec<Option<&'source Value>>, u128), Stop> {
    let mut values = Vec::new();
    let mut live = base
        .checked_add(size_of::<Vec<Option<&Value>>>() as u128)
        .ok_or(Stop::StorageLimit)?;
    reserve(&mut values, dimensions.variables, &mut live, work)?;
    work.charge(dimensions.variables)?;
    values.resize(dimensions.variables, None);
    storage::admit(work, live)?;
    storage::record(work, live)?;
    Ok((values, live - base))
}

// Admit the conservative old+replacement overlap before reservation; read back
// actual allocator capacity before using it. A refusal publishes no query result.
fn reserve<T>(
    values: &mut Vec<T>,
    count: usize,
    live: &mut u128,
    work: &mut Work<'_>,
) -> Result<(), Stop> {
    if values.capacity() >= count {
        return Ok(());
    }
    let replacement = count as u128 * size_of::<T>() as u128;
    storage::admit(
        work,
        live.checked_add(replacement).ok_or(Stop::StorageLimit)?,
    )?;
    work.charge(values.len())?;
    let old = values.capacity() as u128 * size_of::<T>() as u128;
    values
        .try_reserve_exact(count - values.len())
        .map_err(|_| Stop::Allocation)?;
    let actual = values.capacity() as u128 * size_of::<T>() as u128;
    storage::after_reservation(work, live.checked_add(actual).ok_or(Stop::StorageLimit)?)?;
    *live = live
        .checked_sub(old)
        .and_then(|bytes| bytes.checked_add(actual))
        .ok_or(Stop::StorageLimit)?;
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod delta_tests;
