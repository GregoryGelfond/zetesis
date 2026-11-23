//! A lending boundary around an existing, not-yet-started source cursor.
//!
//! The cursor keeps its plan and initial preparation owners. Only the first
//! probe's selected row identities are copied, under the ordinary buffer lease.
//! Observing that probe uses a separate position and never consumes the cursor.

mod emission;
mod generators;
mod producer;
#[cfg(test)]
mod tests;

use super::{
    BodyKind, Buffer, Computation, Context, Counters, Join, RuleIr, StorageLease, checked_rule,
    order, relations, selected_rows,
};
use crate::formula_support::{
    Comparisons, Coverage, Traversal, expression_memos, scalar_selection,
};
use crate::{FormulaFailure, FormulaLimits, ProgramSite};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Support,
    Emission,
    Producer,
    Continuation,
}

/// A common borrowed input for either the isolated fixture or a real cursor.
/// All coordinates belong to the retained rule, plan and immutable epoch.
#[derive(Clone, Copy)]
pub(crate) struct Descriptor<'plan, 'source> {
    pub(super) rule: &'source RuleIr,
    pub(super) plan: &'plan order::Plan<'plan>,
    pub(super) relations: &'source relations::Relations<'source>,
    pub(super) rows: &'plan [usize],
    /// True when this stream was admitted by A′ (`BodyKind::Prime`) and should
    /// use [`super::prime::PRIME_GROUPS`] rather than `TASKS`.
    pub(super) prime: bool,
    /// True when this stream is a formula-factor continuation. Trailing
    /// aggregates are residuals; workers use [`super::prime::classify_continuation_body`].
    pub(super) continuations: bool,
}

impl Descriptor<'_, '_> {
    pub(super) fn len(self) -> usize {
        self.rows.len()
    }

    pub(super) fn location(self) -> ProgramSite {
        self.rule.location
    }

    /// Existing positive-flat streams keep `TASKS` groups. A′ streams use the
    /// data-defined `PRIME_GROUPS` cap. Either count is independent of `--threads`.
    pub(super) fn groups(self) -> usize {
        if self.prime || self.continuations {
            super::prime::PRIME_GROUPS
        } else {
            crate::formula_support::parallel_executor::TASKS
        }
    }
}

/// The original cursor owns the plan and any prepared probe. This owner retains
/// only its row-ID copy and header, not a second charge for borrowed plan data.
pub(crate) struct CursorRows<'cursor, 'source> {
    rule: &'source RuleIr,
    plan: &'cursor order::Plan<'cursor>,
    relations: &'source relations::Relations<'source>,
    rows: Buffer<usize>,
    prime: bool,
    continuations: bool,
    _lease: StorageLease,
}

impl<'cursor, 'source> CursorRows<'cursor, 'source> {
    pub(super) fn prepare<'plan>(
        rule: &'source RuleIr,
        cursor: &'cursor mut Join<'plan, 'source>,
        computation: &mut Computation<'_, 'source>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<Option<Self>, FormulaFailure>
    where
        'source: 'plan,
        'plan: 'cursor,
    {
        Self::prepare_kind(rule, cursor, computation, limits, counters, Kind::Support)
    }

    /// Admit a formula-factor continuation's base join for A′ staging.
    /// Generated cursors with trailing aggregates are expected; a short
    /// non-expanding first probe still splits when it has at least two rows.
    pub(crate) fn prepare_continuation<'plan>(
        rule: &'source RuleIr,
        cursor: &'cursor mut Join<'plan, 'source>,
        computation: &mut Computation<'_, 'source>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<Option<Self>, FormulaFailure>
    where
        'source: 'plan,
        'plan: 'cursor,
    {
        Self::prepare_kind(
            rule,
            cursor,
            computation,
            limits,
            counters,
            Kind::Continuation,
        )
    }

    fn prepare_kind<'plan>(
        rule: &'source RuleIr,
        cursor: &'cursor mut Join<'plan, 'source>,
        computation: &mut Computation<'_, 'source>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        kind: Kind,
    ) -> Result<Option<Self>, FormulaFailure>
    where
        'source: 'plan,
        'plan: 'cursor,
    {
        counters.work(limits, rule.location)?;
        if !std::ptr::eq(cursor.literals.get(), rule.body.as_slice()) {
            return Err(owner(rule.location));
        }
        counters.work(limits, rule.location)?;
        if !std::ptr::eq(cursor.support, computation.query_support()) {
            return Err(owner(rule.location));
        }
        let Some((prime, continuations)) =
            admit_prepared_kind(kind, rule, cursor, computation, limits, counters)?
        else {
            return Ok(None);
        };
        // A scan followed only by fully bound tests remains in the ordinary
        // witnessed consumer. This source-shape decision precedes row-copy or
        // worker admission; it never retries an already claimed task.
        let expanded = generators::expands(&cursor.plan, limits, counters, rule.location)?;
        // Continuation element/base joins may be a single large scan. The
        // first-row split is still useful; a one-row stream stays serial.
        if !expanded && kind != Kind::Continuation {
            return Ok(None);
        }
        let Some(&first) = cursor.plan.patterns.first() else {
            return Ok(None);
        };
        let mut lease = computation.lease();
        let header = size_of::<Self>() - size_of::<Buffer<usize>>();
        lease.observe(header, rule.location)?;
        computation.storage_observed(&lease, 0, header, limits, counters, rule.location)?;
        cursor.prepare_probe(
            first,
            Context::new(computation, limits, counters, rule.location),
        )?;
        let rows = selected_rows(
            cursor.probes[0].as_ref().expect("prepared original probe"),
            computation,
            limits,
            counters,
            rule.location,
        )?;
        if kind == Kind::Continuation && !expanded && rows.len() < 2 {
            return Ok(None);
        }
        Ok(Some(Self {
            rule,
            plan: &cursor.plan,
            relations: cursor.support.relations(),
            rows,
            prime,
            continuations,
            _lease: lease,
        }))
    }

    pub(crate) fn descriptor(&self) -> Descriptor<'_, 'source> {
        Descriptor {
            rule: self.rule,
            plan: self.plan,
            relations: self.relations,
            rows: self.rows.slice(),
            prime: self.prime,
            continuations: self.continuations,
        }
    }
}

/// A support producer's workers evaluate total comparison leaves in their own
/// traversal, as a source-activity producer's do; table joins keep the
/// positive-atom body their row blocks were admitted for.
fn support_body(computation: &Computation<'_, '_>) -> BodyKind {
    if computation.query_support().strategy() == crate::JoinStrategy::Table {
        BodyKind::PositiveAtoms
    } else {
        BodyKind::ProducerLeaves
    }
}

/// Each gate is charged before its read. Unsupported retained preparation is
/// declined, never discarded or rebuilt without its original authority.
pub(super) fn initial(
    cursor: &Join<'_, '_>,
    coverage: Coverage,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<bool, FormulaFailure> {
    initial_kind(cursor, coverage, limits, counters, location, false, false)
}

fn initial_kind(
    cursor: &Join<'_, '_>,
    coverage: Coverage,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
    retain_unused_families: bool,
    allow_generated: bool,
) -> Result<bool, FormulaFailure> {
    macro_rules! require {
        ($condition:expr) => {
            counters.work(limits, location)?;
            if !$condition {
                return Ok(false);
            }
        };
    }
    require!(matches!(cursor.traversal, Traversal::Searching));
    require!(cursor.depth == 0);
    require!(allow_generated || !cursor.generated);
    require!(cursor.head_slots.is_empty());
    require!(cursor.source_evidence == (coverage == Coverage::Complete));
    require!(!cursor.certified_total);
    require!(cursor.coverage == coverage);
    require!(cursor.domains.is_none());
    require!(cursor.expression_keys.is_none());
    require!(cursor.multiples.is_none());
    require!(cursor.range_selection.is_none());
    require!(cursor.checked_guard.is_none());
    require!(cursor.bind_restriction.is_none());
    require!(cursor.anchor.is_none());
    require!(cursor.positive_prefix.is_none());
    require!(cursor.pattern_rows.is_none());
    require!(retain_unused_families || cursor.families.is_none());
    require!(cursor.witnesses.is_none());
    require!(cursor.pending.is_none());
    require!(cursor.pending_head.is_none());
    require!(cursor.family.zero.is_none());
    require!(cursor.failure.is_none());
    require!(matches!(cursor.comparisons, Comparisons::Deferred));
    require!(matches!(cursor.pending_comparisons, Comparisons::Deferred));
    require!(!matches!(
        cursor.scalar_selection,
        scalar_selection::Preparation::Ready { .. }
    ));
    require!(matches!(
        cursor.expression_memos,
        expression_memos::ExpressionMemos::Local(_)
    ));
    for position in &cursor.positions {
        require!(*position == 0);
    }
    for variable in 0..cursor.values.len() {
        require!(!cursor.values.is_bound(variable, location)?);
    }
    Ok(true)
}

fn owner(location: ProgramSite) -> FormulaFailure {
    FormulaFailure::SupportRelation {
        error: zetesis_core::relation::Failure::Owner,
        location,
    }
}

/// Kind-specific cursor admission shared by [`CursorRows::prepare_kind`].
/// Returns `Ok(None)` when this kind declines the existing serial path.
fn admit_prepared_kind<'source>(
    kind: Kind,
    rule: &'source RuleIr,
    cursor: &mut Join<'_, 'source>,
    computation: &mut Computation<'_, 'source>,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Result<Option<(bool, bool)>, FormulaFailure> {
    match kind {
        Kind::Continuation => admit_continuation(rule, cursor, computation, limits, counters),
        Kind::Emission => admit_emission(rule, cursor, computation, limits, counters),
        Kind::Producer => admit_producer(rule, cursor, computation, limits, counters),
        Kind::Support => admit_support(rule, cursor, computation, limits, counters),
    }
}

fn admit_continuation<'source>(
    rule: &'source RuleIr,
    cursor: &mut Join<'_, 'source>,
    computation: &mut Computation<'_, 'source>,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Result<Option<(bool, bool)>, FormulaFailure> {
    let coverage = cursor.coverage;
    if !checked_rule(rule, computation, limits, counters, BodyKind::Continuations)?
        || !initial_kind(
            cursor,
            coverage,
            limits,
            counters,
            rule.location,
            true,
            true,
        )?
    {
        return Ok(None);
    }
    Ok(Some((true, true)))
}

fn admit_emission<'source>(
    rule: &'source RuleIr,
    cursor: &mut Join<'_, 'source>,
    computation: &mut Computation<'_, 'source>,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Result<Option<(bool, bool)>, FormulaFailure> {
    // Existing positive-atom emission keeps its original charge
    // order and still declines comparison bodies: `witnesses::eligible`
    // and `emit_row` remain all-atom, so Prime is only a fallback for
    // generated cursors that the 14-task `!generated` gate rejects.
    if !cursor.generated {
        if !checked_rule(rule, computation, limits, counters, BodyKind::PositiveAtoms)?
            || !initial_kind(
                cursor,
                Coverage::Selected,
                limits,
                counters,
                rule.location,
                true,
                false,
            )?
        {
            return Ok(None);
        }
        return Ok(Some((false, false)));
    }
    if !checked_rule(rule, computation, limits, counters, BodyKind::Prime)?
        || !initial_kind(
            cursor,
            Coverage::Selected,
            limits,
            counters,
            rule.location,
            true,
            true,
        )?
    {
        return Ok(None);
    }
    Ok(Some((true, false)))
}

fn admit_producer<'source>(
    rule: &'source RuleIr,
    cursor: &mut Join<'_, 'source>,
    computation: &mut Computation<'_, 'source>,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Result<Option<(bool, bool)>, FormulaFailure> {
    // Only this complete read-only body certificate makes the retained
    // family owner unreachable from worker traversal. It stays on the
    // original cursor and is never copied into a worker.
    if !checked_rule(
        rule,
        computation,
        limits,
        counters,
        BodyKind::ProducerLeaves,
    )? || !initial_kind(
        cursor,
        Coverage::Selected,
        limits,
        counters,
        rule.location,
        true,
        false,
    )? {
        return Ok(None);
    }
    Ok(Some((false, false)))
}

fn admit_support<'source>(
    rule: &'source RuleIr,
    cursor: &mut Join<'_, 'source>,
    computation: &mut Computation<'_, 'source>,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Result<Option<(bool, bool)>, FormulaFailure> {
    if !cursor.generated {
        // The 14-task path still requires a not-yet-generated cursor, and
        // its work charges stay exactly as they were.
        if !initial(cursor, Coverage::Selected, limits, counters, rule.location)? {
            return Ok(None);
        } else if !checked_rule(
            rule,
            computation,
            limits,
            counters,
            support_body(computation),
        )? {
            // The existing positive-atom / producer-leaf path declined.
            // Try A′ admission so comparison, default-negation and
            // integer-bind bodies can take the parallel path without
            // changing already-admitted rules.
            if !checked_rule(rule, computation, limits, counters, BodyKind::Prime)? {
                return Ok(None);
            }
            return Ok(Some((true, false)));
        }
        return Ok(Some((false, false)));
    }
    if !initial_kind(
        cursor,
        Coverage::Selected,
        limits,
        counters,
        rule.location,
        false,
        true,
    )? || !checked_rule(rule, computation, limits, counters, BodyKind::Prime)?
    {
        // Bind and other body generators set `generated`. Only A′ admits
        // those cursors; the existing 14-task gates are unchanged.
        return Ok(None);
    }
    Ok(Some((true, false)))
}
