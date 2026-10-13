//! Who reads each node's truth: the reverse of a theory's formula DAG.
//!
//! A complete evaluation of one candidate fixes every node's truth. When a
//! later candidate changes some atoms, only a reader of a changed value can
//! change in turn. For each node this index lists the conjunctions,
//! disjunctions and implications with it as an operand, the numeric rows
//! with it as a condition, the producers with it as a body and the original
//! root positions holding it; it also lists each atom's atom nodes and each
//! row's guards. It is immutable, belongs to one plan, and every workspace
//! checking under that plan shares it.

use std::mem::size_of;

use super::{Tick, TightError, TightProducer, filled};
use crate::{NodeView, Theory};
use zetesis_cpu::{Meter, Stop};

/// What a reader is: the kind of the reading object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReaderKind {
    /// A conjunction, disjunction or implication reading the node as an operand.
    Node,
    /// A numeric row reading the node as a condition; its guards read the row.
    Row,
    /// A producer whose body is the node.
    Producer,
    /// An original root position holding the node.
    Root,
}

/// One reader, packed as its kind in the top two bits over its index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Reader(u64);

impl Reader {
    const SHIFT: u32 = u64::BITS - 2;

    fn new(kind: ReaderKind, index: usize) -> Result<Self, TightError> {
        let index = u64::try_from(index).map_err(|_| Stop::InvalidProgram)?;
        if index >> Self::SHIFT != 0 {
            return Err(Stop::InvalidProgram.into());
        }
        let tag = match kind {
            ReaderKind::Node => 0,
            ReaderKind::Row => 1,
            ReaderKind::Producer => 2,
            ReaderKind::Root => 3,
        };
        Ok(Self(tag << Self::SHIFT | index))
    }

    pub(super) fn kind(self) -> ReaderKind {
        match self.0 >> Self::SHIFT {
            0 => ReaderKind::Node,
            1 => ReaderKind::Row,
            2 => ReaderKind::Producer,
            _ => ReaderKind::Root,
        }
    }

    pub(super) fn index(self) -> usize {
        // The index fitted a usize when it was packed.
        usize::try_from(self.0 & ((1 << Self::SHIFT) - 1)).unwrap_or(usize::MAX)
    }
}

/// Ordered lists over one entry vector, one per slot: list `s` is
/// `entries[offsets[s]..offsets[s + 1]]`. A slot is a node, an atom or a numeric
/// row, by relation.
#[derive(Debug)]
struct Lists<T> {
    offsets: Vec<usize>,
    entries: Vec<T>,
}

impl<T: Copy> Lists<T> {
    fn list(&self, slot: usize) -> &[T] {
        &self.entries[self.offsets[slot]..self.offsets[slot + 1]]
    }

    /// Logical payload: offsets and entries by length.
    fn bytes(&self) -> u128 {
        self.offsets.len() as u128 * size_of::<usize>() as u128
            + self.entries.len() as u128 * size_of::<T>() as u128
    }
}

/// The readers of every node, the atom nodes of every atom and the guards of
/// every numeric row of one theory, for one producer list.
#[derive(Debug)]
pub(super) struct Readers {
    of_nodes: Lists<Reader>,
    atom_nodes: Lists<usize>,
    guards: Lists<usize>,
}

impl Readers {
    /// Readers of `node`'s truth, in node, row, producer and root order of discovery.
    pub(super) fn of(&self, node: usize) -> &[Reader] {
        self.of_nodes.list(node)
    }

    /// The atom nodes reading `atom`'s membership.
    pub(super) fn atom_nodes(&self, atom: usize) -> &[usize] {
        self.atom_nodes.list(atom)
    }

    /// The guard nodes reading `row`'s measure.
    pub(super) fn guards(&self, row: usize) -> &[usize] {
        self.guards.list(row)
    }

    /// Reader entries over all nodes.
    pub(super) fn reader_count(&self) -> usize {
        self.of_nodes.entries.len()
    }

    /// Atom-node entries over all atoms.
    pub(super) fn atom_node_count(&self) -> usize {
        self.atom_nodes.entries.len()
    }

    /// Retained logical payload: offsets and entries of the three relations.
    pub(super) fn bytes(&self) -> u128 {
        self.of_nodes.bytes() + self.atom_nodes.bytes() + self.guards.bytes()
    }

    /// A bound on the logical bytes building the index holds at once,
    /// computable from the theory before the build: one count, offset and
    /// cursor per node, atom and numeric row and one more offset per relation;
    /// reader entries bounded by operand occurrences, producers and roots
    /// (each is an operand, a row condition listed once, a producer body or a
    /// root position); atom-node and guard entries together by nodes (each is
    /// one node); and one row mark per 64 rows.
    pub(super) fn byte_bound(theory: &Theory, producers: &[TightProducer]) -> u128 {
        let (nodes, rows) = (theory.nodes().len(), theory.parts().rows().len());
        let slots = (nodes + theory.atom_count() + rows) as u128;
        let word = size_of::<usize>() as u128;
        let readers =
            (theory.parts().occurrences() + producers.len() + theory.roots().len()) as u128;
        slots * word
            + (2 * slots + 3) * word
            + readers * size_of::<Reader>() as u128
            + nodes as u128 * word
            + rows.div_ceil(64) as u128 * size_of::<u64>() as u128
    }

    /// Build the index in two charged passes over the same incidence stream:
    /// one counts each relation's lists, the other places their entries. Each
    /// pass visits every node and its operands or conditions once per row,
    /// every producer and every root position, so the work is linear in the
    /// theory. The caller admits `base` plus [`Self::byte_bound`] before the
    /// build; the counted figure lies within that bound, and an excess is an
    /// invariant failure, not a refusal. Returns the index and the counted
    /// bound on the logical bytes the build holds at once, `base` included.
    pub(super) fn build(
        theory: &Theory,
        producers: &[TightProducer],
        base: u128,
        meter: &mut Meter<'_>,
    ) -> Result<(Self, u64), TightError> {
        let nodes = theory.nodes().len();
        let atoms = theory.atom_count();
        let rows = theory.parts().rows().len();
        let counts = (nodes + atoms + rows) as u128 * size_of::<usize>() as u128;
        let marks = rows.div_ceil(64) as u128 * size_of::<u64>() as u128;
        let mut readers = Counted::new(nodes)?;
        let mut atom_nodes = Counted::new(atoms)?;
        let mut guards = Counted::new(rows)?;
        incidences(theory, producers, meter, |incidence| match incidence {
            Incidence::Reader(node, _) => readers.count(node),
            Incidence::AtomNode(atom, _) => atom_nodes.count(atom),
            Incidence::Guard(row, _) => guards.count(row),
        })?;
        let placed = readers.placed_bytes::<Reader>()?
            + atom_nodes.placed_bytes::<usize>()?
            + guards.placed_bytes::<usize>()?;
        // The counted lists lie within the bound admitted before the build.
        let held = base + counts + marks + placed;
        if held > base + Self::byte_bound(theory, producers) {
            return Err(Stop::InvalidProgram.into());
        }
        let held = u64::try_from(held).map_err(|_| Stop::InvalidProgram)?;
        let mut readers = readers.placement(Reader(0))?;
        let mut atom_nodes = atom_nodes.placement(0)?;
        let mut guards = guards.placement(0)?;
        incidences(theory, producers, meter, |incidence| match incidence {
            Incidence::Reader(node, reader) => readers.place(node, reader),
            Incidence::AtomNode(atom, node) => atom_nodes.place(atom, node),
            Incidence::Guard(row, node) => guards.place(row, node),
        })?;
        let index = Self {
            of_nodes: readers.finish()?,
            atom_nodes: atom_nodes.finish()?,
            guards: guards.finish()?,
        };
        Ok((index, held))
    }
}

/// One incidence of the stream both passes read.
#[derive(Clone, Copy)]
enum Incidence {
    /// A reader of a node.
    Reader(usize, Reader),
    /// An atom node of an atom.
    AtomNode(usize, usize),
    /// A guard node of a numeric row.
    Guard(usize, usize),
}

/// Enumerate every incidence in a fixed order, charging each node, operand,
/// condition, producer and root position. A row's conditions are listed once,
/// with the row's first guard; every guard is listed with its row.
fn incidences(
    theory: &Theory,
    producers: &[TightProducer],
    meter: &mut Meter<'_>,
    mut emit: impl FnMut(Incidence) -> Result<(), TightError>,
) -> Result<(), TightError> {
    let view = theory.view();
    let mut seen = filled(theory.parts().rows().len().div_ceil(64), 0_u64)?;
    for index in 0..view.len() {
        meter.tick()?;
        let node = view.node(index).map_err(|_| Stop::InvalidProgram)?;
        match node {
            NodeView::Atom(atom) => emit(Incidence::AtomNode(atom, index))?,
            NodeView::False => {}
            NodeView::And(operands) | NodeView::Or(operands) => {
                for &operand in operands {
                    meter.tick()?;
                    emit(Incidence::Reader(
                        operand,
                        Reader::new(ReaderKind::Node, index)?,
                    ))?;
                }
            }
            NodeView::Implies(antecedent, consequent) => {
                for operand in [antecedent, consequent] {
                    meter.tick()?;
                    emit(Incidence::Reader(
                        operand,
                        Reader::new(ReaderKind::Node, index)?,
                    ))?;
                }
            }
            NodeView::Threshold(threshold) => {
                let row = numeric_row(theory, index)?;
                emit(Incidence::Guard(row, index))?;
                if first(&mut seen, row)? {
                    for &condition in threshold.conditions {
                        meter.tick()?;
                        emit(Incidence::Reader(
                            condition,
                            Reader::new(ReaderKind::Row, row)?,
                        ))?;
                    }
                }
            }
            NodeView::ExtremumCut(cut) => {
                let row = numeric_row(theory, index)?;
                emit(Incidence::Guard(row, index))?;
                if first(&mut seen, row)? {
                    for &condition in cut.row.conditions {
                        meter.tick()?;
                        emit(Incidence::Reader(
                            condition,
                            Reader::new(ReaderKind::Row, row)?,
                        ))?;
                    }
                }
            }
        }
    }
    for (index, producer) in producers.iter().enumerate() {
        meter.tick()?;
        if let Some(body) = producer.body {
            emit(Incidence::Reader(
                body,
                Reader::new(ReaderKind::Producer, index)?,
            ))?;
        }
    }
    for (position, &root) in theory.roots().iter().enumerate() {
        meter.tick()?;
        emit(Incidence::Reader(
            root,
            Reader::new(ReaderKind::Root, position)?,
        ))?;
    }
    Ok(())
}

/// The numeric row a threshold or extremum guard reads.
pub(super) fn numeric_row(theory: &Theory, node: usize) -> Result<usize, TightError> {
    let node_value = theory.nodes().get(node).ok_or(Stop::InvalidProgram)?;
    let parts = theory.parts();
    let row = if let Some(descriptor) = node_value.threshold_descriptor() {
        parts
            .thresholds()
            .get(descriptor)
            .map(|threshold| threshold.row)
    } else if let Some(descriptor) = node_value.extremum_cut_descriptor() {
        parts.extremum_cuts().get(descriptor).map(|cut| cut.row)
    } else {
        None
    };
    let row = row.ok_or(Stop::InvalidProgram)?;
    if row >= parts.rows().len() {
        return Err(Stop::InvalidProgram.into());
    }
    Ok(row)
}

/// Mark `row` seen, reporting whether this was its first sighting.
fn first(seen: &mut [u64], row: usize) -> Result<bool, TightError> {
    let word = seen.get_mut(row / 64).ok_or(Stop::InvalidProgram)?;
    let bit = 1_u64 << (row % 64);
    let fresh = *word & bit == 0;
    *word |= bit;
    Ok(fresh)
}

/// List lengths counted in the first pass, one per slot.
struct Counted {
    counts: Vec<usize>,
}

impl Counted {
    fn new(slots: usize) -> Result<Self, TightError> {
        Ok(Self {
            counts: filled(slots, 0)?,
        })
    }

    fn count(&mut self, slot: usize) -> Result<(), TightError> {
        let count = self.counts.get_mut(slot).ok_or(Stop::InvalidProgram)?;
        *count = count.checked_add(1).ok_or(Stop::InvalidProgram)?;
        Ok(())
    }

    fn total(&self) -> Result<usize, TightError> {
        self.counts
            .iter()
            .try_fold(0_usize, |sum, &count| sum.checked_add(count))
            .ok_or_else(|| Stop::InvalidProgram.into())
    }

    /// The bytes [`Self::placement`] allocates: offsets, cursors and entries.
    fn placed_bytes<T>(&self) -> Result<u128, TightError> {
        Ok(
            (2 * self.counts.len() as u128 + 1) * size_of::<usize>() as u128
                + self.total()? as u128 * size_of::<T>() as u128,
        )
    }

    /// Prefix sums become each list's start; entries are allocated once.
    fn placement<T: Copy>(self, blank: T) -> Result<Placement<T>, TightError> {
        let total = self.total()?;
        let mut offsets = filled(self.counts.len() + 1, 0)?;
        for (slot, &count) in self.counts.iter().enumerate() {
            offsets[slot + 1] = offsets[slot] + count;
        }
        let mut cursors = filled(self.counts.len(), 0)?;
        cursors.copy_from_slice(&offsets[..self.counts.len()]);
        Ok(Placement {
            offsets,
            cursors,
            entries: filled(total, blank)?,
        })
    }
}

/// Second-pass placement: each list fills its interval in stream order.
struct Placement<T> {
    offsets: Vec<usize>,
    cursors: Vec<usize>,
    entries: Vec<T>,
}

impl<T: Copy> Placement<T> {
    fn place(&mut self, slot: usize, entry: T) -> Result<(), TightError> {
        let cursor = self.cursors.get_mut(slot).ok_or(Stop::InvalidProgram)?;
        if *cursor >= self.offsets[slot + 1] {
            return Err(Stop::InvalidProgram.into());
        }
        self.entries[*cursor] = entry;
        *cursor += 1;
        Ok(())
    }

    /// Both passes read the same stream, so every interval is exactly full.
    fn finish(self) -> Result<Lists<T>, TightError> {
        if self
            .cursors
            .iter()
            .zip(&self.offsets[1..])
            .any(|(cursor, end)| cursor != end)
        {
            return Err(Stop::InvalidProgram.into());
        }
        Ok(Lists {
            offsets: self.offsets,
            entries: self.entries,
        })
    }
}
