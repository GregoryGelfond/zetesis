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
use crate::formula_support::{Comparisons, Coverage, Traversal, projections, scalar_selection};
use crate::{FormulaFailure, FormulaLimits, ProgramSite};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Support,
    Emission,
    Producer,
}

/// A common borrowed input for either the isolated fixture or a real cursor.
/// All coordinates belong to the retained rule, plan and immutable epoch.
#[derive(Clone, Copy)]
pub(crate) struct Descriptor<'plan, 'source> {
    pub(super) rule: &'source RuleIr,
    pub(super) plan: &'plan order::Plan<'plan>,
    pub(super) relations: &'source relations::Relations<'source>,
    pub(super) rows: &'plan [usize],
}

impl Descriptor<'_, '_> {
    pub(super) fn len(self) -> usize {
        self.rows.len()
    }

    pub(super) fn location(self) -> ProgramSite {
        self.rule.location
    }
}

/// The original cursor owns the plan and any prepared probe. This owner retains
/// only its row-ID copy and header, not a second charge for borrowed plan data.
pub(crate) struct CursorRows<'cursor, 'source> {
    rule: &'source RuleIr,
    plan: &'cursor order::Plan<'cursor>,
    relations: &'source relations::Relations<'source>,
    rows: Buffer<usize>,
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
        if kind != Kind::Support {
            // Only this complete read-only body certificate makes the retained
            // family owner unreachable from worker traversal. It stays on the
            // original cursor and is never copied into a worker.
            let body = if kind == Kind::Producer {
                BodyKind::ProducerLeaves
            } else {
                BodyKind::PositiveAtoms
            };
            if !checked_rule(rule, computation, limits, counters, body)?
                || !initial_kind(
                    cursor,
                    Coverage::Selected,
                    limits,
                    counters,
                    rule.location,
                    true,
                )?
            {
                return Ok(None);
            }
        } else if !initial(cursor, Coverage::Selected, limits, counters, rule.location)?
            || !checked_rule(
                rule,
                computation,
                limits,
                counters,
                support_body(computation),
            )?
        {
            return Ok(None);
        }
        // A scan followed only by fully bound tests remains in the ordinary
        // witnessed consumer. This source-shape decision precedes row-copy or
        // worker admission; it never retries an already claimed task.
        if !generators::expands(&cursor.plan, limits, counters, rule.location)? {
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
        Ok(Some(Self {
            rule,
            plan: &cursor.plan,
            relations: cursor.support.relations(),
            rows,
            _lease: lease,
        }))
    }

    pub(crate) fn descriptor(&self) -> Descriptor<'_, 'source> {
        Descriptor {
            rule: self.rule,
            plan: self.plan,
            relations: self.relations,
            rows: self.rows.slice(),
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
    initial_kind(cursor, coverage, limits, counters, location, false)
}

fn initial_kind(
    cursor: &Join<'_, '_>,
    coverage: Coverage,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
    retain_unused_families: bool,
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
    require!(!cursor.generated);
    require!(cursor.head_slots.is_empty());
    require!(cursor.source_evidence == (coverage == Coverage::Complete));
    require!(!cursor.certified_total);
    require!(cursor.coverage == coverage);
    require!(cursor.domains.is_none());
    require!(cursor.expression_keys.is_none());
    require!(cursor.multiples.is_none());
    require!(cursor.range_selection.is_none());
    require!(cursor.checked_guard.is_none());
    require!(cursor.row_filter.is_none());
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
        cursor.projections,
        projections::Projections::Local(_)
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
