//! Fact statements compiled into the fact arena, without a rule per fact.

use zetesis_core::Value;

use super::Compiler;
use crate::fact_expansion::{Cell, Facts, ProductView};
use crate::formula::ceiling;
use crate::formula_facts::Items;
use crate::formula_support::components::Scalar;
use crate::{FormulaFailure, FormulaResource, ProgramSite};

impl Compiler<'_> {
    /// Store a fact statement's products in the arena as items, where its fact
    /// rules would have stood.
    ///
    /// Each product's arity ceiling and predicate admission run once: every row
    /// of a product shares its predicate, so the ceiling's verdict is its first
    /// row's, at the same statement site. Each column value's scalar admission
    /// runs once, however many rows select it. The statement's evidence is
    /// stored once for all its products. A one-row statement of the predicate of
    /// the run that is the last item continues that run, reusing its admitted
    /// predicate.
    pub(super) fn facts(
        &mut self,
        facts: &Facts,
        origins: &[ProgramSite],
        items: &mut Items,
        scratch: &mut Scratch,
    ) -> Result<(), FormulaFailure> {
        let location = self.location;
        if facts.products().next().is_none() {
            // An empty family has no row and so nothing to store.
            return Ok(());
        }
        let previous = items.arena().retained_bytes();
        let evidence = items
            .arena()
            .store_evidence(origins)
            .map_err(|error| allocation(error, location))?;
        let single = facts.products().nth(1).is_none();
        for product in facts.products() {
            let source = product.predicate();
            ceiling(
                FormulaResource::Arity,
                source.arity() as u128,
                self.options.core_limits.max_predicate_arity as u128,
                location,
            )?;
            scratch.layout(product, location)?;
            let run = single && product.len() == 1;
            let open = items
                .arena()
                .open_run(source)
                .filter(|(block, _)| run && items.ends_with(*block));
            // Admit values in the order the product's rows first reach them, so
            // the vocabulary grows as it grew one fact rule at a time: the first
            // row's values, its predicate, then each later column value, last
            // column first, as the rows advance.
            for (column, &start) in product.columns().zip(&scratch.starts) {
                let first = Values::new(column)
                    .next()
                    .expect("a product column has a value");
                scratch.admitted[start] = Some(self.cell(first)?);
            }
            let predicate = match open {
                Some((_, predicate)) => predicate,
                None => {
                    self.source
                        .predicate(source.into(), self.limits, self.counters, location)?
                }
            };
            for (column, &start) in product.columns().rev().zip(scratch.starts.iter().rev()) {
                for (offset, value) in Values::new(column).enumerate().skip(1) {
                    self.budget.poll(location)?;
                    scratch.admitted[start + offset] = Some(self.cell(value)?);
                }
            }
            let arena = items.arena();
            arena
                .reserve_cells(scratch.admitted.len())
                .map_err(|error| allocation(error, location))?;
            let first = arena.cell_count();
            for cell in scratch.admitted.drain(..) {
                arena.push_cell(cell.expect("every column value is admitted"));
            }
            if let Some((block, _)) = open {
                items
                    .arena()
                    .extend_run(block, &evidence)
                    .map_err(|error| allocation(error, location))?;
                continue;
            }
            let block = if run {
                let block = items
                    .arena()
                    .push_run(predicate, source.arity(), &evidence)
                    .map_err(|error| allocation(error, location))?;
                items.arena().open(source.clone(), block);
                block
            } else {
                for start in &mut scratch.starts {
                    *start += first;
                }
                items
                    .arena()
                    .push_product(predicate, &scratch.starts, &evidence)
                    .map_err(|error| allocation(error, location))?
            };
            items.note_block(block);
        }
        self.observe(items, previous)
    }

    /// Admit one column value, as a fact rule's head admitted each of its terms.
    fn cell(&mut self, value: Column<'_>) -> Result<Scalar, FormulaFailure> {
        match value {
            Column::Value(value) => self.root_scalar(value),
            Column::Number(number) => self.root_scalar(&Value::Number(number)),
        }
    }

    /// Charge the arena's retained capacities to the support workspace, as its
    /// neighbouring compiler storage is charged.
    fn observe(&mut self, items: &mut Items, previous: usize) -> Result<(), FormulaFailure> {
        let bytes = items.arena().retained_bytes();
        let lease = items.arena().lease(|| self.source.lease());
        lease.observe(bytes, self.location)?;
        self.source.storage_observed(
            lease,
            previous,
            0,
            self.limits,
            self.counters,
            self.location,
        )
    }
}

/// One column value of a product: a scalar segment's value, or one number of
/// an interval segment.
#[derive(Clone, Copy)]
enum Column<'a> {
    Value(&'a Value),
    Number(i32),
}

/// A column's values in emission order, read from its segments in place.
struct Values<'a, I> {
    cells: I,
    interval: Option<std::ops::RangeInclusive<i32>>,
    borrowed: std::marker::PhantomData<&'a Value>,
}

impl<'a, I: Iterator<Item = Cell<'a>>> Values<'a, I> {
    fn new(cells: I) -> Self {
        Self {
            cells,
            interval: None,
            borrowed: std::marker::PhantomData,
        }
    }
}

impl<'a, I: Iterator<Item = Cell<'a>>> Iterator for Values<'a, I> {
    type Item = Column<'a>;

    fn next(&mut self) -> Option<Column<'a>> {
        loop {
            if let Some(number) = self.interval.as_mut().and_then(Iterator::next) {
                return Some(Column::Number(number));
            }
            match self.cells.next()? {
                Cell::Scalar(value) => return Some(Column::Value(value)),
                Cell::Interval(range) => self.interval = Some(range),
            }
        }
    }
}

/// Compilation scratch reused across fact statements: one product's column
/// starts and its admitted cells, in column order.
#[derive(Default)]
pub(super) struct Scratch {
    starts: Vec<usize>,
    admitted: Vec<Option<Scalar>>,
}

impl Scratch {
    /// Lay out `product`'s columns: each column's start among its cells, and an
    /// empty slot per cell.
    fn layout(
        &mut self,
        product: ProductView<'_>,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        self.starts.clear();
        self.admitted.clear();
        let mut total = 0_usize;
        for column in product.columns() {
            self.starts
                .try_reserve(1)
                .map_err(|error| allocation(error, location))?;
            self.starts.push(total);
            for cell in column {
                total = total.saturating_add(match cell {
                    Cell::Scalar(_) => 1,
                    Cell::Interval(range) => {
                        usize::try_from(crate::integer_range::width(&range)).unwrap_or(usize::MAX)
                    }
                });
            }
        }
        self.admitted
            .try_reserve(total)
            .map_err(|error| allocation(error, location))?;
        self.admitted.resize(total, None);
        Ok(())
    }
}

fn allocation(error: std::collections::TryReserveError, location: ProgramSite) -> FormulaFailure {
    FormulaFailure::AtomAllocation { error, location }
}

/// The reference path the differential tests compare the arena against.
#[cfg(test)]
pub(crate) mod testing {
    use std::cell::Cell;

    thread_local! {
        static FACTS_AS_RULES: Cell<bool> = const { Cell::new(false) };
    }

    /// Whether compilation on this thread builds a rule with an empty body for
    /// each fact, as before the fact arena.
    pub(crate) fn facts_as_rules() -> bool {
        FACTS_AS_RULES.with(Cell::get)
    }

    /// Run `body` with facts compiled as rules on this thread, restoring the
    /// previous setting on return or unwind.
    pub(crate) fn with_facts_as_rules<T>(body: impl FnOnce() -> T) -> T {
        struct Restore(bool);
        impl Drop for Restore {
            fn drop(&mut self) {
                FACTS_AS_RULES.with(|cell| cell.set(self.0));
            }
        }
        let _restore = Restore(FACTS_AS_RULES.with(|cell| cell.replace(true)));
        body()
    }
}
