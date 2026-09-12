//! Immutable world membership over canonically ordered source rows.
//!
//! Atom-major masks are independent of the mutable demanded catalog and its
//! packed stride. A join frame intersects only complete round-snapshot truth.
//! Frozen seeds and pending consequences never enter this representation.

use std::collections::BTreeMap;

use zetesis_core::{Atom, Predicate, Program, Template};

use super::Work;
use crate::Stop;

#[cfg(test)]
#[path = "../../tests/support/world_membership.rs"]
mod tests;

pub(crate) struct Snapshot {
    atoms: Vec<Atom>,
    membership: Vec<u32>,
    workspace: Workspace,
    bytes: usize,
}

/// Batch-owned join storage. Shape is fixed by the program and candidate
/// occurrences; contents carry no truth across source scans. A template reset
/// replaces the root and predicate starts, and extension replaces each child
/// before the source visitor can use that child as a parent.
pub(crate) struct Workspace {
    frames: Vec<u32>,
    starts: Vec<Option<usize>>,
    worlds: usize,
    words: usize,
    bytes: usize,
}

impl Workspace {
    /// Reserve the fixed join shape once per batch. Retained payload must remain
    /// charged between scans and throughout any demanded-catalog growth.
    pub(crate) fn new(
        program: &Program,
        worlds: usize,
        available: usize,
        work: &mut Work<'_>,
    ) -> Result<Self, Stop> {
        if worlds == 0 {
            return Err(Stop::InvalidProgram);
        }
        let mut depth = 0;
        for template in program.templates() {
            work.tick()?;
            depth = depth.max(template.positive().len());
        }
        let mask_width = worlds.div_ceil(32);
        let frame = depth
            .checked_add(1)
            .and_then(|rows| rows.checked_mul(mask_width))
            .ok_or(Stop::Allocation)?;
        let bytes = frame
            .checked_mul(size_of::<u32>())
            .and_then(|bytes| {
                depth
                    .checked_mul(size_of::<Option<usize>>())
                    .and_then(|indices| bytes.checked_add(indices))
            })
            .ok_or(Stop::Allocation)?;
        if bytes > available {
            return Err(Stop::Allocation);
        }
        let frames = zeros(frame, work)?;
        let mut starts = Vec::new();
        starts
            .try_reserve_exact(depth)
            .map_err(|_| Stop::Allocation)?;
        work.mask_bytes += depth * size_of::<Option<usize>>();
        for _ in 0..depth {
            work.tick()?;
            starts.push(None);
        }
        Ok(Self {
            frames,
            starts,
            worlds,
            words: mask_width,
            bytes,
        })
    }

    pub(crate) const fn bytes(&self) -> usize {
        self.bytes
    }

    /// Rebuild membership from complete current snapshots, including existing
    /// atom identities that have gained worlds. The one symbolic union copy is
    /// reserved independently. `available` includes this workspace's live bytes.
    pub(crate) fn snapshot(
        self,
        catalog: &BTreeMap<Atom, u32>,
        snapshots: &[u32],
        stride: usize,
        available: usize,
        work: &mut Work<'_>,
    ) -> Result<Snapshot, Stop> {
        work.mask_bytes = self.bytes;
        if stride == 0 || stride.checked_mul(self.worlds) != Some(snapshots.len()) {
            return Err(Stop::InvalidProgram);
        }
        let mut rows = 0usize;
        for id in catalog.values() {
            if *id as usize / 32 >= stride {
                return Err(Stop::InvalidProgram);
            }
            for world in 0..self.worlds {
                work.mask_word()?;
                if present(snapshots, stride, world, *id) {
                    rows = rows.checked_add(1).ok_or(Stop::Allocation)?;
                    break;
                }
            }
        }
        let matrix = rows.checked_mul(self.words).ok_or(Stop::Allocation)?;
        let bytes = matrix
            .checked_mul(size_of::<u32>())
            .and_then(|bytes| bytes.checked_add(self.bytes))
            .ok_or(Stop::Allocation)?;
        if bytes > available {
            return Err(Stop::Allocation);
        }
        let mut atoms = Vec::new();
        atoms
            .try_reserve_exact(rows)
            .map_err(|_| Stop::Allocation)?;
        let mut membership = zeros(matrix, work)?;
        for (atom, id) in catalog {
            let row = atoms.len();
            let mut included = false;
            for world in 0..self.worlds {
                work.mask_word()?;
                if present(snapshots, stride, world, *id) {
                    work.mask_word()?;
                    let entry = membership
                        .get_mut(row * self.words + world / 32)
                        .ok_or(Stop::InvalidProgram)?;
                    *entry |= 1 << (world % 32);
                    included = true;
                }
            }
            if included {
                atoms.push(atom.clone());
            }
        }
        if atoms.len() != rows {
            return Err(Stop::InvalidProgram);
        }
        Ok(Snapshot {
            atoms,
            membership,
            workspace: self,
            bytes,
        })
    }
}

impl Snapshot {
    pub(crate) const fn bytes(&self) -> usize {
        self.bytes
    }

    /// Drop this round's atoms and membership before retaining only join storage.
    pub(crate) fn into_workspace(self) -> Workspace {
        self.workspace
    }

    pub(crate) fn parts(&mut self) -> (&[Atom], Join<'_>) {
        (
            &self.atoms,
            Join {
                atoms: &self.atoms,
                membership: &self.membership,
                frames: &mut self.workspace.frames,
                starts: &mut self.workspace.starts,
                worlds: self.workspace.worlds,
                words: self.workspace.words,
            },
        )
    }
}

fn zeros(count: usize, work: &mut Work<'_>) -> Result<Vec<u32>, Stop> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Stop::Allocation)?;
    work.mask_bytes += count * size_of::<u32>();
    for _ in 0..count {
        work.mask_word()?;
        values.push(0);
    }
    Ok(values)
}

fn present(snapshots: &[u32], stride: usize, world: usize, atom: u32) -> bool {
    snapshots[world * stride + atom as usize / 32] & (1 << (atom % 32)) != 0
}

/// Frame d describes the conjunction of the first d chosen positive rows.
/// The iterative source visitor owns bindings/cursors; this borrow owns only
/// their necessary world-membership condition and never establishes gate truth.
pub(crate) struct Join<'a> {
    atoms: &'a [Atom],
    membership: &'a [u32],
    frames: &'a mut [u32],
    starts: &'a mut [Option<usize>],
    worlds: usize,
    words: usize,
}

impl Join<'_> {
    pub(super) fn reset(&mut self, template: &Template, work: &mut Work<'_>) -> Result<(), Stop> {
        for (index, value) in self.frames[..self.words].iter_mut().enumerate() {
            work.mask_word()?;
            let remaining = self.worlds - index * 32;
            *value = if remaining >= 32 {
                u32::MAX
            } else {
                (1 << remaining) - 1
            };
        }
        for (depth, pattern) in template.positive().iter().enumerate() {
            self.starts[depth] = self.row_start(pattern.predicate(), work)?;
        }
        Ok(())
    }

    fn row_start(&self, predicate: &Predicate, work: &mut Work<'_>) -> Result<Option<usize>, Stop> {
        let mut start = 0;
        let mut end = self.atoms.len();
        while start < end {
            work.tick()?;
            let middle = start + (end - start) / 2;
            let actual = self.atoms[middle].predicate();
            for name in [actual.name(), predicate.name()] {
                work.charge(name.len())?;
            }
            if actual < predicate {
                start = middle + 1;
            } else {
                end = middle;
            }
        }
        work.tick()?;
        Ok(self
            .atoms
            .get(start)
            .is_some_and(|atom| atom.predicate() == predicate)
            .then_some(start))
    }

    pub(super) fn extend(
        &mut self,
        depth: usize,
        tuple: usize,
        work: &mut Work<'_>,
    ) -> Result<bool, Stop> {
        let row = self.starts[depth]
            .and_then(|start| start.checked_add(tuple))
            .ok_or(Stop::InvalidProgram)?;
        let membership = self
            .membership
            .get(row * self.words..)
            .and_then(|words| words.get(..self.words))
            .ok_or(Stop::InvalidProgram)?;
        let (parents, children) = self.frames.split_at_mut((depth + 1) * self.words);
        let parent = &parents[depth * self.words..][..self.words];
        let child = &mut children[..self.words];
        let mut nonempty = false;
        for ((previous, present), next) in parent.iter().zip(membership).zip(child) {
            work.mask_word()?;
            *next = previous & present;
            nonempty |= *next != 0;
        }
        if !nonempty {
            work.pruned_prefixes += 1;
        }
        Ok(nonempty)
    }
}
