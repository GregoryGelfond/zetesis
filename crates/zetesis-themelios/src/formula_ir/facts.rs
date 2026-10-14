//! Fact statements compiled into the fact arena, without a rule per fact.

use zetesis_core::Value;

use super::{Compiler, Counters, components};
use crate::expansion::Budget;
use crate::fact_expansion::{Cell, Facts, ProductView};
use crate::formula::ceiling;
use crate::formula_facts::{Growth, Items, LeasedReserve};
use crate::formula_support::components::Scalar;
use crate::formula_support::{StorageLease, growth_target};
use crate::{FormulaFailure, FormulaLimits, FormulaResource, ProgramSite};
use zetesis_core::catalog::{AssignmentError, AssignmentFailure, Error};

impl Compiler<'_> {
    /// Store a fact statement's products in the arena as fact blocks, where
    /// its fact rules would have stood.
    ///
    /// Each product's arity ceiling and predicate admission run once: every row
    /// of a product shares its predicate, so the ceiling's verdict is its first
    /// row's, at the same statement site. Each column value's scalar admission
    /// runs once, however many rows select it. The statement's evidence is
    /// stored once for all its blocks. A one-row statement of the predicate of
    /// the run that is the last item continues that run, reusing its admitted
    /// predicate.
    ///
    /// After a product's admissions and before the arena grows, the growth it
    /// needs (its cells, its block entry and columns or its run row's end, and
    /// with the first product the statement's evidence) is admitted against
    /// the support workspace through the arena's lease. Each vector grows
    /// geometrically when its whole replacement fits the allowance, and
    /// exactly otherwise, as the support's other leased storage grows.
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
        let single = facts.products().nth(1).is_none();
        let mut evidence = None;
        for product in facts.products() {
            let source = product.predicate();
            ceiling(
                FormulaResource::Arity,
                source.arity() as u128,
                self.options.core_limits.max_predicate_arity as u128,
                location,
            )?;
            scratch.layout(product, self.budget, location)?;
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
            let mut growth = Growth {
                cells: scratch.admitted.len(),
                ..Growth::default()
            };
            if evidence.is_none() {
                growth.evidence = origins.len();
            }
            if open.is_some() {
                growth.ends = 1;
            } else if run {
                growth.blocks = 1;
                growth.ends = 1;
            } else {
                growth.blocks = 1;
                growth.columns = scratch.starts.len();
            }
            let mut room = ArenaGrowth {
                source: &self.source,
                limits: self.limits,
                counters: self.counters,
                location,
            };
            items
                .arena()
                .reserve(growth, || self.source.lease(), &mut room)?;
            let arena = items.arena();
            let evidence = evidence.get_or_insert_with(|| arena.store_evidence(origins));
            let first = arena.cell_count();
            for cell in scratch.admitted.drain(..) {
                arena.push_cell(cell.expect("every column value is admitted"));
            }
            if let Some((block, _)) = open {
                arena.extend_run(block, evidence);
                continue;
            }
            let block = if run {
                let block = arena.push_run(predicate, source.arity(), evidence);
                arena.open(source.clone(), block);
                block
            } else {
                for start in &mut scratch.starts {
                    *start += first;
                }
                arena.push_product(predicate, &scratch.starts, evidence)
            };
            items.note_block(block);
        }
        Ok(())
    }

    /// Admit one column value, as a fact rule's head admitted each of its terms.
    fn cell(&mut self, value: Column<'_>) -> Result<Scalar, FormulaFailure> {
        match value {
            Column::Value(value) => self.root_scalar(value),
            Column::Number(number) => self.root_scalar(&Value::Number(number)),
        }
    }
}

/// Grows the fact arena under its lease in the compiler's support workspace,
/// admitting each vector's whole replacement (the vector's old and new
/// capacity, beside every other buffer the lease holds) before allocating it:
/// geometrically when that replacement fits the allowance, exactly when only
/// the requirement does. The arena's growth charges storage, not work, as the
/// compiler's other storage does.
struct ArenaGrowth<'c, 'a> {
    source: &'c components::Admission<'a>,
    limits: &'c FormulaLimits,
    counters: &'c Counters,
    location: ProgramSite,
}

impl LeasedReserve for ArenaGrowth<'_, '_> {
    fn reserve_leased<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
        lease: &mut StorageLease,
    ) -> Result<(), FormulaFailure> {
        let previous = lease.bytes();
        let others = previous
            .checked_sub(values.capacity() * size_of::<T>())
            .expect("the arena's lease holds each vector it grows");
        let maximum = self.source.allowance(lease, self.limits, self.location)?;
        let result = growth_target::<T>(
            values.len(),
            values.capacity(),
            additional,
            others,
            maximum,
            true,
        )
        .and_then(|target| match target {
            None => Ok(()),
            Some(target) => values
                .try_reserve_exact(target - values.len())
                .map_err(|_| {
                    AssignmentFailure::Assignment(AssignmentError::Storage(Error::Allocation))
                }),
        });
        let actual = values
            .capacity()
            .checked_mul(size_of::<T>())
            .and_then(|bytes| bytes.checked_add(others))
            .ok_or_else(|| {
                crate::formula_binding::assignment(
                    AssignmentError::Storage(Error::Overflow),
                    self.location,
                )
            })?;
        lease.observe(actual, self.location)?;
        let observed = self.source.storage_observed(
            lease,
            previous,
            others,
            self.limits,
            self.counters,
            self.location,
        );
        self.source
            .storage_result(result, lease, self.limits, self.location)?;
        observed
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
    /// empty slot per cell. The scratch is charged to the expansion's scalar
    /// bytes as it is reserved, as the compiler's other buffers are; it is
    /// reused across statements, so only its growth is charged.
    fn layout(
        &mut self,
        product: ProductView<'_>,
        budget: &mut Budget,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        self.starts.clear();
        self.admitted.clear();
        crate::formula_pattern::reserve_amortized(
            &mut self.starts,
            product.columns().count(),
            budget,
            location,
        )?;
        let mut total = 0_usize;
        for column in product.columns() {
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
        crate::formula_pattern::reserve_amortized(&mut self.admitted, total, budget, location)?;
        self.admitted.resize(total, None);
        Ok(())
    }
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

#[cfg(test)]
mod tests {
    use themelios_program::program::{Atom, Rule, Statement};
    use themelios_program::provenance::WithProvenance;
    use themelios_program::symbol::Name;
    use themelios_program::term::Term;

    use super::{ArenaGrowth, Scratch};
    use crate::expansion::Budget;
    use crate::formula_facts::{FactArena, Growth};
    use crate::formula_ir::Compiler;
    use crate::formula_ir::testing::with_compiler;
    use crate::formula_support::components::Scalar;
    use crate::{
        ExpansionLimits, FormulaFailure, FormulaLimits, FormulaResource, ProgramSite, StatementId,
    };

    /// `p(1..upper).`
    fn interval(upper: i32) -> WithProvenance<Statement> {
        WithProvenance::constructed(
            Rule::fact(Atom::new(
                Name::new("p").unwrap(),
                [Term::Interval {
                    lower: Box::new(1.into()),
                    upper: Box::new(upper.into()),
                }],
            ))
            .into(),
        )
    }

    #[test]
    fn the_fact_scratch_is_charged_as_it_grows() {
        let location = ProgramSite::statement(StatementId::new(0), None);
        let mut expansion = Budget::new(ExpansionLimits::default(), 100);
        let wide = crate::fact_expansion::facts(&interval(10), &mut expansion, location)
            .unwrap()
            .unwrap();
        let narrow = crate::fact_expansion::facts(&interval(4), &mut expansion, location)
            .unwrap()
            .unwrap();
        let mut budget = Budget::new(ExpansionLimits::default(), 100);
        let mut scratch = Scratch::default();
        let charged = size_of::<usize>() + 10 * size_of::<Option<Scalar>>();
        scratch
            .layout(wide.products().next().unwrap(), &mut budget, location)
            .unwrap();
        assert_eq!(budget.usage().scalar_bytes, charged);
        // A narrower product reuses the room, and nothing more is charged.
        scratch
            .layout(narrow.products().next().unwrap(), &mut budget, location)
            .unwrap();
        assert_eq!(budget.usage().scalar_bytes, charged);
    }

    /// Limits that leave the fact arena `bytes` of support storage beside
    /// what the compiler's workspace already holds.
    fn allowing(compiler: &Compiler<'_>, bytes: usize) -> FormulaLimits {
        let defaults = FormulaLimits::default();
        let probe = compiler.source.lease();
        let outside = defaults.max_support_bytes
            - compiler
                .source
                .allowance(&probe, &defaults, compiler.location)
                .unwrap();
        FormulaLimits {
            max_support_bytes: outside + bytes,
            ..defaults
        }
    }

    #[test]
    fn arena_growth_beyond_the_allowance_is_refused_before_allocating() {
        with_compiler(ExpansionLimits::default(), |compiler| {
            let limits = allowing(compiler, 1024);
            let mut arena = FactArena::default();
            let mut room = ArenaGrowth {
                source: &compiler.source,
                limits: &limits,
                counters: compiler.counters,
                location: compiler.location,
            };
            let growth = Growth {
                cells: 1024,
                ..Growth::default()
            };
            let refused = arena.reserve(growth, || compiler.source.lease(), &mut room);
            assert!(matches!(
                refused,
                Err(FormulaFailure::Limit {
                    resource: FormulaResource::SupportBytes,
                    ..
                })
            ));
            assert_eq!(arena.retained_bytes(), 0);
        });
    }

    #[test]
    fn arena_growth_takes_exact_room_when_only_that_fits() {
        // Four cells grow to five: doubling to eight would hold the old four
        // and the new eight at once, twelve cells, past an allowance of ten,
        // where the exact five with the old four, nine, fit.
        let cell = size_of::<Scalar>();
        with_compiler(ExpansionLimits::default(), |compiler| {
            let one = compiler
                .root_scalar(&zetesis_core::Value::Number(1))
                .unwrap();
            let limits = allowing(compiler, 10 * cell);
            let mut arena = FactArena::default();
            let mut room = ArenaGrowth {
                source: &compiler.source,
                limits: &limits,
                counters: compiler.counters,
                location: compiler.location,
            };
            let cells = |cells| Growth {
                cells,
                ..Growth::default()
            };
            arena
                .reserve(cells(4), || compiler.source.lease(), &mut room)
                .unwrap();
            for _ in 0..4 {
                arena.push_cell(one);
            }
            arena
                .reserve(cells(1), || compiler.source.lease(), &mut room)
                .unwrap();
            assert_eq!(arena.retained_bytes(), 5 * cell);
        });
    }
}
