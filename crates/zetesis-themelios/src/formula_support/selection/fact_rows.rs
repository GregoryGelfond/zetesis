//! The atoms of a program's fact rows, kept in row order.
//!
//! Emission names each fact row's atom twice: once when it collects the fixed
//! facts and once when it emits the row's root. The first pass keeps each
//! row's source position here, block by block, in item order. The second pass
//! visits the same blocks in the same order and reads each row's atom back by
//! its position instead of interning the row again. A block that is not the
//! next recorded one is interned again by its reader. When support completion
//! recorded the rows in its first round, the first pass copies that record
//! instead of interning at all.
//!
//! R rows in B blocks retain R + 2B cells under the selection's lease and
//! cost one charged unit per row to record and one to read back.

use super::{Computation, Counters, SourceAtom, SourceSelection, owner_failure};
use crate::formula_facts::FactBlock;
use crate::formula_support::{Context, reserve};
use crate::{FormulaFailure, FormulaLimits, ProgramSite};

pub(crate) struct FactAtoms {
    rows: SourceSelection,
    /// Each recorded block's identity and its first row's index in `rows`.
    blocks: Vec<(usize, usize)>,
    /// The next block a reader may take.
    next: usize,
}

/// One recorded block's rows, read back in row order.
pub(crate) struct FactBlockAtoms<'a> {
    rows: &'a SourceSelection,
    range: std::ops::Range<usize>,
}

impl FactAtoms {
    pub(crate) fn new(
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &Counters,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        Ok(Self {
            rows: SourceSelection::new(computation, limits, counters, location)?,
            blocks: Vec::new(),
            next: 0,
        })
    }

    /// The atoms support completion recorded for `blocks`, the program's
    /// fact blocks in item order, when `computation` appends to that support's
    /// catalog and the record holds exactly their rows; otherwise none. One
    /// unit per row copied.
    pub(crate) fn recorded<'b>(
        blocks: impl Iterator<Item = FactBlock<'b>>,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Option<Self>, FormulaFailure> {
        counters.work(limits, location)?;
        let rows = computation.recorded_fact_rows();
        if rows.is_empty() {
            return Ok(None);
        }
        let mut atoms = Self::new(computation, limits, counters, location)?;
        for block in blocks {
            atoms.begin(block, computation, limits, counters, location)?;
            let start = atoms.rows.positions.len();
            let Some(cells) = start
                .checked_add(block.len())
                .and_then(|end| rows.get(start..end))
            else {
                return Ok(None);
            };
            for &position in cells {
                counters.work(limits, location)?;
                atoms.rows.positions.push(position);
            }
        }
        Ok((atoms.rows.positions.len() == rows.len()).then_some(atoms))
    }

    /// Read the record again from its first block.
    pub(crate) fn rewind(&mut self) {
        self.next = 0;
    }

    fn other_bytes(&self) -> usize {
        size_of::<Self>()
            + self.rows.positions.capacity() * size_of::<usize>()
            + self.blocks.capacity() * size_of::<(usize, usize)>()
    }

    /// Open `block`'s record, reserving a cell for each of its rows.
    pub(crate) fn begin(
        &mut self,
        block: FactBlock<'_>,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        counters.work(limits, location)?;
        let other = self.other_bytes() - self.blocks.capacity() * size_of::<(usize, usize)>();
        reserve(
            &mut self.blocks,
            1,
            &mut self.rows.lease,
            other,
            Context::new(computation, limits, counters, location),
        )?;
        let other = self.other_bytes() - self.rows.positions.capacity() * size_of::<usize>();
        reserve(
            &mut self.rows.positions,
            block.len(),
            &mut self.rows.lease,
            other,
            Context::new(computation, limits, counters, location),
        )?;
        self.blocks.push((block.id(), self.rows.positions.len()));
        Ok(())
    }

    /// Record the open block's next row atom.
    pub(crate) fn push(
        &mut self,
        atom: &SourceAtom,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        counters.work(limits, location)?;
        if !self.rows.scope.same(&atom.scope) {
            return Err(owner_failure(location));
        }
        debug_assert!(self.rows.positions.len() < self.rows.positions.capacity());
        self.rows.positions.push(atom.position);
        Ok(())
    }

    /// The recorded rows of `block` when it is the next recorded block, of
    /// the same scope as `computation`, with every row recorded.
    pub(crate) fn take(
        &mut self,
        block: FactBlock<'_>,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Option<FactBlockAtoms<'_>>, FormulaFailure> {
        counters.work(limits, location)?;
        let Some(&(id, start)) = self.blocks.get(self.next) else {
            return Ok(None);
        };
        let end = self
            .blocks
            .get(self.next + 1)
            .map_or(self.rows.positions.len(), |&(_, start)| start);
        if id != block.id()
            || end - start != block.len()
            || !self.rows.scope.same(&computation.source_scope(location)?)
        {
            return Ok(None);
        }
        self.next += 1;
        Ok(Some(FactBlockAtoms {
            rows: &self.rows,
            range: start..end,
        }))
    }
}

impl FactBlockAtoms<'_> {
    /// The next row's atom, one unit each.
    pub(crate) fn next(
        &mut self,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<SourceAtom, FormulaFailure> {
        counters.work(limits, location)?;
        let index = self.range.next().expect("one recorded atom per row");
        Ok(SourceAtom {
            scope: self.rows.scope.clone(),
            position: self.rows.positions[index],
        })
    }
}
