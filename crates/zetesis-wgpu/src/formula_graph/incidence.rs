//! Optional transpose from physical domain slots to their consuming gates.
//!
//! Both an operand and its gate's output affect a transfer. Atom occurrences
//! therefore use their shared physical slot, while distinct auxiliary outputs
//! keep their identity. Repeated incidences are harmless: atomic bit insertion
//! coalesces them. The directory is immutable and shared by every candidate;
//! two candidate-local generations prevent a racing wakeup from being lost.

use super::capacity;
use crate::GpuError;
use zetesis_cpu::Cancellation;

/// Appended CSR offsets and gate IDs, followed nowhere by candidate state.
/// `offset` is nonzero, leaving zero available for the dense execution route.
pub(crate) struct Layout {
    pub(crate) offset: u32,
    pub(crate) words: u32,
    pub(crate) dirty_words: u32,
    pub(crate) setup_work: u32,
    pub(crate) sweep_work: u32,
    variables: u32,
    entries: u32,
}

fn poll(cancellation: &Cancellation) -> Result<(), GpuError> {
    cancellation.poll().map_err(GpuError::interrupted)
}

impl Layout {
    /// Count the admitted physical graph without allocating a second owner.
    /// No incidences means there is no gate work for the optional index to save.
    pub(crate) fn new(
        nodes: &[u32],
        node_count: u32,
        variables: u32,
        offset: u32,
        cancellation: &Cancellation,
    ) -> Result<Option<Self>, GpuError> {
        let mut entries = 0_u32;
        for node in nodes.chunks_exact(4).take(node_count as usize) {
            poll(cancellation)?;
            let operands = match node[0] {
                0 | 1 => continue,
                2..=4 => 2,
                5..=9 => node[2],
                10 => 0,
                _ => return Err(capacity("invalid formula incidence operation")),
            };
            entries = entries
                .checked_add(operands)
                .and_then(|n| n.checked_add(1))
                .ok_or_else(|| capacity("formula incidence count overflows"))?;
        }
        if entries == 0 {
            return Ok(None);
        }
        if offset == 0 {
            return Err(capacity(
                "formula incidence directory overlaps dense marker",
            ));
        }
        let words = variables
            .checked_add(1)
            .and_then(|n| n.checked_add(entries))
            .ok_or_else(|| capacity("formula incidence directory overflows"))?;
        offset
            .checked_add(words)
            .filter(|end| *end <= u32::MAX - 64)
            .ok_or_else(|| capacity("formula incidence address overflows"))?;
        let dirty_words = node_count.div_ceil(32);
        let setup_work = dirty_words
            .checked_mul(2)
            .ok_or_else(|| capacity("formula dirty initialization work overflows"))?;
        // Per sweep: clear the consumed generation, test every gate, and bound
        // all wakes. A two-bit domain shrinks at most twice; each wake reads
        // two endpoints, then one ID and two atomic bit words per incidence.
        let sweep_work = variables
            .checked_mul(4)
            .and_then(|n| entries.checked_mul(6).and_then(|e| n.checked_add(e)))
            .and_then(|n| n.checked_add(node_count))
            .and_then(|n| n.checked_add(dirty_words))
            .ok_or_else(|| capacity("formula dirty sweep work overflows"))?;
        Ok(Some(Self {
            offset,
            words,
            dirty_words,
            setup_work,
            sweep_work,
            variables,
            entries,
        }))
    }

    pub(crate) fn domain_words(&self) -> Result<u32, GpuError> {
        self.dirty_words
            .checked_mul(2)
            .and_then(|n| n.checked_add(self.variables))
            .filter(|n| *n <= u32::MAX - 64)
            .ok_or_else(|| capacity("formula dirty domain addresses overflow"))
    }

    /// Build the transpose in its reserved final storage. Counts become ends,
    /// then starts during reverse insertion; no temporary cursor vector exists.
    pub(crate) fn pack(
        &self,
        nodes: &[u32],
        node_count: u32,
        roots: &mut Vec<u32>,
        cancellation: &Cancellation,
    ) -> Result<(), GpuError> {
        let offset = self.offset as usize;
        let end = offset + self.words as usize;
        if roots.len() != offset || roots.capacity() < end {
            return Err(capacity(
                "formula incidence storage differs from reservation",
            ));
        }
        roots.resize(end, 0);
        let order = offset + self.variables as usize + 1;
        visit(
            nodes,
            node_count,
            self.variables,
            false,
            cancellation,
            |_, slot| {
                let count = &mut roots[offset + slot as usize + 1];
                *count = count
                    .checked_add(1)
                    .ok_or_else(|| capacity("formula slot incidence count overflows"))?;
                Ok(())
            },
        )?;
        for slot in 1..=self.variables as usize {
            poll(cancellation)?;
            roots[offset + slot] = roots[offset + slot]
                .checked_add(roots[offset + slot - 1])
                .ok_or_else(|| capacity("formula incidence prefix overflows"))?;
        }
        if roots[order - 1] != self.entries {
            return Err(capacity("formula incidence coverage differs"));
        }
        visit(
            nodes,
            node_count,
            self.variables,
            true,
            cancellation,
            |gate, slot| {
                let end = &mut roots[offset + slot as usize + 1];
                let position = super::dependency::insert_position(end, self.entries)?;
                roots[order + position] = gate;
                Ok(())
            },
        )?;
        roots[offset..order].rotate_left(1);
        roots[order - 1] = self.entries;
        Ok(())
    }
}

fn visit(
    nodes: &[u32],
    node_count: u32,
    variables: u32,
    reverse: bool,
    cancellation: &Cancellation,
    mut insert: impl FnMut(u32, u32) -> Result<(), GpuError>,
) -> Result<(), GpuError> {
    for position in 0..node_count {
        poll(cancellation)?;
        let gate = if reverse {
            node_count - position - 1
        } else {
            position
        };
        let node = &nodes[gate as usize * 4..gate as usize * 4 + 4];
        if node[0] == 0 || node[0] == 1 {
            continue;
        }
        let mut slot = |value| {
            if value >= variables {
                return Err(capacity("formula incidence slot exceeds domains"));
            }
            insert(gate, value)
        };
        slot(node[3])?;
        match node[0] {
            2..=4 => {
                slot(nodes[node[1] as usize * 4 + 3])?;
                slot(nodes[node[2] as usize * 4 + 3])?;
            }
            5..=9 => {
                let tail = node_count as usize * 4;
                let row = if node[0] >= 7 {
                    nodes[tail + node[1] as usize] + 3
                } else {
                    node[1]
                };
                for position in 0..node[2] as usize {
                    poll(cancellation)?;
                    let child = nodes[tail + row as usize + position];
                    slot(nodes[child as usize * 4 + 3])?;
                }
            }
            10 => {}
            _ => return Err(capacity("invalid formula incidence operation")),
        }
    }
    Ok(())
}
