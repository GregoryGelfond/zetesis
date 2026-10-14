//! The prepared program as items: rules, and facts held in fact blocks.
//!
//! A fact statement `p(t₁, …, tₙ).` expands to one product per argument
//! alternative (`fact_expansion`). Compilation stores the facts in the fact
//! arena instead of building a rule with an empty body for each of its rows:
//! a block's predicate is admitted once, its cells hold admitted scalar keys,
//! and the statement's evidence is stored once. A fact block is one item in one
//! of two layouts: a product, the Cartesian product of one statement
//! alternative's columns, or a run, consecutive one-row statements of one
//! predicate stored row by row with one evidence range per statement.
//!
//! Readers traverse the prepared program as [`Item`]s: a rule, or a fact block
//! standing exactly where its fact rules stood, its rows in expansion order
//! (alternatives in order, last argument fastest). Every consumer that visits
//! the rules in order visits items in the same order, so wherever the rule form
//! discovered, staged, interned or emitted a fact rule's head, the block's row
//! is discovered, staged, interned or emitted at the same point. A run is one
//! item: it keeps its statements' rows in order, and a reader that counts items
//! counts it once.
//!
//! Storage is O(admitted cells + evidence + blocks). Every vector of an arena
//! grows only through a caller's [`LeasedReserve`], which admits the growth
//! before it is allocated; filling reserved room allocates nothing. Reading a
//! row allocates nothing: a product's cell is selected from its column by the
//! row's mixed-radix position.

use std::ops::Range;

use crate::formula_ir::RuleIr;
use crate::formula_support::StorageLease;
use crate::formula_support::components::{Predicate, Scalar};
use crate::{FormulaFailure, ProgramSite};

/// One position of the item sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    /// A rule, by its index among the rules.
    Rule(usize),
    /// A fact block, by its index in the arena.
    Facts(usize),
}

/// Reserves room in a vector before anything is written to it, admitting the
/// allocation under `lease` first. The lease holds every buffer its caller
/// grows through it, so its receipt less `values`'s own capacity is the bytes
/// it holds besides `values`.
pub(crate) trait LeasedReserve {
    /// Reserve room for `additional` more entries in `values`.
    ///
    /// # Errors
    /// A refused admission, or an allocation failure, before `values` changes.
    fn reserve_leased<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
        lease: &mut StorageLease,
    ) -> Result<(), FormulaFailure>;
}

/// The prepared program in item order.
///
/// Every rule has exactly one rule slot, and the `k`-th rule slot in item
/// order names rule `k`. The rules therefore keep their relative order, a
/// rule's index among the rules is stable for readers that never see facts,
/// and a reader that pairs the rule slots with the rules in their stored order
/// pairs each slot with its own rule. Each operation that adds, removes or
/// moves rules keeps this: a rule is only ever appended through
/// [`Self::push_rule`], which records its slot.
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
    /// A fact block, standing where its fact rules stood.
    Facts(FactBlock<'a>),
}

/// The room [`Items::reserve_split`] reserves before anything moves: the moved
/// sequence with its vectors reserved for exactly what moves into them, and
/// the kept rules' and items' replacement vectors.
pub(crate) struct SplitRoom {
    moved: Items,
    kept_rules: Vec<RuleIr>,
    kept_order: Vec<Slot>,
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
            Slot::Facts(block) => Item::Facts(FactBlock {
                arena: &self.facts,
                block,
            }),
        }
    }

    /// Every item, in order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = Item<'_>> + '_ {
        self.order.iter().map(|&slot| self.item(slot))
    }

    /// Every fact block, in item order. A block another sequence took by
    /// [`Self::split_off`] is no longer an item, and is not listed.
    pub(crate) fn blocks(&self) -> impl Iterator<Item = FactBlock<'_>> + '_ {
        self.order.iter().filter_map(|&slot| match slot {
            Slot::Facts(block) => Some(FactBlock {
                arena: &self.facts,
                block,
            }),
            Slot::Rule(_) => None,
        })
    }

    /// The program's producer occurrences: its rules and its fact rows.
    pub(crate) fn occurrences(&self) -> usize {
        self.rules.len() + self.blocks().map(FactBlock::len).sum::<usize>()
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

    /// Move the rules out. The fact blocks stay, and the rules' item
    /// positions no longer resolve; only a consumer done with the item
    /// sequence takes them.
    pub(crate) fn take_rules(&mut self) -> Vec<RuleIr> {
        self.order.retain(|slot| matches!(slot, Slot::Facts(_)));
        std::mem::take(&mut self.rules)
    }

    /// Append a rule as the last item, recording its slot.
    pub(crate) fn push_rule(&mut self, rule: RuleIr) {
        self.order.push(Slot::Rule(self.rules.len()));
        self.rules.push(rule);
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
    /// `keep` decides each rule once, in rule order. Allocates nothing: the
    /// kept rules are compacted in place.
    pub(crate) fn retain_rules(&mut self, mut keep: impl FnMut(&RuleIr) -> bool) {
        let rules = &mut self.rules;
        // Rule slots name the rules in their stored order (the type's
        // invariant), so the slots visit the rules in order: below `kept` lie
        // the rules kept so far, from `kept` to the rule visited the rules
        // dropped so far, and above it the rules not yet visited.
        let mut next = 0;
        let mut kept = 0;
        self.order.retain_mut(|slot| match slot {
            Slot::Rule(index) => {
                debug_assert_eq!(*index, next, "the k-th rule slot names rule k");
                next += 1;
                if keep(&rules[*index]) {
                    rules.swap(kept, *index);
                    *index = kept;
                    kept += 1;
                    true
                } else {
                    false
                }
            }
            Slot::Facts(_) => true,
        });
        rules.truncate(kept);
    }

    /// Reserve, before anything moves, the room moving the items `selected`
    /// marks (one flag per item, nonzero to move) needs:
    /// - under `moved_lease`, the moved sequence's rules and items, and its
    ///   arena's vectors, each for the summed requirement of everything that
    ///   moves into it;
    /// - under `kept_lease`, the replacement vectors of this sequence's kept
    ///   rules and items.
    ///
    /// The moved arena holds no lease of its own: its bytes are
    /// `moved_lease`'s.
    ///
    /// # Errors
    /// A refused admission or allocation, before anything moves.
    ///
    /// # Panics
    /// Panics if `selected` does not hold one flag per item.
    pub(crate) fn reserve_split(
        &self,
        selected: &[u8],
        room: &mut impl LeasedReserve,
        moved_lease: &mut StorageLease,
        kept_lease: &mut StorageLease,
    ) -> Result<SplitRoom, FormulaFailure> {
        assert_eq!(selected.len(), self.order.len(), "one flag per item");
        let mut moving = 0;
        let mut moving_rules = 0;
        let mut growth = Growth::default();
        for (slot, flag) in self.order.iter().zip(selected) {
            if *flag == 0 {
                continue;
            }
            moving += 1;
            match *slot {
                Slot::Rule(_) => moving_rules += 1,
                Slot::Facts(block) => growth.add(self.facts.footprint(block)),
            }
        }
        let mut split = SplitRoom {
            moved: Self::default(),
            kept_rules: Vec::new(),
            kept_order: Vec::new(),
        };
        room.reserve_leased(&mut split.moved.rules, moving_rules, moved_lease)?;
        room.reserve_leased(&mut split.moved.order, moving, moved_lease)?;
        split.moved.facts.reserve_in(growth, room, moved_lease)?;
        room.reserve_leased(
            &mut split.kept_rules,
            self.rules.len() - moving_rules,
            kept_lease,
        )?;
        room.reserve_leased(&mut split.kept_order, self.order.len() - moving, kept_lease)?;
        Ok(split)
    }

    /// Move the items `selected` marks into the `room` that
    /// [`Self::reserve_split`] reserved for them, keeping both sequences'
    /// order, and return the moved sequence. Allocates nothing.
    ///
    /// The moved fact blocks are copied. Their storage stays in this arena,
    /// which keeps its capacity and lease, and they are no longer its items:
    /// until this arena is dropped, a moved block's cells and evidence are
    /// held twice, here and in the moved sequence, each under its own lease.
    ///
    /// # Panics
    /// Panics if `selected` does not hold one flag per item.
    pub(crate) fn split_off(&mut self, selected: &[u8], room: SplitRoom) -> Self {
        assert_eq!(selected.len(), self.order.len(), "one flag per item");
        let SplitRoom {
            mut moved,
            mut kept_rules,
            mut kept_order,
        } = room;
        // Rule slots name the rules in their stored order (the type's
        // invariant), so the k-th rule slot takes the k-th rule.
        let mut rules = std::mem::take(&mut self.rules).into_iter();
        for (&slot, flag) in self.order.iter().zip(selected) {
            match (slot, *flag != 0) {
                (Slot::Rule(_), true) => {
                    push_reserved(&mut moved.order, Slot::Rule(moved.rules.len()));
                    let rule = rules.next().expect("one rule per rule slot");
                    push_reserved(&mut moved.rules, rule);
                }
                (Slot::Rule(_), false) => {
                    push_reserved(&mut kept_order, Slot::Rule(kept_rules.len()));
                    let rule = rules.next().expect("one rule per rule slot");
                    push_reserved(&mut kept_rules, rule);
                }
                (Slot::Facts(block), true) => {
                    push_reserved(&mut moved.order, Slot::Facts(moved.facts.blocks.len()));
                    self.facts.copy_block(block, &mut moved.facts);
                }
                (Slot::Facts(block), false) => push_reserved(&mut kept_order, Slot::Facts(block)),
            }
        }
        self.rules = kept_rules;
        self.order = kept_order;
        moved
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

    /// Each vector's unused capacity: the rules, the items, and the arena's
    /// blocks, cells, columns, evidence and ends.
    #[cfg(test)]
    pub(crate) fn spare_room(&self) -> [usize; 7] {
        let facts = &self.facts;
        [
            self.rules.capacity() - self.rules.len(),
            self.order.capacity() - self.order.len(),
            facts.blocks.capacity() - facts.blocks.len(),
            facts.cells.capacity() - facts.cells.len(),
            facts.columns.capacity() - facts.columns.len(),
            facts.evidence.capacity() - facts.evidence.len(),
            facts.ends.capacity() - facts.ends.len(),
        ]
    }

    /// The bytes the arena's lease holds, and the arena's retained bytes.
    #[cfg(test)]
    pub(crate) fn arena_receipt(&self) -> (Option<usize>, usize) {
        (self.facts.leased_bytes(), self.facts.retained_bytes())
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

/// Append `value` to room reserved for it: the reservation admitted the
/// allocation, and this push makes none.
fn push_reserved<T>(values: &mut Vec<T>, value: T) {
    debug_assert!(
        values.len() < values.capacity(),
        "pushed into reserved room"
    );
    values.push(value);
}

/// Append `slice` to room reserved for it, making no allocation.
fn extend_reserved<T: Clone>(values: &mut Vec<T>, slice: &[T]) {
    debug_assert!(
        values.capacity() - values.len() >= slice.len(),
        "extended into reserved room"
    );
    values.extend_from_slice(slice);
}

/// What a statement adds to an arena, or a split moves into one: entries per
/// vector.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Growth {
    pub(crate) blocks: usize,
    pub(crate) cells: usize,
    pub(crate) columns: usize,
    pub(crate) evidence: usize,
    pub(crate) ends: usize,
}

impl Growth {
    fn add(&mut self, other: Self) {
        self.blocks += other.blocks;
        self.cells += other.cells;
        self.columns += other.columns;
        self.evidence += other.evidence;
        self.ends += other.ends;
    }
}

/// Facts stored by block, with each statement's evidence once.
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

/// A statement's evidence in the arena, shared by the blocks it expands to.
#[derive(Clone, Debug)]
pub(crate) struct Evidence(Range<usize>);

impl FactArena {
    /// Reserve `growth` under the arena's lease, installed by `install` on
    /// first use, admitting each vector's growth before it is allocated.
    ///
    /// # Errors
    /// A refused admission or allocation, before the arena changes.
    pub(crate) fn reserve(
        &mut self,
        growth: Growth,
        install: impl FnOnce() -> StorageLease,
        room: &mut impl LeasedReserve,
    ) -> Result<(), FormulaFailure> {
        let mut lease = self.lease.take().unwrap_or_else(install);
        let reserved = self.reserve_in(growth, room, &mut lease);
        self.lease = Some(lease);
        reserved
    }

    /// Reserve `growth` in each vector under `lease`.
    fn reserve_in(
        &mut self,
        growth: Growth,
        room: &mut impl LeasedReserve,
        lease: &mut StorageLease,
    ) -> Result<(), FormulaFailure> {
        room.reserve_leased(&mut self.blocks, growth.blocks, lease)?;
        room.reserve_leased(&mut self.cells, growth.cells, lease)?;
        room.reserve_leased(&mut self.columns, growth.columns, lease)?;
        room.reserve_leased(&mut self.evidence, growth.evidence, lease)?;
        room.reserve_leased(&mut self.ends, growth.ends, lease)
    }

    /// Store a statement's evidence once for the blocks it expands to, in
    /// reserved room.
    pub(crate) fn store_evidence(&mut self, origins: &[ProgramSite]) -> Evidence {
        let start = self.evidence.len();
        extend_reserved(&mut self.evidence, origins);
        Evidence(start..self.evidence.len())
    }

    /// Append one admitted cell to the block under construction, in reserved
    /// room.
    pub(crate) fn push_cell(&mut self, cell: Scalar) {
        push_reserved(&mut self.cells, cell);
    }

    /// The number of admitted cells so far, the start of the next column or row.
    pub(crate) fn cell_count(&self) -> usize {
        self.cells.len()
    }

    /// Start a product of `predicate`, in reserved room, over the columns
    /// whose cells begin at each of `starts` and run to the next start, the
    /// last one to the current cell count. The product has as many rows as the
    /// columns' widths multiply to, and its rows share `evidence`.
    pub(crate) fn push_product(
        &mut self,
        predicate: Predicate,
        starts: &[usize],
        evidence: &Evidence,
    ) -> usize {
        let first = self.columns.len();
        // Last column fastest: a column advances once every row of the
        // columns after it has passed.
        let mut stride = 1_usize;
        let mut end = self.cells.len();
        for &start in starts.iter().rev() {
            push_reserved(
                &mut self.columns,
                Column {
                    cells: start..end,
                    stride,
                },
            );
            stride = stride.saturating_mul(end - start);
            end = start;
        }
        self.columns[first..].reverse();
        let block = self.blocks.len();
        push_reserved(
            &mut self.blocks,
            Block {
                predicate,
                arity: starts.len(),
                rows: stride,
                layout: Layout::Product {
                    columns: first..self.columns.len(),
                    evidence: evidence.0.clone(),
                },
            },
        );
        block
    }

    /// Start a run of `predicate`, in reserved room, whose first row is the
    /// `arity` cells pushed last, with `evidence`.
    pub(crate) fn push_run(
        &mut self,
        predicate: Predicate,
        arity: usize,
        evidence: &Evidence,
    ) -> usize {
        let block = self.blocks.len();
        push_reserved(
            &mut self.blocks,
            Block {
                predicate,
                arity,
                rows: 1,
                layout: Layout::Run {
                    cells: self.cells.len() - arity,
                    ends: self.ends.len(),
                    evidence: evidence.0.start,
                },
            },
        );
        push_reserved(&mut self.ends, evidence.0.end);
        block
    }

    /// Continue the run `block`, in reserved room, with the row of cells
    /// pushed last, whose evidence was stored last.
    pub(crate) fn extend_run(&mut self, block: usize, evidence: &Evidence) {
        debug_assert_eq!(
            self.ends.last().copied(),
            Some(evidence.0.start),
            "a run's rows store their evidence contiguously"
        );
        push_reserved(&mut self.ends, evidence.0.end);
        self.blocks[block].rows += 1;
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

    /// The bytes the arena's lease holds, if it has one.
    #[cfg(test)]
    pub(crate) fn leased_bytes(&self) -> Option<usize> {
        self.lease.as_ref().map(StorageLease::bytes)
    }

    /// What [`Self::copy_block`] appends for `block`, per vector.
    fn footprint(&self, block: usize) -> Growth {
        let source = FactBlock { arena: self, block };
        let mut growth = Growth {
            blocks: 1,
            cells: source.cells().len(),
            ..Growth::default()
        };
        match &self.blocks[block].layout {
            Layout::Product { columns, evidence } => {
                growth.columns = columns.len();
                growth.evidence = evidence.len();
            }
            Layout::Run { .. } => {
                growth.ends = self.blocks[block].rows;
                growth.evidence = source.rows().map(|row| row.evidence().len()).sum();
            }
        }
        growth
    }

    /// Append a copy of `block` to `target`, in the room its
    /// [`Self::footprint`] reserved there.
    fn copy_block(&self, block: usize, target: &mut Self) {
        let source = &self.blocks[block];
        let facts = FactBlock { arena: self, block };
        let cells = target.cells.len();
        extend_reserved(&mut target.cells, facts.cells());
        let layout = match &source.layout {
            Layout::Product { columns, evidence } => {
                let first = target.columns.len();
                for column in &self.columns[columns.clone()] {
                    let start = cells + (column.cells.start - facts.cells_start());
                    push_reserved(
                        &mut target.columns,
                        Column {
                            cells: start..start + column.cells.len(),
                            stride: column.stride,
                        },
                    );
                }
                let start = target.evidence.len();
                extend_reserved(&mut target.evidence, &self.evidence[evidence.clone()]);
                Layout::Product {
                    columns: first..target.columns.len(),
                    evidence: start..target.evidence.len(),
                }
            }
            Layout::Run { .. } => {
                let ends = target.ends.len();
                let evidence = target.evidence.len();
                for row in facts.rows() {
                    extend_reserved(&mut target.evidence, row.evidence());
                    push_reserved(&mut target.ends, target.evidence.len());
                }
                Layout::Run {
                    cells,
                    ends,
                    evidence,
                }
            }
        };
        push_reserved(
            &mut target.blocks,
            Block {
                predicate: source.predicate,
                arity: source.arity,
                rows: source.rows,
                layout,
            },
        );
    }
}

/// A fact block: one item holding rows of one predicate, in one of two
/// layouts. A product holds one statement alternative's rows as the Cartesian
/// product of its columns; a run holds consecutive one-row statements, one row
/// each.
#[derive(Clone, Copy)]
pub(crate) struct FactBlock<'a> {
    arena: &'a FactArena,
    block: usize,
}

impl<'a> FactBlock<'a> {
    fn block(self) -> &'a Block {
        &self.arena.blocks[self.block]
    }

    /// The predicate, admitted once for the block.
    pub(crate) fn predicate(self) -> Predicate {
        self.block().predicate
    }

    /// The block's arity.
    pub(crate) fn arity(self) -> usize {
        self.block().arity
    }

    /// The number of rows, at least one.
    pub(crate) fn len(self) -> usize {
        self.block().rows
    }

    /// The site of the block's first statement.
    pub(crate) fn location(self) -> ProgramSite {
        self.rows()
            .next()
            .expect("a stored block has a row")
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

    /// Every admitted cell of the block: each product column's distinct
    /// values, or each run row's cells.
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

    /// The rows in expansion order.
    pub(crate) fn rows(self) -> impl Iterator<Item = Row<'a>> + use<'a> {
        (0..self.len()).map(move |index| Row { facts: self, index })
    }
}

/// One row of a fact block: one fact.
#[derive(Clone, Copy)]
pub(crate) struct Row<'a> {
    facts: FactBlock<'a>,
    index: usize,
}

impl<'a> Row<'a> {
    /// The row's admitted cells, in argument order.
    pub(crate) fn cells(self) -> impl Iterator<Item = Scalar> + use<'a> {
        let arena = self.facts.arena;
        let block = self.facts.block();
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
        let arena = self.facts.arena;
        match &self.facts.block().layout {
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
