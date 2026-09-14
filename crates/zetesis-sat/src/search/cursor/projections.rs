//! Exact complete semantic projections, indexed by a bounded binary trie.

use std::num::NonZeroU32;

use crate::search::Budget;
use crate::{AdmissionError, Assignment, Incomplete};

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
    empty_blocked: bool,
}
impl Projections {
    pub(super) fn new(width: usize) -> Self {
        Self {
            width,
            nodes: Vec::new(),
            empty_blocked: false,
        }
    }

    /// The caller has admitted one logical exclusion of exactly this width.
    /// Every complete-depth path is one excluded projection; intermediate
    /// prefixes are not keys. Only a fully constructed suffix is attached.
    pub(super) fn insert(
        &mut self,
        width: usize,
        value: impl Fn(usize) -> bool,
        budget: &mut Budget<'_>,
    ) -> Result<(), Incomplete> {
        budget.tick()?;
        if width != self.width {
            return Err(Incomplete::InvalidWitness);
        }
        if width == 0 {
            self.empty_blocked = true;
            return Ok(());
        }
        if self.nodes.is_empty() {
            self.suffix(0, &value, budget)?;
            return Ok(());
        }
        let mut node = 0;
        for variable in 0..width {
            budget.tick()?;
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

    // At most one node per admitted logical exclusion bit plus one root.
    // Request room for the missing suffix, not another full key or clause.
    // Amortized reservation avoids copying the arena for each inserted key.
    // Shape bounds count nodes; spare capacity/allocator rounding remain
    // outside admission units, which are not an allocated-byte ceiling.
    fn suffix(
        &mut self,
        depth: usize,
        value: &impl Fn(usize) -> bool,
        budget: &mut Budget<'_>,
    ) -> Result<NodeId, Incomplete> {
        let start = self.nodes.len();
        let additional = suffix_size(self.width, depth, start)?;
        let first = NodeId::new(start)?;
        self.nodes
            .try_reserve(additional)
            .map_err(|_| Incomplete::Allocation)?;
        let result = (|| {
            for variable in depth..self.width {
                budget.tick()?;
                let mut children = [None, None];
                // The preflight covers this forward link and the terminal node.
                children[usize::from(value(variable))] = Some(NodeId::new(self.nodes.len() + 1)?);
                self.nodes.push(children);
            }
            budget.tick()?;
            self.nodes.push([None, None]);
            Ok(first)
        })();
        if result.is_err() {
            self.nodes.truncate(start);
        }
        result
    }

    pub(super) fn permits(
        &self,
        assignment: &Assignment,
        budget: &mut Budget<'_>,
    ) -> Result<bool, Incomplete> {
        budget.tick()?;
        if assignment.variables() < self.width {
            return Err(Incomplete::InvalidWitness);
        }
        if self.width == 0 {
            return Ok(!self.empty_blocked);
        }
        if self.nodes.is_empty() {
            return Ok(true);
        }
        let mut node = 0;
        for variable in 0..self.width {
            budget.tick()?;
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

#[cfg(test)]
mod tests;
