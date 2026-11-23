//! Exact witnessed source occurrences cross a task boundary; projection does not.
//!
//! A worker stages only the changed suffix already reported by the ordinary
//! witness cursor. Its receipt remains separate from success. After all workers
//! settle, the coordinator resolves those occurrences against their retained
//! immutable sources and uses the existing `RowHead` projection and canonical
//! writer. No assignment frame or typed source value is copied into transport.

mod binding;
mod heads;
mod replay;
pub(crate) use heads::HeadKeys;
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
    Continuation,
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

    pub(crate) fn for_continuations(
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
            Purpose::Continuation,
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

    pub(crate) fn continuations(&self) -> bool {
        self.base.continuations
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
    /// Fixed-width projected key words, when every staged row was keyed.
    /// Empty keys still have one logical identity per completion.
    keys: Option<Buffer<u32>>,
    key_width: usize,
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
        let header = size_of::<Self>()
            - size_of::<Buffer<Completion>>()
            - size_of::<PackedCoords>()
            - size_of::<Option<Buffer<u32>>>();
        lease.observe(header, location)?;
        computation.storage_observed(&lease, 0, header, limits, counters, location)?;
        Ok(Self {
            plan,
            completions: Buffer::new(computation, limits, counters, location)?,
            suffixes: PackedCoords::new(plan.coord_width, computation, limits, counters, location)?,
            keys: None,
            key_width: 0,
            _lease: lease,
        })
    }

    /// Start keying this empty batch's rows through `keyer`.
    fn key_heads(
        &mut self,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<(), FormulaFailure> {
        if self.completions.len() != 0 {
            return Err(owner(self.plan.location()));
        }
        self.keys = Some(Buffer::new(
            computation,
            limits,
            counters,
            self.plan.location(),
        )?);
        Ok(())
    }

    /// Stage `row`, then its head key if this batch is still keyed. A row that
    /// cannot be keyed returns the whole batch to the ordinary path.
    fn stage_keyed(
        &mut self,
        row: &WitnessRow<'_, '_>,
        keyer: Option<&mut heads::Keyer>,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<(), FormulaFailure> {
        self.stage(row, computation, limits, counters)?;
        let (Some(keyer), Some(_)) = (keyer, self.keys.as_ref()) else {
            return Ok(());
        };
        let location = self.plan.location();
        let key = keyer.key(
            self.plan,
            row,
            Context::new(computation, limits, counters, location),
        )?;
        if let Some(words) = key {
            if self.completions.len() == 1 {
                self.key_width = words.len();
            } else if self.key_width != words.len() {
                return Err(owner(location));
            }
            let keys = self.keys.as_mut().expect("checked keyed batch");
            keys.reserve(words.len(), computation, limits, counters, location)?;
            for word in words {
                keys.push(*word, computation, limits, counters, location)?;
            }
            return Ok(());
        }
        self.keys = None;
        Ok(())
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
        None,
        budget,
        Context::new(&mut computation, limits, counters, location),
    )
}

/// This private read sees heads already pending at task entry. It does not see
/// heads staged by any task until the coordinator commits the complete batch.
#[cfg(test)]
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
    execute_keyed(
        prepared,
        block,
        Writer { append, keys: None },
        limits,
        budget,
        counters,
    )
}

/// A worker's borrowed writer and the publication's keying policy.
#[derive(Clone, Copy)]
pub(crate) struct Writer<'a, 'source> {
    pub(crate) append: &'a crate::formula_support::SupportAppend<'source>,
    pub(crate) keys: Option<&'a HeadKeys>,
}

/// [`execute_borrowed`], staging each row's charged head-key words.
pub(crate) fn execute_keyed<'plan, 'source>(
    prepared: &'plan WitnessPlan<'plan, 'source>,
    block: Range<usize>,
    Writer { append, keys }: Writer<'_, 'source>,
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
        keys,
        budget,
        Context::new(&mut computation, limits, counters, location),
    )
}

fn replay_plan(
    prepared: &WitnessPlan<'_, '_>,
    computation: &Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<(super::prime::ResidualSkip, bool), FormulaFailure> {
    let skip = if prepared.prime() {
        residual_skip(prepared, computation, limits, counters, location)?
    } else {
        super::prime::ResidualSkip::none()
    };
    let owner_replay =
        prepared.prime() && super::prime::owner_replays(&prepared.base.rule.body, skip);
    Ok((skip, owner_replay))
}

fn residual_skip(
    prepared: &WitnessPlan<'_, '_>,
    computation: &Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<super::prime::ResidualSkip, FormulaFailure> {
    let classified = if prepared.base.continuations {
        super::prime::classify_continuation_body(
            &prepared.base.rule.body,
            prepared.base.rule.body_variables,
            computation,
            limits,
            counters,
            location,
        )?
    } else {
        super::prime::classify_body(
            &prepared.base.rule.body,
            prepared.base.rule.body_variables,
            computation,
            limits,
            counters,
            location,
        )?
    };
    classified
        .map(|class| class.skip)
        .ok_or_else(|| owner(location))
}

fn traverse<'plan, 'source, 'read>(
    prepared: &'plan WitnessPlan<'plan, 'source>,
    rows: &[usize],
    support: &Support<'read>,
    keys: Option<&HeadKeys>,
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
        let mut keyer = keys
            .map(|_| heads::Keyer::new(Context::new(computation, limits, counters, location)))
            .transpose()?;
        if keyer.is_some() {
            result.key_heads(computation, limits, counters)?;
        }
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
            join.skip_residuals(residual_skip(
                prepared,
                computation,
                limits,
                counters,
                location,
            )?);
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
            result.stage_keyed(&row, keyer.as_mut(), computation, limits, counters)?;
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
#[cfg(test)]
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
    execute_prefixed_keyed(
        prepared,
        prefixes,
        block,
        Writer { append, keys: None },
        limits,
        budget,
        counters,
    )
}

/// [`execute_prefixed`], staging each row's charged head-key words.
pub(crate) fn execute_prefixed_keyed<'plan, 'source>(
    prepared: &'plan WitnessPlan<'plan, 'source>,
    prefixes: &Prefixes<'_>,
    block: Range<usize>,
    Writer { append, keys }: Writer<'_, 'source>,
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
        keys,
        budget,
        Context::new(&mut computation, limits, counters, location),
    )
}

fn traverse_prefixed<'plan, 'source, 'read>(
    prepared: &'plan WitnessPlan<'plan, 'source>,
    prefixes: &Prefixes<'_>,
    block: Range<usize>,
    support: &Support<'read>,
    keys: Option<&HeadKeys>,
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
        let mut keyer = keys
            .map(|_| heads::Keyer::new(Context::new(computation, limits, counters, location)))
            .transpose()?;
        if keyer.is_some() {
            result.key_heads(computation, limits, counters)?;
        }
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
        let skip = residual_skip(prepared, computation, limits, counters, location)?;
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
                result.stage_keyed(&row, keyer.as_mut(), computation, limits, counters)?;
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

/// The rule a support publication belongs to, and its frozen head keys when
/// its workers interned them.
pub(super) struct Rule {
    pub(super) index: usize,
    pub(super) heads: Option<heads::Seen>,
}

/// Whether publication replays this plan's residual assignments rather than
/// projecting rows; such heads are not keyed.
pub(super) fn owner_replays(
    prepared: &WitnessPlan<'_, '_>,
    computation: &Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Result<bool, FormulaFailure> {
    replay_plan(prepared, computation, limits, counters, prepared.location())
        .map(|(_, owner_replay)| owner_replay)
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
    Rule {
        index: rule_index,
        mut heads,
    }: Rule,
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
    let (skip, owner_replay) = replay_plan(prepared, computation, limits, counters, location)?;
    let decided = decided(support, owner_replay, rule_index);
    let mut row_head = None;
    let mut assignment_head = None;
    let mut binding = None;
    let mut published = 0;
    if owner_replay && heads.is_some() {
        return Err(owner(location));
    }
    for batch in batches {
        let batch = core::borrow::Borrow::borrow(&batch);
        let mut rows = replay.batch(batch, limits, counters)?;
        while let Some(row) = rows.next(Context::new(&*computation, limits, counters, location))? {
            if repeated(heads.as_mut(), limits, counters, location)? {
                published = one_more(published, location)?;
                continue;
            }
            if owner_replay {
                let values = if let Some(existing) = binding.as_mut() {
                    existing
                } else {
                    binding = Some(RowBinding::new(
                        prepared,
                        &row,
                        Context::new(&*computation, limits, counters, location),
                    )?);
                    binding.as_mut().expect("just installed")
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
                    published = one_more(published, location)?;
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
                published = one_more(published, location)?;
                Ok::<_, FormulaFailure>(())
            })?;
        }
    }
    Ok(published)
}

/// The rule's decided residual literals, when publication replays them.
fn decided(support: &Support<'_>, owner_replay: bool, rule_index: usize) -> Vec<usize> {
    if owner_replay {
        support
            .certainty()
            .map(|certainty| certainty.decided(rule_index).collect())
            .unwrap_or_default()
    } else {
        Vec::new()
    }
}

fn one_more(published: usize, location: ProgramSite) -> Result<usize, FormulaFailure> {
    zetesis_algebra::Meter::check_count(published, 1, usize::MAX).map_err(|_| owner(location))
}

/// Whether a keyed row repeats a head an earlier row of this publication
/// already admitted and retained: admission would find that atom, and
/// retention is idempotent, so the row changes nothing.
fn repeated(
    heads: Option<&mut heads::Seen>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<bool, FormulaFailure> {
    let Some(heads) = heads else {
        return Ok(false);
    };
    Ok(heads
        .first(limits, counters, location)?
        .is_some_and(|first| !first))
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
