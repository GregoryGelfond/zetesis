//! The prepared program as items: rules, and facts held as columnar products.
//!
//! A fact statement `p(t₁, …, tₙ).` expands to one product per argument
//! alternative (`fact_expansion`). Compilation stores each product in the fact
//! arena instead of building a rule with an empty body for each of its rows:
//! the product's predicate is admitted once, its columns hold admitted scalar
//! keys, and the statement's evidence is stored once. Consecutive one-row
//! statements of one predicate share a run: one row of cells and one evidence
//! range per statement.
//!
//! Readers traverse the prepared program as [`Item`]s: a rule, or a fact
//! product standing exactly where its fact rules stood, its rows in expansion
//! order (alternatives in order, last argument fastest). Every consumer that
//! visits the rules in order visits items in the same order, so wherever the
//! rule form discovered, staged, interned or emitted a fact rule's head, the
//! product's row is discovered, staged, interned or emitted at the same point.
//! A run stands for its statements' consecutive products; it changes storage,
//! not visit order.
//!
//! Storage is O(admitted cells + evidence + products + runs) and is reserved
//! fallibly. Reading a row allocates nothing: a product's cell is selected from
//! its column by the row's mixed-radix position.

use std::collections::TryReserveError;
use std::ops::Range;

use crate::ProgramSite;
use crate::formula_ir::RuleIr;
use crate::formula_support::StorageLease;
use crate::formula_support::components::{Predicate, Scalar};

/// The bytes one position of the item sequence retains.
pub(crate) const ITEM_BYTES: usize = size_of::<Slot>();

/// One position of the item sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    /// A rule, by its index among the rules.
    Rule(usize),
    /// A fact block, by its index in the arena.
    Facts(usize),
}

/// The prepared program in item order. The rules keep their relative order,
/// so a rule's index among the rules is stable for readers that never see
/// facts.
#[derive(Default)]
pub(crate) struct Items {
    rules: Vec<RuleIr>,
    facts: FactArena,
    order: Vec<Slot>,
}

/// One item of the prepared program.
#[derive(Clone, Copy)]
pub(crate) enum Item<'a> {
    /// A rule and its index among the rules.
    Rule(usize, &'a RuleIr),
    /// A fact product, standing where its fact rules stood.
    Facts(FactProduct<'a>),
}

impl Items {
    /// The number of items: rules plus stored fact blocks.
    pub(crate) fn len(&self) -> usize {
        self.order.len()
    }

    /// The item at `position`.
    ///
    /// # Panics
    /// Panics if `position` is not below [`Self::len`].
    pub(crate) fn get(&self, position: usize) -> Item<'_> {
        self.item(self.order[position])
    }

    fn item(&self, slot: Slot) -> Item<'_> {
        match slot {
            Slot::Rule(index) => Item::Rule(index, &self.rules[index]),
            Slot::Facts(block) => Item::Facts(FactProduct {
                arena: &self.facts,
                block,
            }),
        }
    }

    /// Every item, in order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = Item<'_>> + '_ {
        self.order.iter().map(|&slot| self.item(slot))
    }

    /// Every fact product, in item order.
    pub(crate) fn products(&self) -> impl Iterator<Item = FactProduct<'_>> + '_ {
        (0..self.facts.blocks.len()).map(|block| FactProduct {
            arena: &self.facts,
            block,
        })
    }

    /// The program's producer occurrences: its rules and its fact rows.
    pub(crate) fn occurrences(&self) -> usize {
        self.rules.len() + self.products().map(FactProduct::len).sum::<usize>()
    }

    /// The rules alone, in item order.
    pub(crate) fn rules(&self) -> &[RuleIr] {
        &self.rules
    }

    /// The rules alone, for in-place refinement of their compiled form.
    #[cfg(test)]
    pub(crate) fn rules_mut(&mut self) -> &mut [RuleIr] {
        &mut self.rules
    }

    /// Move the rules out. The fact products stay, and the rules' item
    /// positions no longer resolve; only a consumer done with the item
    /// sequence takes them.
    pub(crate) fn take_rules(&mut self) -> Vec<RuleIr> {
        self.order.retain(|slot| matches!(slot, Slot::Facts(_)));
        std::mem::take(&mut self.rules)
    }

    /// Record the rules appended to [`Self::rules_vec`] since there were
    /// `start` rules, as the next items.
    pub(crate) fn note_rules(&mut self, start: usize) {
        self.order.extend((start..self.rules.len()).map(Slot::Rule));
    }

    /// The rule storage, for compilation steps that append several rules.
    /// Callers record the appended rules with [`Self::note_rules`].
    pub(crate) fn rules_vec(&mut self) -> &mut Vec<RuleIr> {
        &mut self.rules
    }

    /// The fact arena under construction.
    pub(crate) fn arena(&mut self) -> &mut FactArena {
        &mut self.facts
    }

    /// Whether the last item is the run `block`, so a further one-row statement
    /// of its predicate continues it.
    pub(crate) fn ends_with(&self, block: usize) -> bool {
        self.order.last() == Some(&Slot::Facts(block))
    }

    /// Record a block the arena has just started as the next item.
    pub(crate) fn note_block(&mut self, block: usize) {
        self.order.push(Slot::Facts(block));
    }

    /// Keep the rules `keep` accepts and every fact block, in their order.
    pub(crate) fn retain_rules(&mut self, mut keep: impl FnMut(&RuleIr) -> bool) {
        let mut renumbered = Vec::with_capacity(self.rules.len());
        let mut next = 0;
        for rule in &self.rules {
            if keep(rule) {
                renumbered.push(Some(next));
                next += 1;
            } else {
                renumbered.push(None);
            }
        }
        let mut position = 0;
        self.rules.retain(|_| {
            let kept = renumbered[position].is_some();
            position += 1;
            kept
        });
        self.order.retain_mut(|slot| match slot {
            Slot::Rule(index) => match renumbered[*index] {
                Some(new) => {
                    *index = new;
                    true
                }
                None => false,
            },
            Slot::Facts(_) => true,
        });
    }

    /// Move the items `selected` marks (one flag per item, nonzero to move) into
    /// a new sequence, keeping both sequences' order. The moved fact blocks are
    /// copied into the new sequence's own arena, which holds no lease: its
    /// retained bytes are the caller's to charge.
    ///
    /// # Errors
    /// Refuses an allocation failure before anything moves.
    pub(crate) fn split_off(&mut self, selected: &[u8]) -> Result<Self, TryReserveError> {
        assert_eq!(selected.len(), self.order.len(), "one flag per item");
        let moving = selected.iter().filter(|flag| **flag != 0).count();
        let moving_rules = self
            .order
            .iter()
            .zip(selected)
            .filter(|(slot, flag)| **flag != 0 && matches!(slot, Slot::Rule(_)))
            .count();
        let mut moved = Self::default();
        moved.rules.try_reserve_exact(moving_rules)?;
        moved.order.try_reserve_exact(moving)?;
        for (slot, flag) in self.order.iter().zip(selected) {
            if let (Slot::Facts(block), true) = (slot, *flag != 0) {
                self.facts.reserve_copy(*block, &mut moved.facts)?;
            }
        }
        let mut kept_rules = Vec::new();
        kept_rules.try_reserve_exact(self.rules.len() - moving_rules)?;
        let mut kept_order = Vec::new();
        kept_order.try_reserve_exact(self.order.len() - moving)?;
        // Rule slots name the rules in their stored order.
        let mut rules = std::mem::take(&mut self.rules).into_iter();
        for (&slot, flag) in self.order.iter().zip(selected) {
            match (slot, *flag != 0) {
                (Slot::Rule(_), true) => {
                    moved.order.push(Slot::Rule(moved.rules.len()));
                    moved
                        .rules
                        .push(rules.next().expect("one rule per rule slot"));
                }
                (Slot::Rule(_), false) => {
                    kept_order.push(Slot::Rule(kept_rules.len()));
                    kept_rules.push(rules.next().expect("one rule per rule slot"));
                }
                (Slot::Facts(block), true) => {
                    moved.order.push(Slot::Facts(moved.facts.blocks.len()));
                    self.facts.copy_block(block, &mut moved.facts);
                }
                (Slot::Facts(block), false) => kept_order.push(Slot::Facts(block)),
            }
        }
        self.rules = kept_rules;
        self.order = kept_order;
        Ok(moved)
    }

    /// The retained capacities of the rules' and arena's vectors, and of each
    /// rule's body and origins.
    pub(crate) fn retained_bytes(&self) -> u128 {
        self.rules.capacity() as u128 * size_of::<RuleIr>() as u128
            + self
                .rules
                .iter()
                .map(|rule| {
                    rule.body.capacity() as u128 * size_of::<crate::formula_ir::LiteralIr>() as u128
                        + rule.origins.capacity() as u128 * size_of::<ProgramSite>() as u128
                })
                .sum::<u128>()
            + self.order.capacity() as u128 * size_of::<Slot>() as u128
            + self.facts.retained_bytes() as u128
    }

    /// Append a rule as the last item.
    #[cfg(test)]
    pub(crate) fn push_rule(&mut self, rule: RuleIr) {
        self.order.push(Slot::Rule(self.rules.len()));
        self.rules.push(rule);
    }

    /// Every admitted cell of the arena, for tests that edit a fact.
    #[cfg(test)]
    pub(crate) fn cells_mut(&mut self) -> &mut [Scalar] {
        &mut self.facts.cells
    }

    /// Remove the rule at `index` and its item.
    #[cfg(test)]
    pub(crate) fn remove_rule(&mut self, index: usize) -> RuleIr {
        let rule = self.rules.remove(index);
        self.order.retain_mut(|slot| match slot {
            Slot::Rule(position) if *position == index => false,
            Slot::Rule(position) => {
                if *position > index {
                    *position -= 1;
                }
                true
            }
            Slot::Facts(_) => true,
        });
        rule
    }
}

/// Facts stored by product and run, with each statement's evidence once.
#[derive(Default)]
pub(crate) struct FactArena {
    blocks: Vec<Block>,
    /// Admitted cells: each product column's values, and each run's rows
    /// row-major.
    cells: Vec<Scalar>,
    /// Product columns.
    columns: Vec<Column>,
    /// Statement evidence: once per statement of a product, once per run row.
    evidence: Vec<ProgramSite>,
    /// Each run row's evidence end, an offset into `evidence`.
    ends: Vec<usize>,
    /// The arena's retained bytes in the compiler's support workspace.
    lease: Option<StorageLease>,
    /// During compilation, the last run and its source predicate: a further
    /// one-row statement of that predicate continues it while it is the last item.
    open: Option<(zetesis_core::Predicate, usize)>,
}

struct Block {
    predicate: Predicate,
    arity: usize,
    rows: usize,
    layout: Layout,
}

enum Layout {
    /// The Cartesian product of `columns`, last column fastest; every row
    /// shares the statement's `evidence`.
    Product {
        columns: Range<usize>,
        evidence: Range<usize>,
    },
    /// Rows stored row-major from `cells`. Row `i`'s evidence ends at
    /// `ends[ends + i]` and begins where the previous row's ended, or at
    /// `evidence` for the first row.
    Run {
        cells: usize,
        ends: usize,
        evidence: usize,
    },
}

/// One product column: its admitted cells, in emission order, and the number of
/// rows that pass before it advances.
#[derive(Clone)]
struct Column {
    cells: Range<usize>,
    stride: usize,
}

/// A statement's evidence in the arena, shared by the products it expands to.
#[derive(Clone, Debug)]
pub(crate) struct Evidence(Range<usize>);

impl FactArena {
    /// Store a statement's evidence once for the products it expands to.
    pub(crate) fn store_evidence(
        &mut self,
        origins: &[ProgramSite],
    ) -> Result<Evidence, TryReserveError> {
        self.evidence.try_reserve(origins.len())?;
        let start = self.evidence.len();
        self.evidence.extend_from_slice(origins);
        Ok(Evidence(start..self.evidence.len()))
    }

    /// Reserve room for `additional` admitted cells.
    pub(crate) fn reserve_cells(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.cells.try_reserve(additional)
    }

    /// Append one admitted cell to the block under construction.
    pub(crate) fn push_cell(&mut self, cell: Scalar) {
        self.cells.push(cell);
    }

    /// The number of admitted cells so far, the start of the next column or row.
    pub(crate) fn cell_count(&self) -> usize {
        self.cells.len()
    }

    /// Start a product of `predicate` over the columns whose cells begin at
    /// each of `starts` and run to the next start, the last one to the current
    /// cell count. The product has as many rows as the columns' widths
    /// multiply to, and its rows share `evidence`.
    pub(crate) fn push_product(
        &mut self,
        predicate: Predicate,
        starts: &[usize],
        evidence: &Evidence,
    ) -> Result<usize, TryReserveError> {
        self.columns.try_reserve(starts.len())?;
        self.blocks.try_reserve(1)?;
        let first = self.columns.len();
        // Last column fastest: a column advances once every row of the
        // columns after it has passed.
        let mut stride = 1_usize;
        let mut end = self.cells.len();
        for &start in starts.iter().rev() {
            self.columns.push(Column {
                cells: start..end,
                stride,
            });
            stride = stride.saturating_mul(end - start);
            end = start;
        }
        self.columns[first..].reverse();
        let block = self.blocks.len();
        self.blocks.push(Block {
            predicate,
            arity: starts.len(),
            rows: stride,
            layout: Layout::Product {
                columns: first..self.columns.len(),
                evidence: evidence.0.clone(),
            },
        });
        Ok(block)
    }

    /// Start a run of `predicate` whose first row is the `arity` cells pushed
    /// last, with `evidence`.
    pub(crate) fn push_run(
        &mut self,
        predicate: Predicate,
        arity: usize,
        evidence: &Evidence,
    ) -> Result<usize, TryReserveError> {
        self.blocks.try_reserve(1)?;
        self.ends.try_reserve(1)?;
        let block = self.blocks.len();
        self.blocks.push(Block {
            predicate,
            arity,
            rows: 1,
            layout: Layout::Run {
                cells: self.cells.len() - arity,
                ends: self.ends.len(),
                evidence: evidence.0.start,
            },
        });
        self.ends.push(evidence.0.end);
        Ok(block)
    }

    /// Continue the run `block` with the row of cells pushed last, whose
    /// evidence was stored last.
    pub(crate) fn extend_run(
        &mut self,
        block: usize,
        evidence: &Evidence,
    ) -> Result<(), TryReserveError> {
        self.ends.try_reserve(1)?;
        debug_assert_eq!(
            self.ends.last().copied(),
            Some(evidence.0.start),
            "a run's rows store their evidence contiguously"
        );
        self.ends.push(evidence.0.end);
        self.blocks[block].rows += 1;
        Ok(())
    }

    /// The last run, if its source predicate is `predicate`: its block and its
    /// admitted predicate.
    pub(crate) fn open_run(
        &self,
        predicate: &zetesis_core::Predicate,
    ) -> Option<(usize, Predicate)> {
        let (open, block) = self.open.as_ref()?;
        (open == predicate).then(|| (*block, self.blocks[*block].predicate))
    }

    /// Record the run `block` of `predicate` as the one later statements may continue.
    pub(crate) fn open(&mut self, predicate: zetesis_core::Predicate, block: usize) {
        self.open = Some((predicate, block));
    }

    /// The retained capacities of the arena's vectors.
    pub(crate) fn retained_bytes(&self) -> usize {
        self.blocks.capacity() * size_of::<Block>()
            + self.cells.capacity() * size_of::<Scalar>()
            + self.columns.capacity() * size_of::<Column>()
            + self.evidence.capacity() * size_of::<ProgramSite>()
            + self.ends.capacity() * size_of::<usize>()
    }

    /// The lease holding the arena's retained bytes, installed by `install`
    /// on first use.
    pub(crate) fn lease(&mut self, install: impl FnOnce() -> StorageLease) -> &mut StorageLease {
        self.lease.get_or_insert_with(install)
    }

    /// Reserve, in `target`, everything [`Self::copy_block`] appends for `block`.
    fn reserve_copy(&self, block: usize, target: &mut Self) -> Result<(), TryReserveError> {
        let product = FactProduct { arena: self, block };
        target.blocks.try_reserve(1)?;
        target.cells.try_reserve(product.cells().len())?;
        match &self.blocks[block].layout {
            Layout::Product { columns, evidence } => {
                target.columns.try_reserve(columns.len())?;
                target.evidence.try_reserve(evidence.len())?;
            }
            Layout::Run { .. } => {
                target.ends.try_reserve(self.blocks[block].rows)?;
                let rows = product.rows();
                let evidence = rows.map(|row| row.evidence().len()).sum();
                target.evidence.try_reserve(evidence)?;
            }
        }
        Ok(())
    }

    /// Append a copy of `block` to `target`, whose room
    /// [`Self::reserve_copy`] reserved.
    fn copy_block(&self, block: usize, target: &mut Self) {
        let source = &self.blocks[block];
        let product = FactProduct { arena: self, block };
        let cells = target.cells.len();
        target.cells.extend_from_slice(product.cells());
        let layout = match &source.layout {
            Layout::Product { columns, evidence } => {
                let first = target.columns.len();
                for column in &self.columns[columns.clone()] {
                    let start = cells + (column.cells.start - product.cells_start());
                    target.columns.push(Column {
                        cells: start..start + column.cells.len(),
                        stride: column.stride,
                    });
                }
                let start = target.evidence.len();
                target
                    .evidence
                    .extend_from_slice(&self.evidence[evidence.clone()]);
                Layout::Product {
                    columns: first..target.columns.len(),
                    evidence: start..target.evidence.len(),
                }
            }
            Layout::Run { .. } => {
                let ends = target.ends.len();
                let evidence = target.evidence.len();
                for row in product.rows() {
                    target.evidence.extend_from_slice(row.evidence());
                    target.ends.push(target.evidence.len());
                }
                Layout::Run {
                    cells,
                    ends,
                    evidence,
                }
            }
        };
        target.blocks.push(Block {
            predicate: source.predicate,
            arity: source.arity,
            rows: source.rows,
            layout,
        });
    }
}

/// A fact product: one statement's alternative, or a run of one-row
/// statements, of one predicate.
#[derive(Clone, Copy)]
pub(crate) struct FactProduct<'a> {
    arena: &'a FactArena,
    block: usize,
}

impl<'a> FactProduct<'a> {
    fn block(self) -> &'a Block {
        &self.arena.blocks[self.block]
    }

    /// The predicate, admitted once for the product.
    pub(crate) fn predicate(self) -> Predicate {
        self.block().predicate
    }

    /// The product's arity.
    pub(crate) fn arity(self) -> usize {
        self.block().arity
    }

    /// The number of rows, at least one.
    pub(crate) fn len(self) -> usize {
        self.block().rows
    }

    /// The site of the product's first statement.
    pub(crate) fn location(self) -> ProgramSite {
        self.rows()
            .next()
            .expect("a stored product has a row")
            .location()
    }

    fn cells_start(self) -> usize {
        match &self.block().layout {
            Layout::Product { columns, .. } => self
                .arena
                .columns
                .get(columns.start)
                .map_or(self.arena.cells.len(), |column| column.cells.start),
            Layout::Run { cells, .. } => *cells,
        }
    }

    /// Every admitted cell of the product: each column's distinct values, or
    /// each run row's cells.
    pub(crate) fn cells(self) -> &'a [Scalar] {
        let start = self.cells_start();
        let end = match &self.block().layout {
            Layout::Product { columns, .. } => self.arena.columns[columns.clone()]
                .last()
                .map_or(start, |column| column.cells.end),
            Layout::Run { cells, .. } => cells + self.block().rows * self.block().arity,
        };
        &self.arena.cells[start..end]
    }

    /// The bytes the product retains in its arena: its block, cells, columns
    /// and evidence.
    pub(crate) fn retained_bytes(self) -> usize {
        let (columns, evidence, ends) = match &self.block().layout {
            Layout::Product { columns, evidence } => (columns.len(), evidence.len(), 0),
            Layout::Run { .. } => (
                0,
                self.rows().map(|row| row.evidence().len()).sum(),
                self.len(),
            ),
        };
        size_of::<Block>()
            + size_of_val(self.cells())
            + columns * size_of::<Column>()
            + evidence * size_of::<ProgramSite>()
            + ends * size_of::<usize>()
    }

    /// The rows in expansion order.
    pub(crate) fn rows(self) -> impl Iterator<Item = Row<'a>> + use<'a> {
        (0..self.len()).map(move |index| Row {
            product: self,
            index,
        })
    }
}

/// One row of a fact product: one fact.
#[derive(Clone, Copy)]
pub(crate) struct Row<'a> {
    product: FactProduct<'a>,
    index: usize,
}

impl<'a> Row<'a> {
    /// The row's admitted cells, in argument order.
    pub(crate) fn cells(self) -> impl Iterator<Item = Scalar> + use<'a> {
        let arena = self.product.arena;
        let block = self.product.block();
        let index = self.index;
        (0..block.arity).map(move |column| match &block.layout {
            Layout::Product { columns, .. } => {
                let column = &arena.columns[columns.start + column];
                let width = column.cells.len();
                arena.cells[column.cells.start + (index / column.stride) % width]
            }
            Layout::Run { cells, .. } => arena.cells[cells + index * block.arity + column],
        })
    }

    /// The evidence of the row's statement.
    pub(crate) fn evidence(self) -> &'a [ProgramSite] {
        let arena = self.product.arena;
        match &self.product.block().layout {
            Layout::Product { evidence, .. } => &arena.evidence[evidence.clone()],
            Layout::Run { ends, evidence, .. } => {
                let start = if self.index == 0 {
                    *evidence
                } else {
                    arena.ends[ends + self.index - 1]
                };
                &arena.evidence[start..arena.ends[ends + self.index]]
            }
        }
    }

    /// The site of the row's statement: its evidence's first entry, as for the
    /// fact rule it replaces.
    pub(crate) fn location(self) -> ProgramSite {
        self.evidence()[0]
    }
}

#[cfg(test)]
mod tests;
