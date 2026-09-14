//! Exact complete semantic projections, indexed by a bounded binary trie.

use std::num::NonZeroU32;

use crate::search::Budget;
use crate::{
    AdmissionError, Assignment, Incomplete, ProjectionLimits, ProjectionResource,
    ProjectionStatistics,
};

/// A zero-based arena position represented by its positive successor.
/// Absence belongs to Option; neither root zero nor a valid child is a sentinel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
struct NodeId(NonZeroU32);

impl NodeId {
    fn new(index: usize) -> Result<Self, Incomplete> {
        u32::try_from(index)
            .ok()
            .and_then(|value| value.checked_add(1))
            .and_then(NonZeroU32::new)
            .map(Self)
            .ok_or(Incomplete::Admission(AdmissionError::Overflow))
    }

    fn index(self) -> Result<usize, Incomplete> {
        usize::try_from(self.0.get() - 1).map_err(|_| AdmissionError::Overflow.into())
    }
}

/// Admit the complete suffix before reserving or publishing any part of it.
/// Its last zero-based index must have a representable positive successor.
fn suffix_size(width: usize, depth: usize, start: usize) -> Result<usize, Incomplete> {
    let additional = width
        .checked_sub(depth)
        .and_then(|n| n.checked_add(1))
        .ok_or(Incomplete::CounterOverflow)?;
    let length = start
        .checked_add(additional)
        .ok_or(Incomplete::CounterOverflow)?;
    u32::try_from(length).map_err(|_| AdmissionError::Overflow)?;
    Ok(additional)
}

#[derive(Debug)]
pub(super) struct Projections {
    width: usize,
    nodes: Vec<[Option<NodeId>; 2]>,
    limits: ProjectionLimits,
    entries: u64,
    work: u64,
    peak_bytes: u128,
}
impl Projections {
    pub(super) fn new(width: usize, limits: ProjectionLimits) -> Result<Self, Incomplete> {
        let peak_bytes = size_of::<Self>() as u128;
        bound(
            ProjectionResource::Bytes,
            peak_bytes,
            limits.max_bytes as u128,
        )?;
        Ok(Self {
            width,
            nodes: Vec::new(),
            limits,
            entries: 0,
            work: 0,
            peak_bytes,
        })
    }

    pub(super) fn statistics(&self) -> ProjectionStatistics {
        ProjectionStatistics {
            entries: self.entries,
            nodes: self.nodes.len(),
            retained_bytes: Self::bytes(self.nodes.capacity()),
            peak_bytes: self.peak_bytes,
            work: self.work,
        }
    }

    fn tick(&mut self, budget: &mut Budget<'_>) -> Result<(), Incomplete> {
        let work = self
            .work
            .checked_add(1)
            .ok_or(Incomplete::CounterOverflow)?;
        budget.tick()?;
        self.work = work;
        Ok(())
    }

    fn next_entry(&self) -> Result<u64, Incomplete> {
        let required = u128::from(self.entries) + 1;
        bound(
            ProjectionResource::Entries,
            required,
            u128::from(self.limits.max_entries),
        )?;
        u64::try_from(required).map_err(|_| Incomplete::CounterOverflow)
    }

    /// Every complete-depth path is one excluded projection; intermediate
    /// prefixes are not keys. Admit a distinct entry only after identifying a
    /// missing key, then attach only a fully constructed suffix. A failed
    /// attempt keeps work/capacity receipts but changes no prior key or count.
    pub(super) fn insert(
        &mut self,
        width: usize,
        value: impl Fn(usize) -> bool,
        budget: &mut Budget<'_>,
    ) -> Result<(), Incomplete> {
        self.tick(budget)?;
        if width != self.width {
            return Err(Incomplete::InvalidWitness);
        }
        if width == 0 {
            if self.entries == 0 {
                self.entries = self.next_entry()?;
            }
            return Ok(());
        }
        if self.nodes.is_empty() {
            self.suffix(0, &value, budget)?;
            return Ok(());
        }
        let mut node = 0;
        for variable in 0..width {
            self.tick(budget)?;
            let branch = usize::from(value(variable));
            if let Some(child) = self.nodes[node][branch] {
                node = child.index()?;
            } else {
                let child = self.suffix(variable + 1, &value, budget)?;
                self.nodes[node][branch] = Some(child);
                return Ok(());
            }
        }
        Ok(())
    }

    // Reserve only the missing suffix, keeping the compact trie and amortized
    // vector growth. Logical nodes and actual capacity are different resources.
    // No link or entry count is published before all fallible work completes.
    fn suffix(
        &mut self,
        depth: usize,
        value: &impl Fn(usize) -> bool,
        budget: &mut Budget<'_>,
    ) -> Result<NodeId, Incomplete> {
        let start = self.nodes.len();
        let additional = suffix_size(self.width, depth, start)?;
        let entries = self.next_entry()?;
        let first = NodeId::new(start)?;
        self.reserve(additional)?;
        let result = (|| {
            for variable in depth..self.width {
                self.tick(budget)?;
                let mut children = [None, None];
                // The preflight covers this forward link and the terminal node.
                children[usize::from(value(variable))] = Some(NodeId::new(self.nodes.len() + 1)?);
                self.nodes.push(children);
            }
            self.tick(budget)?;
            self.nodes.push([None, None]);
            Ok(first)
        })();
        if result.is_err() {
            self.nodes.truncate(start);
        } else {
            self.entries = entries;
        }
        result
    }

    fn bytes(capacity: usize) -> u128 {
        size_of::<Self>() as u128 + capacity as u128 * size_of::<[Option<NodeId>; 2]>() as u128
    }

    fn reserve(&mut self, additional: usize) -> Result<(), Incomplete> {
        let required = self
            .nodes
            .len()
            .checked_add(additional)
            .ok_or(Incomplete::CounterOverflow)?;
        bound(
            ProjectionResource::Nodes,
            required as u128,
            self.limits.max_nodes as u128,
        )?;
        let old_capacity = self.nodes.capacity();
        if required > old_capacity {
            let old_bytes = Self::bytes(old_capacity) - size_of::<Self>() as u128;
            // This is the minimum requested growth overlap. Vec may choose
            // larger capacity, so actual capacity is read back before any key
            // publication. Failed admission can retain that enlarged arena.
            bound(
                ProjectionResource::Bytes,
                old_bytes + Self::bytes(required),
                self.limits.max_bytes as u128,
            )?;
            self.nodes
                .try_reserve(additional)
                .map_err(|_| Incomplete::Allocation)?;
            self.peak_bytes = self
                .peak_bytes
                .max(old_bytes + Self::bytes(self.nodes.capacity()));
        }
        bound(
            ProjectionResource::Bytes,
            self.peak_bytes,
            self.limits.max_bytes as u128,
        )
    }

    pub(super) fn permits(
        &mut self,
        assignment: &Assignment,
        budget: &mut Budget<'_>,
    ) -> Result<bool, Incomplete> {
        self.tick(budget)?;
        if assignment.variables() < self.width {
            return Err(Incomplete::InvalidWitness);
        }
        if self.width == 0 {
            return Ok(self.entries == 0);
        }
        if self.nodes.is_empty() {
            return Ok(true);
        }
        let mut node = 0;
        for variable in 0..self.width {
            self.tick(budget)?;
            let value = usize::from(
                assignment
                    .value(variable)
                    .ok_or(Incomplete::InvalidWitness)?,
            );
            let Some(child) = self.nodes[node][value] else {
                return Ok(true);
            };
            node = child.index()?;
        }
        Ok(false)
    }
}

fn bound(resource: ProjectionResource, required: u128, limit: u128) -> Result<(), Incomplete> {
    if required > limit {
        Err(Incomplete::ProjectionLimit {
            resource,
            required,
            limit,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod limits;
