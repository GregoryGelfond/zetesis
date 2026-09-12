//! Reusable queries borrow one immutable relation snapshot.
//!
//! Growing snapshots admit indexed queries only. A completed snapshot can own
//! shared table indices; live selections borrow its relations, so inserting a
//! later index never invalidates an earlier join depth. No tuple payload moves.

#[cfg(test)]
mod tests;

use std::cell::{Cell, RefCell};
use std::mem::size_of;
use std::ops::Deref;

use themelios_base::span::Location;
use zetesis_core::{AtomKey, AtomPattern, Term, Value};
use zetesis_cpu::table::{self, Cause, Domain, Resource, Selection, Table};

use super::{Counters, PositivePattern, Relations};
use crate::formula::ceiling;
use crate::grounding_observer::Event;
use crate::{FormulaFailure, FormulaLimits, FormulaResource, JoinStrategy};

/// Query state is separate from both the catalog and its immutable row views.
/// The single-threaded source builder owns this workspace; table indices are
/// immutable after publication, while every query owns its independent mask.
pub(crate) struct Support<'source> {
    relations: &'source Relations<'source>,
    tables: Option<TableWorkspace<'source>>,
    live: Cell<usize>,
    entries: Cell<usize>,
}

struct TableWorkspace<'source> {
    indices: RefCell<Vec<Table<'source, 'source>>>,
    // No source cancellation door exists; this private control has no deadline.
    control: zetesis_cpu::Control,
}

impl<'source> Deref for Support<'source> {
    type Target = Relations<'source>;

    fn deref(&self) -> &Self::Target {
        self.relations
    }
}

impl<'source> Support<'source> {
    pub(crate) fn indexed(
        relations: &'source Relations<'source>,
        limits: &FormulaLimits,
        counters: &Counters,
        location: Location,
    ) -> Result<Self, FormulaFailure> {
        Self::completed(relations, JoinStrategy::Indexed, limits, counters, location)
    }

    pub(super) fn completed(
        relations: &'source Relations<'source>,
        strategy: JoinStrategy,
        limits: &FormulaLimits,
        counters: &Counters,
        location: Location,
    ) -> Result<Self, FormulaFailure> {
        let bytes = relations
            .bytes
            .checked_add(size_of::<Self>())
            .ok_or_else(|| allocation(Cause::Overflow, location))?;
        ceiling(
            FormulaResource::SupportBytes,
            bytes as u128,
            limits.max_support_bytes as u128,
            location,
        )?;
        counters.record(Event::SupportPeakBytes(bytes as u128));
        Ok(Self {
            relations,
            tables: (strategy == JoinStrategy::Table).then(|| TableWorkspace {
                indices: RefCell::new(Vec::new()),
                control: zetesis_cpu::Control::default(),
            }),
            live: Cell::new(bytes),
            entries: Cell::new(relations.entries),
        })
    }

    pub(super) fn contains(
        &self,
        key: &AtomKey<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<bool, FormulaFailure> {
        let scoped = self.indexed_limits(limits, counters, location)?;
        self.relations
            .contains(key, &scoped, counters, location)
            .map_err(|error| self.indexed_failure(error))
    }

    pub(super) fn probe(
        &self,
        pattern: &AtomPattern,
        values: &[Option<Value>],
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<&[usize]>, FormulaFailure> {
        Self::admit(self.live.get(), limits, counters, location)?;
        counters.record(Event::IndexedProbe);
        self.relations.probe_with_bytes(
            pattern,
            values,
            limits,
            counters,
            location,
            self.live.get() - self.relations.bytes,
        )
    }

    fn indexed_limits(
        &self,
        limits: &FormulaLimits,
        counters: &Counters,
        location: Location,
    ) -> Result<FormulaLimits, FormulaFailure> {
        Self::admit(self.live.get(), limits, counters, location)?;
        let mut scoped = *limits;
        scoped.max_support_bytes -= self.live.get() - self.relations.bytes;
        Ok(scoped)
    }

    fn indexed_failure(&self, error: FormulaFailure) -> FormulaFailure {
        match error {
            FormulaFailure::Limit {
                resource: FormulaResource::SupportBytes,
                observed,
                limit,
                location,
            } => {
                let outer = (self.live.get() - self.relations.bytes) as u128;
                FormulaFailure::Limit {
                    resource: FormulaResource::SupportBytes,
                    observed: observed + outer,
                    limit: limit + outer,
                    location,
                }
            }
            other => other,
        }
    }

    /// Only an absent strategy, structural pattern or absent relation declines
    /// this operation. Every attempted preparation/selection failure propagates.
    pub(super) fn select(
        &self,
        pattern: PositivePattern<'_>,
        values: &[Option<Value>],
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<Rows<'_, 'source>>, FormulaFailure> {
        let Some(workspace) = &self.tables else {
            return Ok(None);
        };
        let control = &workspace.control;
        let PositivePattern::Flat(pattern) = pattern else {
            counters.record(Event::TableInapplicableProbe);
            return Ok(None);
        };
        let Some(relation) = self.relations.relation(pattern.predicate()) else {
            counters.record(Event::TableInapplicableProbe);
            return Ok(None);
        };
        Self::admit(self.live.get(), limits, counters, location)?;
        let mut scratch = Scratch::new(self, limits, counters, location)?;
        let bound = bind_domains(pattern, values, &mut scratch, counters)?;
        let mut tables = workspace.indices.borrow_mut();
        let found = find_table(&tables, pattern, &bound.scope, limits, counters, location)?;
        let index = if let Some(index) = found {
            counters.record(Event::TableReuse);
            index
        } else {
            self.reserve_tables(&mut tables, limits, counters, location)?;
            let outer = self.live.get() - relation.storage().retained_bytes;
            let table_limits = self.table_limits(limits, counters, outer, location)?;
            let prepared = Table::prepare(relation, &bound.scope, table_limits, control);
            let table = self.result(prepared, true, outer, limits, counters, location)?;
            let retained = table.statistics().retained_bytes - size_of::<Table<'_, '_>>();
            self.live.set(self.live.get() + retained);
            self.entries
                .set(self.entries.get() + table.support_entries());
            counters.record(Event::TableIndexBytes(retained));
            let index = tables.len();
            tables.push(table);
            counters.record(Event::TablePreparation);
            index
        };
        let table = &tables[index];
        let outer =
            self.live.get() - relation.storage().retained_bytes - table.statistics().retained_bytes
                + ROW_LEASE_BYTES;
        let mut table_limits = self.table_limits(limits, counters, outer, location)?;
        // The existing table already owns these admitted entries; selection
        // checks the input's count, rather than adding entries to the cache.
        table_limits.max_entries = table.support_entries();
        counters.record(Event::JoinProbe);
        counters.record(Event::TableProbe);
        let selected = table.select(&bound.domains, table_limits, control);
        let selection = self.result(selected, false, outer, limits, counters, location)?;
        let bytes = selection.statistics().retained_bytes + ROW_LEASE_BYTES;
        self.live.set(self.live.get() + bytes);
        Ok(Some(Rows {
            selection,
            live: &self.live,
            bytes,
        }))
    }

    fn table_limits(
        &self,
        limits: &FormulaLimits,
        counters: &Counters,
        outer: usize,
        location: Location,
    ) -> Result<table::Limits, FormulaFailure> {
        Self::admit(self.live.get(), limits, counters, location)?;
        Ok(table::Limits {
            max_entries: limits
                .max_support_index_entries
                .saturating_sub(self.entries.get()),
            max_bytes: limits.max_support_bytes - outer,
            max_work: limits.max_work.saturating_sub(counters.work),
        })
    }

    fn result<T: Receipt>(
        &self,
        result: Result<T, table::Failure>,
        preparing: bool,
        outer: usize,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<T, FormulaFailure> {
        let base_work = counters.work;
        let (work, peak) = match &result {
            Ok(value) => (value.receipt().work, value.receipt().peak_bytes),
            Err(error) => (error.work, error.peak_bytes),
        };
        counters.charge_work(u128::from(work), limits, location)?;
        counters.record(if preparing {
            Event::TablePrepareWork(work)
        } else {
            Event::TableQueryWork(work)
        });
        counters.record(Event::SupportPeakBytes(outer as u128 + peak as u128));
        result.map_err(|error| {
            let mapped = match &error.cause {
                Cause::Limit {
                    resource: Resource::Work,
                    observed,
                    ..
                } => Some((
                    FormulaResource::Work,
                    u128::from(base_work) + observed,
                    u128::from(limits.max_work),
                )),
                Cause::Limit {
                    resource: Resource::Bytes,
                    observed,
                    ..
                } => Some((
                    FormulaResource::SupportBytes,
                    outer as u128 + observed,
                    limits.max_support_bytes as u128,
                )),
                Cause::Limit {
                    resource: Resource::Entries,
                    observed,
                    ..
                } if preparing => Some((
                    FormulaResource::SupportIndexEntries,
                    self.entries.get() as u128 + observed,
                    limits.max_support_index_entries as u128,
                )),
                _ => None,
            };
            mapped.map_or(
                FormulaFailure::SupportTable { error, location },
                |(resource, observed, limit)| FormulaFailure::Limit {
                    resource,
                    observed,
                    limit,
                    location,
                },
            )
        })
    }

    fn admit(
        bytes: usize,
        limits: &FormulaLimits,
        counters: &Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        ceiling(
            FormulaResource::SupportBytes,
            bytes as u128,
            limits.max_support_bytes as u128,
            location,
        )?;
        counters.record(Event::SupportPeakBytes(bytes as u128));
        Ok(())
    }

    fn reserve_tables(
        &self,
        tables: &mut Vec<Table<'source, 'source>>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<(), FormulaFailure> {
        if tables.len() < tables.capacity() {
            return Ok(());
        }
        let old = tables.capacity() * size_of::<Table<'_, '_>>();
        let desired = tables
            .len()
            .saturating_add(1)
            .max(tables.capacity().saturating_mul(2));
        let next = desired
            .checked_mul(size_of::<Table<'_, '_>>())
            .ok_or_else(|| allocation(Cause::Overflow, location))?;
        let peak = self
            .live
            .get()
            .checked_add(next)
            .ok_or_else(|| allocation(Cause::Overflow, location))?;
        Self::admit(peak, limits, counters, location)?;
        counters.charge_work(tables.len() as u128, limits, location)?;
        tables
            .try_reserve_exact(desired - tables.len())
            .map_err(|_| allocation(Cause::Allocation, location))?;
        let actual = tables.capacity() * size_of::<Table<'_, '_>>();
        let peak = self
            .live
            .get()
            .checked_add(actual)
            .ok_or_else(|| allocation(Cause::Overflow, location))?;
        self.live.set(self.live.get() - old + actual);
        counters.record(Event::SupportPeakBytes(peak as u128));
        Self::admit(peak, limits, counters, location)
    }
}

struct BoundDomains<'value> {
    scope: Vec<usize>,
    domains: Vec<Domain<'value>>,
}

/// Canonical table labels are distinct from source variable slots. Each
/// constant owns one label; repeated source slots share their first label.
/// The descriptors borrow whole values only for this call. None means an
/// unbound valid slot, whereas an out-of-range source slot is a typed failure.
fn bind_domains<'value>(
    pattern: &'value AtomPattern,
    values: &'value [Option<Value>],
    scratch: &mut Scratch<'_, '_>,
    counters: &mut Counters,
) -> Result<BoundDomains<'value>, FormulaFailure> {
    let limits = scratch.limits;
    let location = scratch.location;
    let mut scope = Vec::new();
    let mut domains = Vec::new();
    scratch.reserve(&mut scope, pattern.terms().len(), counters)?;
    scratch.reserve(&mut domains, pattern.terms().len(), counters)?;
    for (column, term) in pattern.terms().iter().enumerate() {
        counters.work(limits, location)?;
        let mut alias = None;
        if let Term::Variable(variable) = term {
            for (previous, other) in pattern.terms()[..column].iter().enumerate() {
                counters.work(limits, location)?;
                if other == &Term::Variable(*variable) {
                    alias = Some(scope[previous]);
                    break;
                }
            }
        }
        let variable = alias.unwrap_or(domains.len());
        scope.push(variable);
        if alias.is_none() {
            let value = match term {
                Term::Constant(value) => Some(value),
                Term::Variable(variable) => values
                    .get(*variable)
                    .ok_or(FormulaFailure::UnsafeVariable {
                        variable: *variable,
                        location,
                    })?
                    .as_ref(),
            };
            domains.push(value.map_or(Domain::Unrestricted, Domain::Singleton));
        }
    }
    Ok(BoundDomains { scope, domains })
}

fn find_table(
    tables: &[Table<'_, '_>],
    pattern: &AtomPattern,
    scope: &[usize],
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: Location,
) -> Result<Option<usize>, FormulaFailure> {
    let mut found = None;
    for (index, table) in tables.iter().enumerate() {
        counters.charge_work(
            1 + pattern.predicate().name().len() as u128 + scope.len() as u128,
            limits,
            location,
        )?;
        if table.relation().predicate() == pattern.predicate() && table.scope() == scope {
            found = Some(index);
            break;
        }
    }
    Ok(found)
}

trait Receipt {
    fn receipt(&self) -> table::Statistics;
}
impl Receipt for Table<'_, '_> {
    fn receipt(&self) -> table::Statistics {
        self.statistics()
    }
}
impl Receipt for Selection<'_, '_> {
    fn receipt(&self) -> table::Statistics {
        self.statistics()
    }
}

const ROW_LEASE_BYTES: usize =
    size_of::<Rows<'static, 'static>>() - size_of::<Selection<'static, 'static>>();

/// The selection owns its mask; this lease accounts for all active join depths.
pub(super) struct Rows<'owner, 'source> {
    selection: Selection<'source, 'source>,
    live: &'owner Cell<usize>,
    bytes: usize,
}
impl Rows<'_, '_> {
    pub(super) fn next(
        &self,
        from: usize,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<usize>, FormulaFailure> {
        self.selection
            .next_row_with(from, || counters.work(limits, location))
    }
}
impl Drop for Rows<'_, '_> {
    fn drop(&mut self) {
        self.live.set(self.live.get() - self.bytes);
    }
}

struct Scratch<'a, 'source> {
    support: &'a Support<'source>,
    limits: &'a FormulaLimits,
    location: Location,
    bytes: usize,
}
impl<'a, 'source> Scratch<'a, 'source> {
    fn new(
        support: &'a Support<'source>,
        limits: &'a FormulaLimits,
        counters: &Counters,
        location: Location,
    ) -> Result<Self, FormulaFailure> {
        let bytes = 2 * size_of::<Vec<usize>>() + size_of::<Self>();
        let total = support
            .live
            .get()
            .checked_add(bytes)
            .ok_or_else(|| allocation(Cause::Overflow, location))?;
        Support::admit(total, limits, counters, location)?;
        support.live.set(total);
        Ok(Self {
            support,
            limits,
            location,
            bytes,
        })
    }
    fn reserve<T>(
        &mut self,
        values: &mut Vec<T>,
        count: usize,
        counters: &mut Counters,
    ) -> Result<(), FormulaFailure> {
        let bytes = count
            .checked_mul(size_of::<T>())
            .ok_or_else(|| allocation(Cause::Overflow, self.location))?;
        let proposed = self
            .support
            .live
            .get()
            .checked_add(bytes)
            .ok_or_else(|| allocation(Cause::Overflow, self.location))?;
        Support::admit(proposed, self.limits, counters, self.location)?;
        values
            .try_reserve_exact(count)
            .map_err(|_| allocation(Cause::Allocation, self.location))?;
        let actual = values
            .capacity()
            .checked_mul(size_of::<T>())
            .ok_or_else(|| allocation(Cause::Overflow, self.location))?;
        let owned = self
            .bytes
            .checked_add(actual)
            .ok_or_else(|| allocation(Cause::Overflow, self.location))?;
        let total = self
            .support
            .live
            .get()
            .checked_add(actual)
            .ok_or_else(|| allocation(Cause::Overflow, self.location))?;
        self.bytes = owned;
        self.support.live.set(total);
        counters.record(Event::SupportPeakBytes(total as u128));
        Support::admit(total, self.limits, counters, self.location)
    }
}
impl Drop for Scratch<'_, '_> {
    fn drop(&mut self) {
        self.support.live.set(self.support.live.get() - self.bytes);
    }
}

fn allocation(cause: Cause, location: Location) -> FormulaFailure {
    FormulaFailure::SupportTable {
        error: table::Failure {
            cause,
            work: 0,
            peak_bytes: 0,
        },
        location,
    }
}
