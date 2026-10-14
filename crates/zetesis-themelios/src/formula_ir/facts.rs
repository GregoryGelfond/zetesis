//! Fact statements compiled into the fact arena, without a rule per fact.

use zetesis_core::Value;

use super::{Compiler, Counters, components};
use crate::expansion::Budget;
use crate::fact_expansion::{Cell, Facts, ProductView};
use crate::formula::ceiling;
use crate::formula_facts::{FactArena, Items, LeasedReserve, ProductColumn, Segment};
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
            for (column, first_cell) in product.columns().zip(&mut scratch.first) {
                let first = Values::new(column)
                    .next()
                    .expect("a product column has a value");
                *first_cell = Some(self.cell(first)?);
            }
            let predicate = match open {
                Some((_, predicate)) => predicate,
                None => {
                    self.source
                        .predicate(source.into(), self.limits, self.counters, location)?
                }
            };
            // The first value of each column is already admitted. Record it
            // before that column's tail, without admitting it a second time.
            // A component gap caused by another column's first value is kept
            // as a segment boundary; numeric adjacency alone proves nothing
            // about occurrence coordinates.
            let mut stride = 1_usize;
            for (index, column) in (0..scratch.columns.len())
                .rev()
                .zip(product.columns().rev())
            {
                let start = scratch.segments.len();
                let mut values = Values::new(column);
                let first = values.next().expect("a product column has a value");
                let scalar = scratch.first[index].expect("the first value is admitted");
                scratch.push(start, 0, first, scalar, self.budget, location)?;
                let mut width = 1;
                for value in values {
                    self.budget.poll(location)?;
                    let scalar = self.cell(value)?;
                    scratch.push(start, width, value, scalar, self.budget, location)?;
                    width += 1;
                }
                scratch.columns[index] =
                    ProductColumn::new(start..scratch.segments.len(), width, stride);
                stride = stride.saturating_mul(width);
            }
            let growth = FactArena::growth(
                &scratch.columns,
                &scratch.segments,
                evidence.is_none().then_some(origins),
                run,
                open.is_some(),
            );
            let mut room = ArenaGrowth {
                source: &self.source,
                limits: self.limits,
                counters: self.counters,
                location,
            };
            items
                .arena()
                .reserve(growth, || self.source.lease(), &mut room)?;
            let columns = if run {
                Some(items.arena().reserve_row(
                    open.map(|(block, _)| block),
                    source.arity(),
                    &mut room,
                )?)
            } else {
                None
            };
            let arena = items.arena();
            let evidence = evidence.get_or_insert_with(|| arena.store_evidence(origins));
            let block = if let Some(columns) = columns {
                arena.push_row(columns.clone(), &scratch.first);
                if let Some((block, _)) = open {
                    arena.extend_run(block, evidence);
                    continue;
                }
                let block = arena.push_run(predicate, columns, evidence);
                arena.open(source.clone(), block);
                block
            } else {
                arena.push_product(predicate, &scratch.columns, &scratch.segments, evidence)
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

/// One product's column descriptors, first admitted coordinates, and compact
/// segments. The first-row coordinates must survive until the predicate is
/// admitted; the remaining values then arrive in reverse column order.
#[derive(Default)]
pub(super) struct Scratch {
    columns: Vec<ProductColumn>,
    first: Vec<Option<Scalar>>,
    segments: Vec<Segment>,
}

impl Scratch {
    /// Reserve scratch by source segments and arity, not interval width. An
    /// interval's first value may be separated from its tail by other columns'
    /// first occurrences, so allow one extra segment per column. Unexpected
    /// coordinate gaps grow this buffer through the same charged allocator.
    /// Buffers are reused across statements and only growth is charged.
    fn layout(
        &mut self,
        product: ProductView<'_>,
        budget: &mut Budget,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        self.columns.clear();
        self.first.clear();
        self.segments.clear();
        let arity = product.columns().count();
        crate::formula_pattern::reserve_amortized(&mut self.columns, arity, budget, location)?;
        crate::formula_pattern::reserve_amortized(&mut self.first, arity, budget, location)?;
        let segments = product
            .columns()
            .fold(arity, |total, column| total.saturating_add(column.count()));
        crate::formula_pattern::reserve_amortized(&mut self.segments, segments, budget, location)?;
        self.columns.resize(arity, ProductColumn::new(0..0, 0, 0));
        self.first.resize(arity, None);
        Ok(())
    }

    /// Record one already admitted value. Only an integer following another
    /// integer in this column can extend a segment, and only after both its
    /// value and actual occurrence coordinate have passed the adjacency check.
    fn push(
        &mut self,
        start: usize,
        position: usize,
        value: Column<'_>,
        scalar: Scalar,
        budget: &mut Budget,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        if let Column::Number(number) = value
            && self.segments.len() > start
            && self
                .segments
                .last_mut()
                .expect("this column has a segment")
                .append(number, scalar)
        {
            return Ok(());
        }
        crate::formula_pattern::reserve_amortized(&mut self.segments, 1, budget, location)?;
        self.segments.push(match value {
            Column::Value(_) => Segment::scalar(position, scalar),
            Column::Number(number) => Segment::integer(position, number, scalar),
        });
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
    use crate::formula_facts::{FactArena, Growth, Items, ProductColumn, Segment};
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
        let charged =
            size_of::<ProductColumn>() + size_of::<Option<Scalar>>() + 2 * size_of::<Segment>();
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

    #[test]
    fn compact_products_keep_actual_scalar_admission_order_and_work() {
        let location = ProgramSite::statement(StatementId::new(0), None);
        let statement = WithProvenance::constructed(
            Rule::fact(Atom::new(
                Name::new("p").unwrap(),
                [4, 3].map(|upper| Term::Interval {
                    lower: Box::new(1.into()),
                    upper: Box::new(upper.into()),
                }),
            ))
            .into(),
        );
        let mut expansion = Budget::new(ExpansionLimits::default(), 100);
        let facts = crate::fact_expansion::facts(&statement, &mut expansion, location)
            .unwrap()
            .unwrap();
        let reference = with_compiler(ExpansionLimits::default(), |compiler| {
            let number = zetesis_core::Value::Number;
            let mut left = vec![compiler.root_scalar(&number(1)).unwrap()];
            let mut right = vec![compiler.root_scalar(&number(1)).unwrap()];
            let source = facts.products().next().unwrap().predicate();
            let predicate = compiler
                .source
                .predicate(
                    source.into(),
                    compiler.limits,
                    compiler.counters,
                    compiler.location,
                )
                .unwrap();
            for value in 2..=3 {
                right.push(compiler.root_scalar(&number(value)).unwrap());
            }
            for value in 2..=4 {
                left.push(compiler.root_scalar(&number(value)).unwrap());
            }
            let rows: Vec<_> = left
                .into_iter()
                .flat_map(|left| right.iter().map(move |&right| vec![left, right]))
                .collect();
            (predicate, rows, compiler.counters.accounting.work)
        });
        let actual = with_compiler(ExpansionLimits::default(), |compiler| {
            let mut items = Items::default();
            compiler
                .facts(&facts, &[location], &mut items, &mut Scratch::default())
                .unwrap();
            let block = items.blocks().next().unwrap();
            let rows: Vec<_> = block
                .rows()
                .map(|row| row.cells().collect::<Vec<_>>())
                .collect();
            (block.predicate(), rows, compiler.counters.accounting.work)
        });
        assert_eq!(actual, reference);
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
            let growth = Growth::segments(1024);
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
        // Four segments grow to five: doubling holds twelve at replacement,
        // past the allowance of ten; exact growth holds nine and fits.
        let cell = size_of::<Segment>();
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
            arena
                .reserve(Growth::segments(4), || compiler.source.lease(), &mut room)
                .unwrap();
            arena.fill_segments_for_test(Segment::scalar(0, one), 4);
            arena
                .reserve(Growth::segments(1), || compiler.source.lease(), &mut room)
                .unwrap();
            assert_eq!(arena.retained_bytes(), 5 * cell);
        });
    }
}
