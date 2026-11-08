//! Checked integer expression indexes over one immutable relation snapshot.
//!
//! A retained index is a certificate: every source row's expression evaluated
//! to an integer with the ordinary checked scalar operations. Its entries are
//! (integer key, source row), sorted by both coordinates. Equal-key postings
//! consequently retain source order, including after a first-new interval cut.
//! A nonnumeric input or undefined operation declines the whole index. Owner,
//! resource and interruption failures propagate and publish no partial entry.
//!
//! The expression uses the existing IR operations, with variable coordinates
//! replaced by column coordinates. No source term or tuple payload is copied.
//! For n rows and m expression nodes, construction takes O(n*m + n*log(n))
//! work and O(n + m) space; the retained index lookup takes O(log(n)) work and
//! constant space. A range query selects k entries, clips their source IDs and
//! sorts its temporary union in source order: O(log(n) + k + s*log(s)) work
//! and O(s) transient space for s rows inside the source interval. When the
//! key range holds more than half as many entries as the interval has rows,
//! the query instead reads the interval's own keys in source order:
//! O(log(n) + s) work, no sort. With d retained
//! descriptors and a columns, descriptor matching costs O(d*m*a); the consumer
//! also evaluates its bound operand.
//!
//! Support completion keeps its certificates from one round to the next. Its
//! rows are only appended and keep their positions, so a certificate over a
//! relation's first rows remains a certificate over those rows in every later
//! snapshot, and a decline stays a decline. A kept certificate holds no snapshot
//! borrow: the next snapshot finds it again by its relation's signature, then
//! extends it with the rows appended since. For Δ new rows the extension
//! evaluates and sorts them, O(Δ*m + Δ*log(Δ)), and merges them from the end.
//! The merge moves only the kept entries whose key exceeds the smallest new
//! key: none when the new keys are at or above the kept maximum, as for a
//! counter, and at most all n otherwise. The kept bytes stay leased in the
//! support workspace for the whole fixpoint, under `max_support_bytes`.

use std::cell::RefCell;
use std::mem::size_of;
use std::ops::Range;

use themelios_program::program::Relation;
use zetesis_core::{PatternRef, TemplateTerm, ValueNodeRef};

use super::signature::{Signature, signature};
use super::{RelationRows, Support};
use crate::ProgramSite;
use crate::formula::ceiling;
use crate::formula_ir::{Expression, Operation};
use crate::formula_support::{
    Buffer, Computation, Context, GroundingWork, StorageLease, reserve, sort,
};
use crate::grounding_observer::Event;
use crate::{FormulaFailure, FormulaResource};

#[cfg(test)]
mod tests;

/// The support owner counts this header. Its lease counts only the entry
/// vector's capacity; each embedded immutable buffer owns its payload lease.
#[derive(Default)]
pub(in crate::formula_support) struct Workspace<'source> {
    cache: RefCell<Option<Cache<'source>>>,
}

/// Certificates kept between rounds of one support completion. They borrow no
/// snapshot; the next round's queries find each again by relation signature.
#[derive(Default)]
pub(in crate::formula_support) struct Retained(Option<Cache<'static>>);

struct Cache<'source> {
    entries: Vec<Entry<'source>>,
    lease: StorageLease,
}

struct Entry<'source> {
    /// The source in this snapshot, once a query here has named its relation.
    source: Option<&'source RelationRows<'source>>,
    relation: Signature,
    expression: Buffer<Operation>,
    /// The certificate or decline covers the source rows before this position.
    covered: usize,
    /// None is a completed decline, never an interrupted or partial build.
    integer: Option<IntegerIndex>,
}

/// Nonempty, fully checked rows. The first and last keys are the certificate's
/// closed integer bounds; only its producer below can construct this value.
struct IntegerIndex {
    entries: Buffer<(i32, usize)>,
}

/// An immutable posting lends its support owner, not a `RefCell` guard. Cache
/// growth can move descriptors but never changes a published entry or offset.
pub(in crate::formula_support) struct Rows<'read, 'source> {
    workspace: &'read Workspace<'source>,
    index: usize,
    range: Range<usize>,
}

impl Rows<'_, '_> {
    pub(in crate::formula_support) fn get(&self, position: usize) -> Option<usize> {
        let offset = self.range.start.checked_add(position)?;
        if offset >= self.range.end {
            return None;
        }
        let cache = self.workspace.cache.borrow();
        let integer = cache.as_ref().expect("a posting retains its cache").entries[self.index]
            .integer
            .as_ref()
            .expect("a posting retains its complete integer certificate");
        Some(integer.entries.slice()[offset].1)
    }
}

impl<'source> Support<'source> {
    /// Test census of the retained payload; inline headers belong to Support
    /// and entry capacity. This does not read the workspace receipt it checks.
    #[cfg(test)]
    pub(in crate::formula_support) fn numeric_index_bytes(&self) -> usize {
        self.expression_keys
            .cache
            .borrow()
            .as_ref()
            .map_or(0, |cache| {
                cache.entries.capacity() * size_of::<Entry<'_>>()
                    + cache
                        .entries
                        .iter()
                        .map(|entry| {
                            entry.relation.name_bytes()
                                + entry.expression.capacity() * size_of::<Operation>()
                                + entry.integer.as_ref().map_or(0, |integer| {
                                    integer.entries.capacity() * size_of::<(i32, usize)>()
                                })
                        })
                        .sum::<usize>()
            })
    }

    /// Look up a numeric key in one column-mapped expression. The caller has
    /// resolved source from this support and proved that pattern binds every
    /// expression input. Empty sources need no certificate and are handled by
    /// the ordinary probe. The residual comparison is never consumed here.
    pub(in crate::formula_support) fn expression_key<'read>(
        &'read self,
        source: &'source RelationRows<'source>,
        expression: &Expression,
        pattern: PatternRef<'_>,
        wanted: i32,
        rows: Range<usize>,
        mut context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Option<Rows<'read, 'source>>, FormulaFailure> {
        let Some(index) = self.numeric_index(source, expression, pattern, &mut context)? else {
            return Ok(None);
        };
        let cache = self.expression_keys.cache.borrow();
        let integer = cache.as_ref().expect("selected numeric cache").entries[index]
            .integer
            .as_ref()
            .expect("selected integer certificate");
        let range = integer.lookup(wanted, rows, &mut context)?;
        context.work.counters.record(Event::ExpressionKeyProbe);
        Ok(Some(Rows {
            workspace: &self.expression_keys,
            index,
            range,
        }))
    }

    /// An ordering query uses the same one-column expression descriptor and
    /// checked index as equality. Its transient union is sorted by source row,
    /// never by numeric key, before the join can visit it.
    pub(in crate::formula_support) fn range_key(
        &self,
        source: &'source RelationRows<'source>,
        expression: &Expression,
        pattern: PatternRef<'_>,
        comparison: (Relation, i32),
        rows: Range<usize>,
        mut context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<Option<Buffer<usize>>, FormulaFailure> {
        let Some(index) = self.numeric_index(source, expression, pattern, &mut context)? else {
            return Ok(None);
        };
        let cache = self.expression_keys.cache.borrow();
        let entry = &cache.as_ref().expect("selected numeric cache").entries[index];
        let integer = entry
            .integer
            .as_ref()
            .expect("selected integer certificate");
        let [Operation::Variable(column)] = *entry.expression.slice() else {
            unreachable!("an ordering key is one column");
        };
        let selected = integer.range(comparison, rows, (source, column), &mut context)?;
        context.work.counters.record(Event::RangeProbe);
        Ok(Some(selected.embedded(context.work.location)?))
    }

    /// Authenticate and select one complete certificate for either query.
    /// A cached decline supplies no index; every failed build stays unpublished.
    fn numeric_index(
        &self,
        source: &'source RelationRows<'source>,
        expression: &Expression,
        pattern: PatternRef<'_>,
        context: &mut Context<'_, &Computation<'_, '_>>,
    ) -> Result<Option<usize>, FormulaFailure> {
        tick(context)?;
        if !self.workspace().owns(&context.computation.lease()) {
            return Err(crate::formula_binding::assignment(
                zetesis_core::catalog::AssignmentError::Read(
                    zetesis_core::catalog::ReadError::ForeignCatalog,
                ),
                context.work.location,
            ));
        }
        let owner = self.resolve(
            pattern,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
        if !owner.is_some_and(|owner| std::ptr::eq(owner, source)) {
            return Err(FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Owner,
                location: context.work.location,
            });
        }
        if source.row_count() == 0 {
            return Ok(None);
        }
        let mut cache = self.expression_keys.cache.borrow_mut();
        let found = match cache.as_mut() {
            Some(cache) => find(&mut cache.entries, source, expression, pattern, context)?,
            None => None,
        };
        let index = if let Some(index) = found {
            let cache = cache.as_mut().expect("a found index has an owner");
            self.extend(&mut cache.entries[index], source, context)?;
            index
        } else {
            let Some(index) =
                self.prepare_expression_key(&mut cache, source, expression, pattern, context)?
            else {
                return Ok(None);
            };
            index
        };
        let cache = cache
            .as_ref()
            .expect("a found or published index has an owner");
        Ok(cache.entries[index].integer.is_some().then_some(index))
    }

    fn prepare_expression_key(
        &self,
        cache: &mut Option<Cache<'source>>,
        source: &'source RelationRows<'source>,
        expression: &Expression,
        pattern: PatternRef<'_>,
        context: &mut Context<'_, &Computation<'_, '_>>,
    ) -> Result<Option<usize>, FormulaFailure> {
        let Some(expression) = columns(expression, pattern, context)? else {
            return Ok(None);
        };
        let relation = signature(source.relation.predicate(), context)?;
        let covered = source.row_count();
        let integer = build(source, expression.slice(), self.entries.get(), context)?;
        if cache.is_none() {
            *cache = Some(Cache {
                entries: Vec::new(),
                lease: context.computation.lease(),
            });
        }
        let cache = cache.as_mut().expect("the query owns its cache");
        reserve(
            &mut cache.entries,
            1,
            &mut cache.lease,
            0,
            Context::new(
                context.computation,
                context.work.limits,
                context.work.counters,
                context.work.location,
            ),
        )?;
        tick(context)?;
        let count = integer.as_ref().map_or(0, |index| index.entries.len());
        let integer = integer
            .map(|index| {
                index
                    .entries
                    .embedded(context.work.location)
                    .map(|entries| IntegerIndex { entries })
            })
            .transpose()?;
        let entry = Entry {
            source: Some(source),
            relation,
            expression: expression.embedded(context.work.location)?,
            covered,
            integer,
        };
        let index = cache.entries.len();
        cache.entries.push(entry);
        self.entries.set(self.entries.get() + count);
        context.work.counters.record(if count == 0 {
            Event::NumericIndexDeclined
        } else {
            Event::NumericIndexBuild
        });
        Ok(Some(index))
    }

    /// Bring a kept certificate up to this snapshot's rows. The rows it covers
    /// are unchanged, so it stays valid for them; only the appended rows are
    /// evaluated. One undefined appended key declines the whole index, exactly
    /// as a build over the complete snapshot would.
    fn extend(
        &self,
        entry: &mut Entry<'source>,
        source: &RelationRows<'_>,
        context: &mut Context<'_, &Computation<'_, '_>>,
    ) -> Result<(), FormulaFailure> {
        let rows = source.row_count();
        if entry.covered == rows {
            return Ok(());
        }
        debug_assert!(entry.covered < rows, "support rows are only appended");
        if let Some(integer) = &mut entry.integer {
            let retained = integer.entries.len();
            let rows = entry.covered..rows;
            if let Some(added) = keys(
                source,
                entry.expression.slice(),
                rows,
                self.entries.get(),
                context,
            )? {
                integer.merge(&added, context)?;
                self.entries.set(self.entries.get() + added.len());
                context.work.counters.record(Event::NumericIndexExtension);
            } else {
                entry.integer = None;
                self.entries.set(self.entries.get() - retained);
                context.work.counters.record(Event::NumericIndexDeclined);
            }
        }
        entry.covered = rows;
        Ok(())
    }
}

impl Workspace<'_> {
    /// Keep this snapshot's certificates for the next round of the same support.
    /// Their snapshot borrows end here; signatures find them again.
    pub(in crate::formula_support) fn retain(
        self,
        location: ProgramSite,
    ) -> Result<Retained, FormulaFailure> {
        let Some(Cache { entries, mut lease }) = self.cache.into_inner() else {
            return Ok(Retained::default());
        };
        let entries: Vec<Entry<'static>> = entries.into_iter().map(Entry::unresolved).collect();
        lease.observe(entries.capacity() * size_of::<Entry<'_>>(), location)?;
        Ok(Retained(Some(Cache { entries, lease })))
    }
}

impl Retained {
    /// The integer entries these certificates keep, for the index ceiling.
    pub(in crate::formula_support) fn entries(&self) -> usize {
        self.0.as_ref().map_or(0, |cache| {
            cache
                .entries
                .iter()
                .filter_map(|entry| entry.integer.as_ref())
                .map(|integer| integer.entries.len())
                .sum()
        })
    }

    /// Lend the kept certificates to the next snapshot's queries.
    pub(in crate::formula_support) fn into_workspace<'source>(self) -> Workspace<'source> {
        Workspace {
            cache: RefCell::new(self.0.map(|Cache { entries, lease }| Cache {
                entries: entries.into_iter().map(Entry::unresolved).collect(),
                lease,
            })),
        }
    }
}

impl Entry<'_> {
    /// The same certificate with no snapshot borrow: a later query resolves it.
    fn unresolved<'next>(self) -> Entry<'next> {
        let Entry {
            source: _,
            relation,
            expression,
            covered,
            integer,
        } = self;
        Entry {
            source: None,
            relation,
            expression,
            covered,
            integer,
        }
    }
}

impl IntegerIndex {
    fn lookup(
        &self,
        wanted: i32,
        rows: Range<usize>,
        context: &mut Context<'_, &Computation<'_, '_>>,
    ) -> Result<Range<usize>, FormulaFailure> {
        tick(context)?;
        let entries = self.entries.slice();
        let low = entries
            .first()
            .expect("a certificate covers a nonempty source")
            .0;
        let high = entries
            .last()
            .expect("a certificate covers a nonempty source")
            .0;
        if wanted < low || wanted > high || rows.is_empty() {
            return Ok(0..0);
        }
        // Ordering by (key, row) intersects equality with the same half-open
        // source interval used by delta::Rows::within. No increment of an i32
        // endpoint is needed, including for MIN and MAX keys.
        let start = lower_bound(entries, (wanted, rows.start), context)?;
        let end = lower_bound(entries, (wanted, rows.end), context)?;
        Ok(start..end)
    }
    /// Select the interval's rows whose key satisfies the comparison, in source
    /// order. The certificate covers every row of `source`, so a column read
    /// from an interval row is the key the index holds for it.
    fn range(
        &self,
        (relation, bound): (Relation, i32),
        rows: Range<usize>,
        (source, column): (&RelationRows<'_>, usize),
        context: &mut Context<'_, &Computation<'_, '_>>,
    ) -> Result<Buffer<usize>, FormulaFailure> {
        tick(context)?;
        let entries = self.entries.slice();
        let keys = match relation {
            Relation::Lt => 0..key_bound(entries, bound, false, context)?,
            Relation::Le => 0..key_bound(entries, bound, true, context)?,
            Relation::Gt => key_bound(entries, bound, true, context)?..entries.len(),
            Relation::Ge => key_bound(entries, bound, false, context)?..entries.len(),
            _ => unreachable!("an ordering query has an ordering relation"),
        };
        let mut selected = buffer(context)?;
        if 2 * keys.len() > rows.len() {
            // The key range would not halve the interval: reading the
            // interval's own keys visits fewer entries and needs no sort. A
            // round's new rows are usually far fewer than the key range.
            for position in rows {
                tick(context)?;
                let key = source
                    .row(position)
                    .and_then(|row| row.value(column))
                    .and_then(|value| number(value.descriptor()))
                    .expect("the certificate covers every source row with an integer");
                if ordered(relation, key, bound) {
                    push(&mut selected, position, context)?;
                }
            }
            return Ok(selected);
        }
        for &(_, position) in &entries[keys] {
            tick(context)?;
            // Clip before retention and before any row is offered. The full
            // snapshot certificate is shared by every delta variant.
            if rows.contains(&position) {
                push(&mut selected, position, context)?;
            }
        }
        sort::by(
            selected.slice_mut(),
            GroundingWork::new(
                context.work.limits,
                context.work.counters,
                context.work.location,
            ),
            |left, right, _| Ok(left.cmp(right)),
        )?;
        Ok(selected)
    }

    /// Merge sorted entries for rows after every row this index holds. Equal
    /// keys keep source order, since each added position exceeds every kept
    /// one. Working from the end moves a kept entry only when an added key is
    /// below it.
    fn merge(
        &mut self,
        added: &Buffer<(i32, usize)>,
        context: &mut Context<'_, &Computation<'_, '_>>,
    ) -> Result<(), FormulaFailure> {
        let added = added.slice();
        let kept = self.entries.len();
        self.entries.resize_embedded(
            kept + added.len(),
            (0, 0),
            context.computation,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
        let entries = self.entries.slice_mut();
        let (mut old, mut new) = (kept, added.len());
        while new > 0 {
            tick(context)?;
            if old > 0 && entries[old - 1] > added[new - 1] {
                entries[old + new - 1] = entries[old - 1];
                old -= 1;
            } else {
                entries[old + new - 1] = added[new - 1];
                new -= 1;
            }
        }
        Ok(())
    }
}

fn ordered(relation: Relation, key: i32, bound: i32) -> bool {
    match relation {
        Relation::Lt => key < bound,
        Relation::Le => key <= bound,
        Relation::Gt => key > bound,
        Relation::Ge => key >= bound,
        _ => unreachable!("an ordering query has an ordering relation"),
    }
}

/// A certificate this snapshot has resolved, or one kept from an earlier round
/// whose relation signature names `source`; the latter is resolved here.
fn find<'source>(
    entries: &mut [Entry<'source>],
    source: &'source RelationRows<'source>,
    expression: &Expression,
    pattern: PatternRef<'_>,
    context: &mut Context<'_, &Computation<'_, '_>>,
) -> Result<Option<usize>, FormulaFailure> {
    for (index, entry) in entries.iter_mut().enumerate() {
        tick(context)?;
        let same_source = match entry.source {
            Some(resolved) => std::ptr::eq(resolved, source),
            None => entry.relation.names(source.relation.predicate(), context)?,
        };
        if same_source && same_expression(entry.expression.slice(), expression, pattern, context)? {
            entry.source = Some(source);
            return Ok(Some(index));
        }
    }
    Ok(None)
}

fn lower_bound(
    entries: &[(i32, usize)],
    wanted: (i32, usize),
    context: &mut Context<'_, &Computation<'_, '_>>,
) -> Result<usize, FormulaFailure> {
    let mut start = 0;
    let mut end = entries.len();
    while start < end {
        tick(context)?;
        let middle = start + (end - start) / 2;
        if entries[middle] < wanted {
            start = middle + 1;
        } else {
            end = middle;
        }
    }
    Ok(start)
}

/// First key >= bound, or > bound when inclusive. Avoid endpoint arithmetic,
/// so MIN/MAX are ordinary query values and duplicate keys stay together.
fn key_bound(
    entries: &[(i32, usize)],
    bound: i32,
    inclusive: bool,
    context: &mut Context<'_, &Computation<'_, '_>>,
) -> Result<usize, FormulaFailure> {
    let (mut start, mut end) = (0, entries.len());
    while start < end {
        tick(context)?;
        let middle = start + (end - start) / 2;
        if entries[middle].0 < bound || (inclusive && entries[middle].0 == bound) {
            start = middle + 1;
        } else {
            end = middle;
        }
    }
    Ok(start)
}

fn build(
    source: &RelationRows<'_>,
    expression: &[Operation],
    retained_entries: usize,
    context: &mut Context<'_, &Computation<'_, '_>>,
) -> Result<Option<IntegerIndex>, FormulaFailure> {
    Ok(keys(
        source,
        expression,
        0..source.row_count(),
        retained_entries,
        context,
    )?
    .map(|entries| IntegerIndex { entries }))
}

/// The sorted (key, row) entries of `rows`, or `None` when one key declines.
fn keys(
    source: &RelationRows<'_>,
    expression: &[Operation],
    rows: Range<usize>,
    retained_entries: usize,
    context: &mut Context<'_, &Computation<'_, '_>>,
) -> Result<Option<Buffer<(i32, usize)>>, FormulaFailure> {
    let mut entries = buffer(context)?;
    let mut scratch = buffer(context)?;
    for position in rows {
        tick(context)?;
        let row = source
            .row(position)
            .expect("source row count bounds the index");
        let value = evaluate(expression, &mut scratch, context, |column, context| {
            tick(context)?;
            let value = row
                .value(column)
                .ok_or_else(|| super::super::components::missing(context.work.location))?;
            Ok(number(value.descriptor()))
        })?;
        let Some(value) = value else {
            return Ok(None);
        };
        ceiling(
            FormulaResource::SupportIndexEntries,
            retained_entries as u128 + entries.len() as u128 + 1,
            context.work.limits.max_support_index_entries as u128,
            context.work.location,
        )?;
        push(&mut entries, (value, position), context)?;
    }
    sort::by(
        entries.slice_mut(),
        GroundingWork::new(
            context.work.limits,
            context.work.counters,
            context.work.location,
        ),
        |left, right, _| Ok(left.cmp(right)),
    )?;
    Ok(Some(entries))
}

/// Checked evaluation of existing IR nodes, with the caller supplying either
/// column values or bound prefix values. Only nonnumbers, unsupported shapes
/// and scalar evaluation failures decline; resource/owner errors propagate.
pub(in crate::formula_support) fn evaluate(
    expression: &[Operation],
    scratch: &mut Buffer<i32>,
    context: &mut Context<'_, &Computation<'_, '_>>,
    mut input: impl FnMut(
        usize,
        &mut Context<'_, &Computation<'_, '_>>,
    ) -> Result<Option<i32>, FormulaFailure>,
) -> Result<Option<i32>, FormulaFailure> {
    scratch.clear();
    context.work.counters.record(Event::ExpressionEvaluation);
    for operation in expression {
        tick(context)?;
        context.work.counters.record(Event::ExpressionNode);
        let value = match *operation {
            Operation::Constant(value) => {
                let value = context.computation.static_scalar(
                    value,
                    context.work.limits,
                    context.work.counters,
                    context.work.location,
                )?;
                number(value.descriptor())
            }
            Operation::Variable(slot) => input(slot, context)?,
            Operation::Unary(operator, argument) => scratch
                .slice()
                .get(argument)
                .and_then(|&value| crate::scalar_arithmetic::unary(operator, value).ok()),
            Operation::Binary(operator, left, right) => scratch
                .slice()
                .get(left)
                .zip(scratch.slice().get(right))
                .and_then(|(&left, &right)| {
                    crate::scalar_arithmetic::binary(operator, left, right).ok()
                }),
            Operation::Absolute(argument) => scratch
                .slice()
                .get(argument)
                .and_then(|&value| crate::scalar_arithmetic::absolute(value).ok()),
            Operation::Constructor(_) => None,
        };
        let Some(value) = value else {
            return Ok(None);
        };
        push(scratch, value, context)?;
    }
    Ok(scratch.slice().last().copied())
}

pub(in crate::formula_support) fn number(value: ValueNodeRef<'_>) -> Option<i32> {
    match value {
        ValueNodeRef::Number(value) => Some(value),
        _ => None,
    }
}

/// One occurrence may bind a slot in several columns. The matcher still
/// checks all aliases, so its first column is a covering key input.
pub(in crate::formula_support) fn column(
    pattern: PatternRef<'_>,
    slot: usize,
    context: &mut Context<'_, &Computation<'_, '_>>,
) -> Result<Option<usize>, FormulaFailure> {
    for column in 0..pattern.terms().len() {
        tick(context)?;
        if matches!(pattern.terms().at(column), Some(TemplateTerm::Variable(found)) if found == slot)
        {
            return Ok(Some(column));
        }
    }
    Ok(None)
}

fn columns(
    expression: &Expression,
    pattern: PatternRef<'_>,
    context: &mut Context<'_, &Computation<'_, '_>>,
) -> Result<Option<Buffer<Operation>>, FormulaFailure> {
    let mut result = buffer(context)?;
    for (position, operation) in expression.nodes.iter().enumerate() {
        tick(context)?;
        let operation = match *operation {
            Operation::Constant(value) => Operation::Constant(value),
            Operation::Variable(slot) => {
                let Some(column) = column(pattern, slot, context)? else {
                    return Ok(None);
                };
                Operation::Variable(column)
            }
            Operation::Unary(op, argument) if argument < position => Operation::Unary(op, argument),
            Operation::Binary(op, left, right) if left < position && right < position => {
                Operation::Binary(op, left, right)
            }
            Operation::Absolute(argument) if argument < position => Operation::Absolute(argument),
            _ => return Ok(None),
        };
        push(&mut result, operation, context)?;
    }
    Ok((!result.slice().is_empty()).then_some(result))
}

fn same_expression(
    stored: &[Operation],
    expression: &Expression,
    pattern: PatternRef<'_>,
    context: &mut Context<'_, &Computation<'_, '_>>,
) -> Result<bool, FormulaFailure> {
    if stored.len() != expression.nodes.len() {
        return Ok(false);
    }
    for (left, right) in stored.iter().zip(&expression.nodes) {
        tick(context)?;
        let same = match (left, right) {
            (Operation::Constant(left), Operation::Constant(right)) => left == right,
            (Operation::Variable(left), Operation::Variable(slot)) => {
                Some(*left) == column(pattern, *slot, context)?
            }
            (Operation::Unary(left, a), Operation::Unary(right, b)) => left == right && a == b,
            (Operation::Binary(left, a, b), Operation::Binary(right, c, d)) => {
                left == right && a == c && b == d
            }
            (Operation::Absolute(left), Operation::Absolute(right)) => left == right,
            _ => false,
        };
        if !same {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(in crate::formula_support) fn tick(
    context: &mut Context<'_, &Computation<'_, '_>>,
) -> Result<(), FormulaFailure> {
    context
        .work
        .counters
        .work(context.work.limits, context.work.location)
}

pub(in crate::formula_support) fn buffer<T>(
    context: &mut Context<'_, &Computation<'_, '_>>,
) -> Result<Buffer<T>, FormulaFailure> {
    Buffer::new(
        context.computation,
        context.work.limits,
        context.work.counters,
        context.work.location,
    )
}

fn push<T>(
    buffer: &mut Buffer<T>,
    value: T,
    context: &mut Context<'_, &Computation<'_, '_>>,
) -> Result<(), FormulaFailure> {
    buffer.push(
        value,
        context.computation,
        context.work.limits,
        context.work.counters,
        context.work.location,
    )
}
