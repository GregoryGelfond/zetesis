//! Fixed disjoint candidate ranges over persistent, exclusive workspaces.

use std::mem::size_of;

use rayon::iter::plumbing::{Producer, ProducerCallback};
use zetesis_core::{Program, SeedView};

use crate::{Check, ClosureWorkspace, Control, Limits, PreparationLimits, PreparedQueries, Stop};
use super::{BatchError, QueryStatistics};

#[derive(Default)]
pub(super) struct Cache {
    prepared: Option<PreparedQueries>,
    workspaces: Vec<ClosureWorkspace>,
    builds: u128,
    active: usize,
    reused: usize,
    reserved: u128,
}

impl Cache {
    pub(super) fn statistics(&self) -> Result<QueryStatistics, BatchError> {
        Ok(QueryStatistics {
            preparation: self.prepared.as_ref().map(PreparedQueries::statistics),
            preparation_builds: self.builds,
            retained_workspaces: self.workspaces.len(),
            active_workspaces: self.active,
            reused_workspaces: self.reused,
            retained_bytes: self.retained()?,
            reserved_bytes: self.reserved,
        })
    }

    fn retire(&mut self) {
        let builds = self.builds;
        *self = Self { builds, ..Self::default() };
    }

    fn overhead(&self) -> u128 {
        size_of::<Self>() as u128 + (self.workspaces.capacity() - self.workspaces.len()) as u128
            * size_of::<ClosureWorkspace>() as u128
    }

    fn retained(&self) -> Result<u128, BatchError> {
        if self.prepared.is_none() && self.workspaces.capacity() == 0 { return Ok(0); }
        self.workspaces.iter().try_fold(self.overhead(), |bytes, workspace| {
            bytes.checked_add(workspace.retained_bytes().map_err(BatchError::Preparation)?)
                .ok_or(BatchError::Preparation(Stop::StorageLimit))
        })
    }

    pub(super) fn admit(&mut self, active: usize, limit: Limits, collective: usize) -> Result<(), BatchError> {
        let shared = self.prepared.as_ref().map_or(0, |prepared| prepared.statistics().retained_bytes);
        let Some(candidate) = limit.max_closure_bytes.checked_sub(shared) else {
            self.retire();
            return Err(BatchError::Preparation(Stop::StorageLimit));
        };
        let candidate = candidate as u128;
        let mut required = self.retained()?;
        for workspace in self.workspaces.iter().take(active) {
            let retained = workspace.retained_bytes().map_err(BatchError::Preparation)?;
            required = required.checked_add(candidate.saturating_sub(retained))
                .ok_or(BatchError::Preparation(Stop::StorageLimit))?;
        }
        let result = ceiling(required, collective);
        if result.is_err() { self.active = 0; self.reused = 0; self.reserved = 0; }
        else { self.reserved = required; }
        result
    }

    pub(super) fn prepare(&mut self, program: &Program, count: usize,
        limits: PreparationLimits, collective: usize, control: &Control) -> Result<(), BatchError>
    {
        control.poll().map_err(BatchError::Preparation)?;
        if self.prepared.as_ref().is_some_and(|prepared| !prepared.program().same_instance(program)) {
            self.retire();
        }
        if let Some(prepared) = &self.prepared {
            if prepared.statistics().retained_bytes > limits.max_bytes {
                return Err(BatchError::Preparation(Stop::StorageLimit));
            }
        } else {
            // The cache header contains the prepared owner; no second payload
            // or preparation header is added after this publication.
            ceiling((size_of::<Self>() + size_of::<PreparedQueries>()) as u128, collective)?;
            let builds = self.builds.checked_add(1).ok_or(BatchError::Preparation(Stop::WorkLimit))?;
            self.prepared = Some(PreparedQueries::new(program, limits, control)
                .map_err(BatchError::Preparation)?);
            self.builds = builds;
        }
        let reused = count.min(self.workspaces.len());
        self.grow(count, collective)?;
        self.active = count;
        self.reused = reused;
        Ok(())
    }

    fn grow(&mut self, count: usize, collective: usize) -> Result<(), BatchError> {
        if self.workspaces.capacity() < count {
            let old = self.retained()?;
            let planned = count as u128 * size_of::<ClosureWorkspace>() as u128;
            ceiling(old.checked_add(planned).ok_or(BatchError::Preparation(Stop::StorageLimit))?, collective)?;
            self.workspaces.try_reserve_exact(count - self.workspaces.len())
                .map_err(|_| BatchError::Preparation(Stop::Allocation))?;
            let actual = self.workspaces.capacity() as u128 * size_of::<ClosureWorkspace>() as u128;
            if let Err(error) = ceiling(old.checked_add(actual)
                .ok_or(BatchError::Preparation(Stop::StorageLimit))?, collective) {
                self.retire();
                return Err(error);
            }
        }
        // New workspace inline headers are already in the admitted vector. Its
        // empty pending-set/root bookkeeping is part of its own named envelope.
        let additional = count.saturating_sub(self.workspaces.len());
        let extra = ClosureWorkspace::default().retained_bytes().map_err(BatchError::Preparation)?
            - size_of::<ClosureWorkspace>() as u128;
        let retained = self.retained()?;
        let proposed = (additional as u128).checked_mul(extra)
            .and_then(|bytes| retained.checked_add(bytes))
            .ok_or(BatchError::Preparation(Stop::StorageLimit))?;
        ceiling(proposed, collective)?;
        self.workspaces.resize_with(count.max(self.workspaces.len()), ClosureWorkspace::default);
        Ok(())
    }

    pub(super) fn execution<'a>(&'a mut self, length: usize, active: usize,
        limits: Limits, control: &'a Control) -> Execution<'a>
    {
        Execution { prepared: self.prepared.as_ref().expect("nonempty batch prepared its program"),
            workspaces: &mut self.workspaces[..active], length, limits, control }
    }
}

fn ceiling(required: u128, limit: usize) -> Result<(), BatchError> {
    if required > limit as u128 { Err(BatchError::ClosureStorage { required, limit: limit as u128 }) }
    else { Ok(()) }
}

pub(super) struct Execution<'a> {
    prepared: &'a PreparedQueries,
    workspaces: &'a mut [ClosureWorkspace],
    length: usize,
    limits: Limits,
    control: &'a Control,
}

impl<'seed> ProducerCallback<SeedView<'seed>> for Execution<'_> {
    type Output = Vec<Result<Check, Stop>>;

    fn callback<P>(self, producer: P) -> Self::Output where P: Producer<Item = SeedView<'seed>> {
        self.run(producer)
    }
}

impl Execution<'_> {
    fn run<'seed, P>(self, producer: P) -> Vec<Result<Check, Stop>>
    where P: Producer<Item = SeedView<'seed>>
    {
        if self.workspaces.len() == 1 {
            let workspace = &mut self.workspaces[0];
            return producer.into_iter().map(|seed| self.prepared.check_view(seed, workspace,
                self.limits, self.control)).collect();
        }
        // Each nonempty range has one exclusive owner. Splits preserve the
        // indexed producer's exact occurrence order, with no seed materialization.
        // Recursion halves the finite workspace count, independently of thread IDs.
        let count = self.workspaces.len();
        let middle = count / 2;
        let left_length = (self.length / count) * middle + (self.length % count).min(middle);
        let (left_rows, right_rows) = producer.split_at(left_length);
        let (left_slots, right_slots) = self.workspaces.split_at_mut(middle);
        let (mut left, right) = rayon::join(
            || Execution { prepared: self.prepared, workspaces: left_slots, length: left_length,
                limits: self.limits, control: self.control }.run(left_rows),
            || Execution { prepared: self.prepared, workspaces: right_slots, length: self.length - left_length,
                limits: self.limits, control: self.control }.run(right_rows));
        // Returned result vectors have their existing separate ownership budget.
        left.extend(right);
        left
    }
}
