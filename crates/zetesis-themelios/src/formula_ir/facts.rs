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
            let columns = Columns::read(&product, location)?;
            let run = single && product.len() == 1;
            let open = items
                .arena()
                .open_run(source)
                .filter(|(block, _)| run && items.ends_with(*block));
            // Admit values in the order the product's rows first reach them, so
            // the vocabulary grows as it grew one fact rule at a time: the first
            // row's values, its predicate, then each later column value, last
            // column first, as the rows advance.
            let mut admitted = Vec::new();
            admitted
                .try_reserve_exact(columns.cells.len())
                .map_err(|error| allocation(error, location))?;
            admitted.resize(columns.cells.len(), None);
            for &start in &columns.starts {
                admitted[start] = Some(self.cell(columns.cells[start])?);
            }
            let predicate = match open {
                Some((_, predicate)) => predicate,
                None => {
                    self.source
                        .predicate(source.into(), self.limits, self.counters, location)?
                }
            };
            for column in (0..columns.starts.len()).rev() {
                let later = columns.starts[column] + 1..columns.end(column);
                for (slot, value) in admitted[later.clone()]
                    .iter_mut()
                    .zip(&columns.cells[later])
                {
                    self.budget.poll(location)?;
                    *slot = Some(self.cell(*value)?);
                }
            }
            let arena = items.arena();
            arena
                .reserve_cells(admitted.len())
                .map_err(|error| allocation(error, location))?;
            let first = arena.cell_count();
            for cell in admitted {
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
                let starts: Vec<usize> = columns.starts.iter().map(|start| first + start).collect();
                items
                    .arena()
                    .push_product(predicate, &starts, &evidence)
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

/// A product's column values, in emission order, column after column.
struct Columns<'a> {
    cells: Vec<Column<'a>>,
    starts: Vec<usize>,
}

impl<'a> Columns<'a> {
    fn read(product: &ProductView<'a>, location: ProgramSite) -> Result<Self, FormulaFailure> {
        let mut columns = Self {
            cells: Vec::new(),
            starts: Vec::new(),
        };
        for column in product.columns() {
            columns
                .starts
                .try_reserve(1)
                .map_err(|error| allocation(error, location))?;
            columns.starts.push(columns.cells.len());
            for cell in column {
                match cell {
                    Cell::Scalar(value) => {
                        columns
                            .cells
                            .try_reserve(1)
                            .map_err(|error| allocation(error, location))?;
                        columns.cells.push(Column::Value(value));
                    }
                    Cell::Interval(range) => {
                        let width = crate::integer_range::width(&range);
                        columns
                            .cells
                            .try_reserve(usize::try_from(width).unwrap_or(usize::MAX))
                            .map_err(|error| allocation(error, location))?;
                        columns.cells.extend(range.map(Column::Number));
                    }
                }
            }
        }
        Ok(columns)
    }

    /// The end of `column`'s values.
    fn end(&self, column: usize) -> usize {
        self.starts
            .get(column + 1)
            .copied()
            .unwrap_or(self.cells.len())
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
