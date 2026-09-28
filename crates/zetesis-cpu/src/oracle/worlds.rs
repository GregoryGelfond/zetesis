//! Immutable world membership over canonically ordered source rows.
//!
//! Atom-major masks are independent of the mutable demanded catalog and its
//! packed stride. A join frame intersects only complete round-snapshot truth.
//! Frozen seeds and pending consequences never enter this representation.

use zetesis_core::catalog::{AtomRef, Atoms, PredicateRef};
use zetesis_core::{Program, TemplateRef};

use super::Work;
use crate::Stop;

#[cfg(test)]
mod tests;

/// Canonical local IDs select round truth while borrowing the committed owner.
/// The ID vector owns no Atom and can outlive mutation of an independent append
/// tail or packed transport. Its retained capacity remains live during callbacks.
pub(crate) struct Rows<'a> {
    atoms: Atoms<'a>,
    positions: Vec<usize>,
    bytes: usize,
}

impl<'a> Rows<'a> {
    pub(crate) fn select(
        atoms: Atoms<'a>,
        mut positions: Vec<usize>,
        snapshots: &[u32],
        stride: usize,
        worlds: usize,
        available: usize,
        work: &mut Work<'_>,
    ) -> Result<Self, Stop> {
        let bytes = positions
            .capacity()
            .checked_mul(size_of::<usize>())
            .ok_or(Stop::Allocation)?;
        work.mask_bytes = work.mask_bytes.checked_add(bytes).ok_or(Stop::Allocation)?;
        if bytes > available {
            return Err(Stop::Allocation);
        }
        if worlds == 0 || stride == 0 || stride.checked_mul(worlds) != Some(snapshots.len()) {
            return Err(Stop::InvalidProgram);
        }
        let mut selected = 0;
        for cursor in 0..positions.len() {
            work.tick()?;
            let id = positions[cursor];
            if id >= atoms.len() || id / 32 >= stride {
                return Err(Stop::InvalidProgram);
            }
            let mut included = false;
            for world in 0..worlds {
                work.mask_word()?;
                if present(snapshots, stride, world, id) {
                    included = true;
                    break;
                }
            }
            if included {
                work.tick()?;
                positions[selected] = id;
                selected += 1;
            }
        }
        positions.truncate(selected);
        Ok(Self {
            atoms,
            positions,
            bytes,
        })
    }

    pub(crate) const fn bytes(&self) -> usize {
        self.bytes
    }

    pub(crate) fn iter(&self) -> impl ExactSizeIterator<Item = AtomRef<'a>> + Clone {
        self.positions
            .iter()
            .map(|&id| self.atoms.at(id).expect("selected source occurrence"))
    }
}

pub(crate) struct Snapshot<'a> {
    rows: Rows<'a>,
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

    /// A reused owner is live before any fallible round preparation starts.
    pub(crate) fn record_retained(&self, work: &mut Work<'_>) {
        work.mask_bytes = self.bytes;
    }

    /// Rebuild membership over selected committed rows, including existing atom
    /// identities that have gained worlds. `available` includes the input ID
    /// vector and this workspace's live bytes. No symbolic atom is copied.
    pub(crate) fn snapshot<'a>(
        self,
        rows: Rows<'a>,
        snapshots: &[u32],
        stride: usize,
        available: usize,
        work: &mut Work<'_>,
    ) -> Result<Snapshot<'a>, Stop> {
        work.mask_bytes = rows.bytes.checked_add(self.bytes).ok_or(Stop::Allocation)?;
        if stride == 0 || stride.checked_mul(self.worlds) != Some(snapshots.len()) {
            return Err(Stop::InvalidProgram);
        }
        let matrix = rows
            .positions
            .len()
            .checked_mul(self.words)
            .ok_or(Stop::Allocation)?;
        let bytes = matrix
            .checked_mul(size_of::<u32>())
            .and_then(|bytes| bytes.checked_add(self.bytes))
            .and_then(|bytes| bytes.checked_add(rows.bytes))
            .ok_or(Stop::Allocation)?;
        if bytes > available {
            return Err(Stop::Allocation);
        }
        let mut membership = zeros(matrix, work)?;
        for (row, &id) in rows.positions.iter().enumerate() {
            for world in 0..self.worlds {
                work.mask_word()?;
                if present(snapshots, stride, world, id) {
                    work.mask_word()?;
                    let entry = membership
                        .get_mut(row * self.words + world / 32)
                        .ok_or(Stop::InvalidProgram)?;
                    *entry |= 1 << (world % 32);
                }
            }
        }
        Ok(Snapshot {
            rows,
            membership,
            workspace: self,
            bytes,
        })
    }
}

impl Snapshot<'_> {
    pub(crate) const fn bytes(&self) -> usize {
        self.bytes
    }

    /// Drop this round's selected IDs and membership, retaining only join storage.
    pub(crate) fn into_workspace(self) -> Workspace {
        self.workspace
    }

    pub(crate) fn parts(
        &mut self,
    ) -> (impl ExactSizeIterator<Item = AtomRef<'_>> + Clone, Join<'_>) {
        (
            self.rows.iter(),
            Join {
                atoms: self.rows.atoms,
                positions: &self.rows.positions,
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

fn present(snapshots: &[u32], stride: usize, world: usize, atom: usize) -> bool {
    snapshots[world * stride + atom / 32] & (1 << (atom % 32)) != 0
}

/// Frame d describes the conjunction of the first d chosen positive rows.
/// The iterative source visitor owns bindings/cursors; this borrow owns only
/// their necessary world-membership condition and never establishes gate truth.
pub(crate) struct Join<'a> {
    atoms: Atoms<'a>,
    positions: &'a [usize],
    membership: &'a [u32],
    frames: &'a mut [u32],
    starts: &'a mut [Option<usize>],
    worlds: usize,
    words: usize,
}

impl Join<'_> {
    pub(super) fn reset(
        &mut self,
        template: TemplateRef<'_>,
        work: &mut Work<'_>,
    ) -> Result<(), Stop> {
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

    fn row_start(
        &self,
        predicate: PredicateRef<'_>,
        work: &mut Work<'_>,
    ) -> Result<Option<usize>, Stop> {
        let mut start = 0;
        let mut end = self.positions.len();
        while start < end {
            work.tick()?;
            let middle = start + (end - start) / 2;
            let actual = self
                .atoms
                .at(self.positions[middle])
                .ok_or(Stop::InvalidProgram)?
                .predicate();
            if actual.compare_ref_with(predicate, || work.tick())?.is_lt() {
                start = middle + 1;
            } else {
                end = middle;
            }
        }
        work.tick()?;
        let Some(&id) = self.positions.get(start) else {
            return Ok(None);
        };
        let actual = self.atoms.at(id).ok_or(Stop::InvalidProgram)?.predicate();
        Ok(actual
            .equals_ref_with(predicate, || work.tick())?
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
