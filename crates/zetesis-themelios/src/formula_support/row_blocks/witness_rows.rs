//! Exact witnessed source occurrences cross a task boundary; projection does not.
//!
//! A worker stages only the changed suffix already reported by the ordinary
//! witness cursor. Its receipt remains separate from success. After all workers
//! settle, the coordinator resolves those occurrences against their retained
//! immutable sources and uses the existing `RowHead` projection and canonical
//! writer. No assignment frame or typed source value is copied into transport.

mod replay;
mod binding;
pub(crate) mod retained;
pub(crate) use binding::RowBinding;
#[cfg(test)]
mod tests;

use std::ops::Range;

#[cfg(test)]
use zetesis_core::atom_registry::TermLookup;
use zetesis_core::relation::Row;

use super::prime::prefixes::Prefixes;
pub(super) use super::prime::{PackedCoords, width_for_index};
use super::{
    Binding, Buffer, Computation, Context, Counters, GroundingWork, HeadIr, Ir, Join, JoinPlan,
    Probe, StorageLease, Support, cursor_rows::Descriptor, delta, relations,
};
use crate::formula_support::{Resolution, RowHead, witnesses::WitnessRow};
use crate::grounding_observer::Event;
use crate::{FormulaFailure, FormulaLimits, ProgramSite, expansion::Budget};

/// Sources are borrowed from this exact immutable epoch, independently of the
/// temporary full-vocabulary lookup each worker must release before publication.
/// Every source coordinate below is tied to this plan, never a free row number.
pub(crate) struct WitnessPlan<'plan, 'source> {
    base: Descriptor<'plan, 'source>,
    purpose: Purpose,
    relations: &'source relations::Relations<'source>,
    sources: Buffer<Resolution<'source>>,
    coord_width: super::prime::CoordWidth,
    _lease: StorageLease,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Purpose {
    Support,
    Emission,
}

pub(crate) use replay::Replay;

impl<'plan, 'source: 'plan> WitnessPlan<'plan, 'source> {
    pub(super) fn new(
        base: Descriptor<'plan, 'source>,
        relations: &'source relations::Relations<'source>,
        support: &Support<'source>,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<Self, FormulaFailure> {
        Self::with_purpose(
            base,
            relations,
            support,
            computation,
            limits,
            counters,
            Purpose::Support,
        )
    }

    pub(crate) fn for_emission(
        base: Descriptor<'plan, 'source>,
        support: &Support<'source>,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<Self, FormulaFailure> {
        Self::with_purpose(
            base,
            support.relations(),
            support,
            computation,
            limits,
            counters,
            Purpose::Emission,
        )
    }

    fn with_purpose(
        base: Descriptor<'plan, 'source>,
        relations: &'source relations::Relations<'source>,
        support: &Support<'source>,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        purpose: Purpose,
    ) -> Result<Self, FormulaFailure> {
        let location = base.location();
        counters.work(limits, location)?;
        if !std::ptr::eq(
            std::ptr::from_ref(relations),
            std::ptr::from_ref(&**support),
        ) {
            return Err(owner(location));
        }
        counters.work(limits, location)?;
        if !std::ptr::eq(relations, base.relations) {
            return Err(owner(location));
        }
        let mut lease = computation.lease();
        let header = size_of::<Self>() - size_of::<Buffer<Resolution<'source>>>();
        lease.observe(header, location)?;
        computation.storage_observed(&lease, 0, header, limits, counters, location)?;
        let mut sources = Buffer::new(computation, limits, counters, location)?;
        sources.reserve(
            base.plan.patterns.len(),
            computation,
            limits,
            counters,
            location,
        )?;
        for pattern in &base.plan.patterns {
            counters.work(limits, location)?;
            let rows = support.resolve(pattern.atom(), limits, counters, location)?;
            sources.push(
                Resolution::Resolved {
                    rows,
                    filter_source: None,
                },
                computation,
                limits,
                counters,
                location,
            )?;
        }
        let mut max_index = base.rows.iter().copied().max().unwrap_or(0);
        for source in sources.slice() {
            if let Some(rows) = source.rows() {
                max_index = max_index.max(rows.row_count().saturating_sub(1));
            }
        }
        let coord_width = width_for_index(max_index, location)?;
        Ok(Self {
            base,
            purpose,
            relations,
            sources,
            coord_width,
            _lease: lease,
        })
    }

    pub(crate) fn len(&self) -> usize {
        self.base.len()
    }

    pub(crate) fn location(&self) -> ProgramSite {
        self.base.location()
    }

    pub(crate) fn groups(&self) -> usize {
        self.base.groups()
    }

    pub(crate) fn prime(&self) -> bool {
        self.base.prime
    }

    pub(crate) fn first_rows(&self) -> &[usize] {
        self.base.rows
    }

    pub(crate) fn rule(&self) -> &'source crate::formula_ir::RuleIr {
        self.base.rule
    }

    pub(in crate::formula_support) fn join_plan(&self) -> &'plan super::order::Plan<'plan> {
        self.base.plan
    }

    pub(crate) fn pattern_count(&self) -> usize {
        self.base.plan.patterns.len()
    }

    pub(crate) fn max_row_index(&self) -> usize {
        let mut max_index = self.base.rows.iter().copied().max().unwrap_or(0);
        for source in self.sources.slice() {
            if let Some(rows) = source.rows() {
                max_index = max_index.max(rows.row_count().saturating_sub(1));
            }
        }
        max_index
    }
}

#[derive(Clone, Copy)]
struct Completion {
    unchanged: usize,
    suffix_end: usize,
}

/// The immutable plan keeps the exact source owners alive. Task lookup scratch
/// is deliberately absent, so returning this batch does not prolong its borrow
/// of the canonical appender and prevent ordered publication.
pub(crate) struct Batch<'plan, 'source> {
    plan: &'plan WitnessPlan<'plan, 'source>,
    completions: Buffer<Completion>,
    suffixes: PackedCoords,
    _lease: StorageLease,
}

impl<'plan, 'source> Batch<'plan, 'source> {
    fn new(
        plan: &'plan WitnessPlan<'plan, 'source>,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<Self, FormulaFailure> {
        let location = plan.location();
        let mut lease = computation.lease();
        let header =
            size_of::<Self>() - size_of::<Buffer<Completion>>() - size_of::<PackedCoords>();
        lease.observe(header, location)?;
        computation.storage_observed(&lease, 0, header, limits, counters, location)?;
        Ok(Self {
            plan,
            completions: Buffer::new(computation, limits, counters, location)?,
            suffixes: PackedCoords::new(plan.coord_width, computation, limits, counters, location)?,
            _lease: lease,
        })
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.completions.len()
    }

    fn stage(
        &mut self,
        row: &WitnessRow<'_, '_>,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<(), FormulaFailure> {
        if self.suffixes.width() != self.plan.coord_width {
            return Err(owner(self.plan.location()));
        }
        stage(
            self.plan.base.plan.patterns.len(),
            &mut self.completions,
            &mut self.suffixes,
            row,
            Context::new(computation, limits, counters, self.plan.location()),
        )
    }
}

/// One suffix encoder serves task batches and retained complete producer rows.
fn stage(
    width: usize,
    completions: &mut Buffer<Completion>,
    suffixes: &mut PackedCoords,
    row: &WitnessRow<'_, '_>,
    context: Context<'_, &Computation<'_, '_>>,
) -> Result<(), FormulaFailure> {
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location,
            },
    } = context;
    counters.charge_work(2, limits, location)?;
    if row.rows.len() != width
        || row.unchanged > row.rows.len()
        || (completions.len() == 0 && row.unchanged != 0)
    {
        return Err(owner(location));
    }
    suffixes.reserve(
        row.rows.len() - row.unchanged,
        computation,
        limits,
        counters,
        location,
    )?;
    for source in &row.rows[row.unchanged..] {
        counters.charge_work(2, limits, location)?;
        suffixes.push(
            source.position(),
            source.source_index(),
            computation,
            limits,
            counters,
            location,
        )?;
    }
    completions.push(
        Completion {
            unchanged: row.unchanged,
            suffix_end: suffixes.len(),
        },
        computation,
        limits,
        counters,
        location,
    )
}

/// `lookup` may borrow the appender for less than the retained source epoch.
/// Staging original occurrence coordinates separates those two lifetimes; no
/// extra source owner, unsafe lifetime extension or copied matching is needed.
#[cfg(test)]
pub(super) fn execute<'plan, 'source, 'read>(
    prepared: &'plan WitnessPlan<'plan, 'source>,
    block: Range<usize>,
    lookup: TermLookup<'read>,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
) -> Result<Batch<'plan, 'source>, FormulaFailure>
where
    'source: 'plan + 'read,
{
    let location = prepared.location();
    let rows = prepared
        .base
        .rows
        .get(block)
        .ok_or_else(|| owner(location))?;
    let support = Support::indexed(prepared.relations, limits, counters, location)?;
    let mut computation = Computation::frozen(lookup, &support);
    traverse(
        prepared,
        rows,
        &support,
        budget,
        Context::new(&mut computation, limits, counters, location),
    )
}

/// This private read sees heads already pending at task entry. It does not see
/// heads staged by any task until the coordinator commits the complete batch.
pub(crate) fn execute_borrowed<'plan, 'source>(
    prepared: &'plan WitnessPlan<'plan, 'source>,
    block: Range<usize>,
    append: &crate::formula_support::SupportAppend<'source>,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
) -> Result<Batch<'plan, 'source>, FormulaFailure>
where
    'source: 'plan,
{
    let location = prepared.location();
    let rows = prepared
        .base
        .rows
        .get(block)
        .ok_or_else(|| owner(location))?;
    let support = Support::indexed(prepared.relations, limits, counters, location)?;
    let mut computation = Computation::borrowed(append, &support, limits, counters, location)?;
    traverse(
        prepared,
        rows,
        &support,
        budget,
        Context::new(&mut computation, limits, counters, location),
    )
}

fn traverse<'plan, 'source, 'read>(
    prepared: &'plan WitnessPlan<'plan, 'source>,
    rows: &[usize],
    support: &Support<'read>,
    budget: &mut Budget,
    context: Context<'_, &mut Computation<'_, 'read>>,
) -> Result<Batch<'plan, 'source>, FormulaFailure>
where
    'source: 'plan + 'read,
{
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location,
            },
    } = context;
    let mut operation = |counters: &mut Counters| {
        let mut result = Batch::new(prepared, computation, limits, counters)?;
        if rows.is_empty() {
            return Ok(result);
        }
        let prefix = Binding::new(computation, limits, counters, location)?;
        let mut join = Join::with_plan(
            &prepared.base.rule.body,
            &prefix,
            prepared.base.rule.variables,
            support,
            JoinPlan::Borrowed(prepared.base.plan),
            None,
            Context::new(computation, limits, counters, location),
        )?;
        join.configure_rule(Ir::Source(prepared.base.rule), limits, counters)?;
        if prepared.prime() {
            let skip = match super::prime::classify_body(
                &prepared.base.rule.body,
                prepared.base.rule.body_variables,
                computation,
                limits,
                counters,
                location,
            )? {
                Some(class) => class.skip,
                None => return Err(owner(location)),
            };
            join.skip_residuals(skip);
        }
        let first = join.plan.patterns[0];
        join.resolve_probe_source(first, GroundingWork::new(limits, counters, location))?;
        join.probes[0] = Some(Probe::Indexed(delta::Rows::Posting(rows)));
        join.retain_witnesses(Context::new(computation, limits, counters, location))?;
        let HeadIr::Normal(Some(head)) = prepared.base.rule.head else {
            unreachable!("admitted normal producer");
        };
        let skip_existing = (prepared.purpose == Purpose::Support).then_some(head);
        while let Some(row) = join.next_witness_row(
            skip_existing,
            budget,
            Context::new(computation, limits, counters, location),
        )? {
            result.stage(&row, computation, limits, counters)?;
        }
        Ok(result)
    };
    if prepared.purpose == Purpose::Support {
        counters.observe_work(Event::SupportJoinWork, operation)
    } else {
        operation(counters)
    }
}

/// Walk each prefix tuple in `block` by seeding a private Join and
/// enumerating its suffix. Used only when A′ chose `d > 1`.
pub(crate) fn execute_prefixed<'plan, 'source>(
    prepared: &'plan WitnessPlan<'plan, 'source>,
    prefixes: &Prefixes<'_>,
    block: Range<usize>,
    append: &crate::formula_support::SupportAppend<'source>,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
) -> Result<Batch<'plan, 'source>, FormulaFailure>
where
    'source: 'plan,
{
    let location = prepared.location();
    if block.end < block.start || block.end > prefixes.len() {
        return Err(owner(location));
    }
    let support = Support::indexed(prepared.relations, limits, counters, location)?;
    let mut computation = Computation::borrowed(append, &support, limits, counters, location)?;
    traverse_prefixed(
        prepared,
        prefixes,
        block,
        &support,
        budget,
        Context::new(&mut computation, limits, counters, location),
    )
}

fn traverse_prefixed<'plan, 'source, 'read>(
    prepared: &'plan WitnessPlan<'plan, 'source>,
    prefixes: &Prefixes<'_>,
    block: Range<usize>,
    support: &Support<'read>,
    budget: &mut Budget,
    context: Context<'_, &mut Computation<'_, 'read>>,
) -> Result<Batch<'plan, 'source>, FormulaFailure>
where
    'source: 'plan + 'read,
{
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location,
            },
    } = context;
    let mut operation = |counters: &mut Counters| {
        let mut result = Batch::new(prepared, computation, limits, counters)?;
        if block.is_empty() {
            return Ok(result);
        }
        let prefix = Binding::new(computation, limits, counters, location)?;
        let mut join = Join::with_plan(
            &prepared.base.rule.body,
            &prefix,
            prepared.base.rule.variables,
            support,
            JoinPlan::Borrowed(prepared.base.plan),
            None,
            Context::new(computation, limits, counters, location),
        )?;
        join.configure_rule(Ir::Source(prepared.base.rule), limits, counters)?;
        let skip = match super::prime::classify_body(
            &prepared.base.rule.body,
            prepared.base.rule.body_variables,
            computation,
            limits,
            counters,
            location,
        )? {
            Some(class) => class.skip,
            None => return Err(owner(location)),
        };
        join.skip_residuals(skip);
        join.retain_witnesses(Context::new(computation, limits, counters, location))?;
        let HeadIr::Normal(Some(head)) = prepared.base.rule.head else {
            unreachable!("admitted normal producer");
        };
        let skip_existing = (prepared.purpose == Purpose::Support).then_some(head);
        for index in block.start..block.end {
            let tuple = prefixes.tuple(index).ok_or_else(|| owner(location))?;
            join.seed_prefix(tuple, location)?;
            while let Some(row) = join.next_witness_row(
                skip_existing,
                budget,
                Context::new(computation, limits, counters, location),
            )? {
                result.stage(&row, computation, limits, counters)?;
            }
        }
        Ok(result)
    };
    if prepared.purpose == Purpose::Support {
        counters.observe_work(Event::SupportJoinWork, operation)
    } else {
        operation(counters)
    }
}

/// All tasks and receipts must settle before this call. Batches are supplied in
/// their original block order. The caller retains the workers' output-capacity
/// reservation outside the coordinator allowance until these buffers drop.
pub(super) fn publish<'plan, 'source: 'plan>(
    prepared: &'plan WitnessPlan<'plan, 'source>,
    batches: impl IntoIterator<Item = impl core::borrow::Borrow<Batch<'plan, 'source>>>,
    support: &Support<'source>,
    computation: &mut Computation<'_, 'source>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    rule_index: usize,
) -> Result<usize, FormulaFailure> {
    let location = prepared.location();
    let HeadIr::Normal(Some(pattern)) = prepared.base.rule.head else {
        unreachable!("admitted normal producer");
    };
    let mut replay = Replay::new(
        prepared,
        support,
        Context::new(&*computation, limits, counters, location),
    )?;
    let skip = if prepared.prime() {
        match super::prime::classify_body(
            &prepared.base.rule.body,
            prepared.base.rule.body_variables,
            computation,
            limits,
            counters,
            location,
        )? {
            Some(class) => class.skip,
            None => return Err(owner(location)),
        }
    } else {
        super::prime::ResidualSkip::none()
    };
    let owner_replay =
        prepared.prime() && super::prime::owner_replays(&prepared.base.rule.body, skip);
    let decided: Vec<usize> = if owner_replay {
        support
            .certainty()
            .map(|certainty| certainty.decided(rule_index).collect())
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let mut row_head = None;
    let mut assignment_head = None;
    let mut binding = None;
    let mut published = 0;
    for batch in batches {
        let batch = core::borrow::Borrow::borrow(&batch);
        let mut rows = replay.batch(batch, limits, counters)?;
        while let Some(row) = rows.next(Context::new(&*computation, limits, counters, location))? {
            if owner_replay {
                let values = match binding.as_mut() {
                    Some(existing) => existing,
                    None => {
                        binding = Some(RowBinding::new(
                            prepared,
                            &row,
                            Context::new(&*computation, limits, counters, location),
                        )?);
                        binding.as_mut().expect("just installed")
                    }
                };
                values.read(
                    &row,
                    Context::new(&*computation, limits, counters, location),
                )?;
                if !values.replay_residuals(
                    skip,
                    decided.iter().copied(),
                    Context::new(computation, limits, counters, location),
                )? {
                    continue;
                }
                counters.observe_work(Event::SupportHeadWork, |counters| {
                    if assignment_head.is_none() {
                        assignment_head = Some(super::super::heads::Head::new(
                            pattern,
                            computation,
                            limits,
                            counters,
                            location,
                        )?);
                    }
                    assignment_head
                        .as_ref()
                        .expect("prepared assignment head")
                        .derive(values.assignment(), computation, limits, counters, location)?;
                    counters.work(limits, location)?;
                    published = zetesis_algebra::Meter::check_count(published, 1, usize::MAX)
                        .map_err(|_| owner(location))?;
                    Ok::<_, FormulaFailure>(())
                })?;
                continue;
            }
            counters.observe_work(Event::SupportHeadWork, |counters| {
                if row_head.is_none() {
                    row_head = Some(RowHead::new(
                        pattern,
                        &row,
                        computation,
                        limits,
                        counters,
                        location,
                    )?);
                }
                let atom = row_head.as_ref().expect("prepared row projection").atom(
                    &row,
                    computation,
                    limits,
                    counters,
                    location,
                )?;
                computation.support(&atom, limits, counters, location)?;
                counters.work(limits, location)?;
                published = zetesis_algebra::Meter::check_count(published, 1, usize::MAX)
                    .map_err(|_| owner(location))?;
                Ok::<_, FormulaFailure>(())
            })?;
        }
    }
    Ok(published)
}

fn restore<'source>(
    rows: &mut Buffer<Row<'source, 'source>>,
    prepared: &WitnessPlan<'_, 'source>,
    suffixes: &PackedCoords,
    start: usize,
    completion: Completion,
    context: Context<'_, &Computation<'_, '_>>,
) -> Result<(), FormulaFailure> {
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location,
            },
    } = context;
    counters.charge_work(2, limits, location)?;
    if completion.unchanged > rows.len() {
        return Err(owner(location));
    }
    while rows.len() > completion.unchanged {
        counters.work(limits, location)?;
        rows.pop();
    }
    if completion.suffix_end > suffixes.len() || start > completion.suffix_end {
        return Err(owner(location));
    }
    for index in start..completion.suffix_end {
        counters.charge_work(2, limits, location)?;
        let (position, source_index) = suffixes.get(index).ok_or_else(|| owner(location))?;
        let source = prepared
            .sources
            .slice()
            .get(rows.len())
            .and_then(|resolution| resolution.rows())
            .ok_or_else(|| owner(location))?;
        let row = source.row(position).ok_or_else(|| owner(location))?;
        counters.work(limits, location)?;
        if row.source_index() != source_index {
            return Err(owner(location));
        }
        rows.push(row, computation, limits, counters, location)?;
    }
    if rows.len() != prepared.base.plan.patterns.len() {
        return Err(owner(location));
    }
    Ok(())
}

fn owner(location: ProgramSite) -> FormulaFailure {
    FormulaFailure::SupportRelation {
        error: zetesis_core::relation::Failure::Owner,
        location,
    }
}
