//! Reusable queries borrow one immutable relation snapshot.
//!
//! Growing snapshots admit indexed queries only. A completed snapshot can own
//! shared table indices; live selections borrow its relations, so inserting a
//! later index never invalidates an earlier join depth. No tuple payload moves.

#[cfg(test)]
mod tests;

use std::cell::{Cell, OnceCell, RefCell};
use std::mem::size_of;
use std::ops::{Deref, Range};

use themelios_base::span::Location;
use zetesis_core::catalog::TermRef;
use zetesis_core::{AtomKey, BindingView, PatternRef, TemplateTerm};
use zetesis_cpu::table::{self, Cause, Domain, Resource, Selection, Table};

use super::relations::RelationRows;
use super::{Counters, GroundingWork, PositivePattern, Relations};
use crate::formula::ceiling;
use crate::grounding_observer::Event;
use crate::{FormulaFailure, FormulaLimits, FormulaResource, JoinStrategy};

mod domains;
pub(crate) use domains::{Candidates, Guards};

/// Query state is separate from both the catalog and its immutable row views.
/// The single-threaded source builder owns this workspace; table indices are
/// immutable after publication, while every query owns its independent mask.
pub(crate) struct Support<'source> {
    relations: &'source Relations<'source>,
    tables: OnceCell<TableWorkspace<'source>>,
    table_strategy: bool,
    live: Cell<usize>,
    entries: Cell<usize>,
    workspace: super::storage::Workspace,
}

struct TableWorkspace<'source> {
    indices: RefCell<Vec<Table<'source, 'source>>>,
    // No source cancellation door exists; this private control has no deadline.
    cancellation: zetesis_cpu::Cancellation,
}

impl<'source> Deref for Support<'source> {
    type Target = Relations<'source>;

    fn deref(&self) -> &Self::Target {
        self.relations
    }
}

impl<'source> Support<'source> {
    pub(super) fn workspace(&self) -> &super::storage::Workspace {
        &self.workspace
    }

    fn append_bytes(&self) -> usize {
        self.relations.current_bytes() - self.relations.bytes
    }
    pub(super) fn live_bytes(&self) -> usize {
        self.live.get() + self.append_bytes() + self.workspace.bytes()
    }
    fn bytes_with_append(&self, base: usize, location: Location) -> Result<usize, FormulaFailure> {
        base.checked_add(self.append_bytes())
            .and_then(|bytes| bytes.checked_add(self.workspace.bytes()))
            .ok_or_else(|| allocation(Cause::Overflow, location))
    }
    /// Query descriptors and active guard/index leases, excluding canonical growth.
    pub(super) fn workspace_bytes(&self) -> usize {
        self.live.get() - self.relations.bytes + self.workspace.bytes()
    }

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
        let live = bytes
            .checked_add(counters.accounting.workspace.bytes())
            .ok_or_else(|| allocation(Cause::Overflow, location))?;
        ceiling(
            FormulaResource::SupportBytes,
            live as u128,
            limits.max_support_bytes as u128,
            location,
        )?;
        counters.record(Event::SupportPeakBytes(live as u128));
        Ok(Self {
            relations,
            workspace: counters.accounting.workspace.clone(),
            tables: OnceCell::new(),
            table_strategy: strategy == JoinStrategy::Table,
            live: Cell::new(bytes),
            entries: Cell::new(relations.entries),
        })
    }

    pub(crate) fn contains(
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

    /// A borrowed row owner is scoped to this query snapshot. It remains valid
    /// across join backtracking; no growing-directory index is retained.
    pub(super) fn resolve(
        &self,
        pattern: PatternRef<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<&'source RelationRows<'source>>, FormulaFailure> {
        Self::admit(self.live_bytes(), limits, counters, location)?;
        self.relations
            .find_with(pattern.predicate(), limits, counters, location)
    }

    #[cfg(test)]
    pub(super) fn probe(
        &self,
        pattern: PatternRef<'_>,
        values: BindingView<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<&[usize]>, FormulaFailure> {
        let rows = self.resolve(pattern, limits, counters, location)?;
        self.probe_at(rows, pattern, values, limits, counters, location)
    }

    pub(super) fn probe_at(
        &self,
        rows: Option<&'source RelationRows<'source>>,
        pattern: PatternRef<'_>,
        values: BindingView<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<&'source [usize]>, FormulaFailure> {
        Self::admit(self.live_bytes(), limits, counters, location)?;
        counters.record(Event::IndexedProbe);
        self.relations.probe_at(
            rows,
            pattern,
            values,
            self.live_bytes() - self.relations.current_bytes(),
            GroundingWork::new(limits, counters, location),
        )
    }

    fn indexed_limits(
        &self,
        limits: &FormulaLimits,
        counters: &Counters,
        location: Location,
    ) -> Result<FormulaLimits, FormulaFailure> {
        Self::admit(self.live_bytes(), limits, counters, location)?;
        let mut scoped = *limits;
        scoped.max_support_bytes -= self.live_bytes() - self.relations.current_bytes();
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
                let outer = (self.live_bytes() - self.relations.current_bytes()) as u128;
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
    #[cfg(test)]
    pub(super) fn select(
        &self,
        pattern: PositivePattern<'_>,
        values: BindingView<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<Rows<'_, 'source>>, FormulaFailure> {
        let rows = if self.table_strategy && matches!(pattern, PositivePattern::Flat(_)) {
            self.resolve(pattern.atom(), limits, counters, location)?
        } else {
            None
        };
        self.select_at(rows, pattern, values, limits, counters, location)
    }

    pub(super) fn select_at(
        &self,
        rows: Option<&'source RelationRows<'source>>,
        pattern: PositivePattern<'_>,
        values: BindingView<'_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: Location,
    ) -> Result<Option<Rows<'_, 'source>>, FormulaFailure> {
        if !self.table_strategy {
            return Ok(None);
        }
        self.select_domains_at(
            rows,
            pattern,
            values,
            FiniteDomains::default(),
            GroundingWork::new(limits, counters, location),
        )
    }

    /// Select necessary finite domains with the same table and mask workspace
    /// used by ordinary table joins, including when ordinary joins are indexed.
    /// The caller establishes coverage before excluding source values. A finite
    /// restriction names one unbound variable present in this flat pattern;
    /// ordinary bound variables and constants retain their singleton domains.
    /// Ranges are borrowed for this call only; returned rows keep source IDs.
    pub(super) fn select_domains_at<'value>(
        &self,
        rows: Option<&'source RelationRows<'source>>,
        pattern: PositivePattern<'value>,
        values: BindingView<'value>,
        finite: FiniteDomains<'value>,
        work: GroundingWork<'_>,
    ) -> Result<Option<Rows<'_, 'source>>, FormulaFailure> {
        let GroundingWork {
            limits,
            counters,
            location,
        } = work;
        let PositivePattern::Flat(pattern) = pattern else {
            counters.record(Event::TableInapplicableProbe);
            return Ok(None);
        };
        let Some(relation) = rows.map(|rows| &rows.relation) else {
            counters.record(Event::TableInapplicableProbe);
            return Ok(None);
        };
        Self::admit(self.live_bytes(), limits, counters, location)?;
        let mut scratch = Scratch::new(self, limits, counters, location)?;
        let bound = bind_domains(pattern, values, finite, &mut scratch, counters)?;
        let workspace = self.tables.get_or_init(|| TableWorkspace {
            indices: RefCell::new(Vec::new()),
            cancellation: zetesis_cpu::Cancellation::default(),
        });
        let cancellation = &workspace.cancellation;
        let mut tables = workspace.indices.borrow_mut();
        let found = find_table(&tables, pattern, &bound.scope, limits, counters, location)?;
        let index = if let Some(index) = found {
            counters.record(Event::TableReuse);
            index
        } else {
            self.reserve_tables(&mut tables, limits, counters, location)?;
            let outer = self.live_bytes() - relation.storage().retained_bytes;
            let table_limits = self.table_limits(limits, counters, outer, location)?;
            let prepared = Table::prepare(relation, &bound.scope, table_limits, cancellation);
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
        let outer = self.live_bytes()
            - relation.storage().retained_bytes
            - table.statistics().retained_bytes
            + ROW_LEASE_BYTES;
        let mut table_limits = self.table_limits(limits, counters, outer, location)?;
        // The existing table already owns these admitted entries; selection
        // checks the input's count, rather than adding entries to the cache.
        table_limits.max_entries = table.support_entries();
        counters.record(Event::JoinProbe);
        counters.record(Event::TableProbe);
        let selected = table.select(&bound.domains, table_limits, cancellation);
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
        Self::admit(self.live_bytes(), limits, counters, location)?;
        Ok(table::Limits {
            max_entries: limits
                .max_support_index_entries
                .saturating_sub(self.entries.get()),
            max_bytes: limits.max_support_bytes - outer,
            max_work: limits.max_work.saturating_sub(counters.accounting.work),
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
        let base_work = counters.accounting.work;
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
            .live_bytes()
            .checked_add(next)
            .ok_or_else(|| allocation(Cause::Overflow, location))?;
        Self::admit(peak, limits, counters, location)?;
        counters.charge_work(tables.len() as u128, limits, location)?;
        tables
            .try_reserve_exact(desired - tables.len())
            .map_err(|_| allocation(Cause::Allocation, location))?;
        let actual = tables.capacity() * size_of::<Table<'_, '_>>();
        let peak = self
            .live_bytes()
            .checked_add(actual)
            .ok_or_else(|| allocation(Cause::Overflow, location))?;
        self.live.set(self.live.get() - old + actual);
        counters.record(Event::SupportPeakBytes(peak as u128));
        Self::admit(peak, limits, counters, location)
    }
}

/// Borrowed necessary domains over source variable slots. One value buffer
/// holds every range; an empty range excludes all values of that variable.
/// The caller charges the buffers and retains their canonical value owners.
#[derive(Clone, Copy, Default)]
pub(super) struct FiniteDomains<'value> {
    pub(super) values: &'value [TermRef<'value>],
    pub(super) variables: &'value [(usize, Range<usize>)],
}

impl FiniteDomains<'_> {
    fn validate(
        &self,
        pattern: PatternRef<'_>,
        values: BindingView<'_>,
        work: &mut GroundingWork<'_>,
    ) -> Result<(), FormulaFailure> {
        for (index, (variable, range)) in self.variables.iter().enumerate() {
            work.counters.work(work.limits, work.location)?;
            if *variable >= values.len() {
                return Err(FormulaFailure::UnsafeVariable {
                    variable: *variable,
                    location: work.location,
                });
            }
            if values.get(*variable).is_some() || self.values.get(range.clone()).is_none() {
                return Err(allocation(Cause::Domains, work.location));
            }
            for (earlier, _) in &self.variables[..index] {
                work.counters.work(work.limits, work.location)?;
                if earlier == variable {
                    return Err(allocation(Cause::Domains, work.location));
                }
            }
            let terms = pattern.terms();
            let mut present = false;
            for column in 0..terms.len() {
                work.counters.work(work.limits, work.location)?;
                present |= terms.at(column) == Some(TemplateTerm::Variable(*variable));
            }
            if !present {
                return Err(allocation(Cause::Domains, work.location));
            }
        }
        Ok(())
    }
}

impl<'value> FiniteDomains<'value> {
    fn domain(
        &self,
        variable: usize,
        work: &mut GroundingWork<'_>,
    ) -> Result<Option<Domain<'value>>, FormulaFailure> {
        for (slot, range) in self.variables {
            work.counters.work(work.limits, work.location)?;
            if *slot == variable {
                let values = self
                    .values
                    .get(range.clone())
                    .expect("validated finite domain");
                return Ok(Some(Domain::Finite(values.into())));
            }
        }
        Ok(None)
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
    pattern: PatternRef<'value>,
    values: BindingView<'value>,
    finite: FiniteDomains<'value>,
    scratch: &mut Scratch<'_, '_>,
    counters: &mut Counters,
) -> Result<BoundDomains<'value>, FormulaFailure> {
    let limits = scratch.limits;
    let location = scratch.location;
    finite.validate(
        pattern,
        values,
        &mut GroundingWork::new(limits, counters, location),
    )?;
    let mut scope = Vec::new();
    let mut domains = Vec::new();
    scratch.reserve(&mut scope, pattern.terms().len(), counters)?;
    scratch.reserve(&mut domains, pattern.terms().len(), counters)?;
    let terms = pattern.terms();
    for column in 0..terms.len() {
        counters.work(limits, location)?;
        let term = terms.at(column).expect("checked pattern arity");
        let mut alias = None;
        if let TemplateTerm::Variable(variable) = term {
            for (previous, prior_scope) in scope.iter().enumerate().take(column) {
                counters.work(limits, location)?;
                let other = terms.at(previous).expect("checked pattern prefix");
                if other == TemplateTerm::Variable(variable) {
                    alias = Some(*prior_scope);
                    break;
                }
            }
        }
        let variable = alias.unwrap_or(domains.len());
        scope.push(variable);
        if alias.is_none() {
            let value = match term {
                TemplateTerm::Constant(value) => Some(value),
                TemplateTerm::Variable(variable) => {
                    if variable >= values.len() {
                        return Err(FormulaFailure::UnsafeVariable { variable, location });
                    }
                    values.get(variable)
                }
            };
            let finite = if let TemplateTerm::Variable(variable) = term {
                finite.domain(
                    variable,
                    &mut GroundingWork::new(limits, counters, location),
                )?
            } else {
                None
            };
            domains.push(
                finite.unwrap_or_else(|| value.map_or(Domain::Unrestricted, Domain::Singleton)),
            );
        }
    }
    Ok(BoundDomains { scope, domains })
}

fn find_table(
    tables: &[Table<'_, '_>],
    pattern: PatternRef<'_>,
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
        Support::admit(
            support.bytes_with_append(total, location)?,
            limits,
            counters,
            location,
        )?;
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
        Support::admit(
            self.support.bytes_with_append(proposed, self.location)?,
            self.limits,
            counters,
            self.location,
        )?;
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
        counters.record(Event::SupportPeakBytes(
            total as u128
                + self.support.append_bytes() as u128
                + self.support.workspace.bytes() as u128,
        ));
        Support::admit(
            self.support.bytes_with_append(total, self.location)?,
            self.limits,
            counters,
            self.location,
        )
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
