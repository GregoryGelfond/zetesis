//! Private qualification of disjoint leading-row blocks of the existing join.
//!
//! The coordinator fixes one immutable support epoch, delta variant and ordinary
//! join plan. Workers borrow that plan and disjoint contiguous portions of its
//! first selected row stream. Every suffix is traversed by `Join`; this module
//! owns no matching, join-order, arithmetic or source-admission implementation.
//! Publication remains ordered and single-owner. The coordinator owns execution
//! policy; the original binding-transport prototype remains a test fixture.

pub(crate) mod component_rows;
pub(crate) mod cursor_rows;
mod objective_rows;
#[cfg(test)]
mod tests;
pub(crate) mod witness_rows;
pub(crate) use objective_rows::try_numeric as parallel_numeric_rows;
pub(crate) mod coordinator;
pub(crate) mod prime;

#[cfg(test)]
use std::ops::Range;

#[cfg(test)]
use zetesis_core::atom_registry::TermLookup;
#[cfg(test)]
use zetesis_core::catalog::TermKey;

#[cfg(test)]
use super::heads;
use super::{
    Binding, Buffer, Computation, Context, Counters, GroundingWork, HeadIr, Ir, Join, JoinPlan,
    LiteralIr, Probe, StorageLease, Support, delta, order, relations,
};
#[cfg(test)]
use crate::expansion::Budget;
use crate::formula_ir::RuleIr;
use crate::{FormulaFailure, FormulaLimits, ProgramSite};
use themelios_program::program::DefaultNegation;

/// No local generator, arithmetic family, negative readiness, structural pattern
/// or newly constructed head value enters this first worker kernel. Constants
/// in a flat pattern are already canonical source values, including structured
/// constants; only their existing identities are read.
#[cfg(test)]
pub(super) fn eligible(rule: &RuleIr) -> bool {
    matches!(rule.head, HeadIr::Normal(Some(_)))
        && rule.body_variables == rule.variables
        && !rule.body.is_empty()
        && rule
            .body
            .iter()
            .all(|literal| matches!(literal, LiteralIr::Atom(DefaultNegation::None, _)))
}

/// Resolve every admitted flat pattern through the same source authority as
/// matching. Unsupported syntax declines; a malformed/foreign coordinate keeps
/// its ordinary typed source error rather than being disguised as a fallback.
#[cfg(test)]
fn checked_eligible(
    rule: &RuleIr,
    computation: &Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
) -> Result<bool, FormulaFailure> {
    checked_rule(rule, computation, limits, counters, BodyKind::PositiveAtoms)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BodyKind {
    PositiveAtoms,
    ProducerLeaves,
    /// Widened A′ admission: worker-pure comparisons, default negation,
    /// structural patterns, and integer binds. Trailing residuals (`not not`,
    /// term-minting binds, and any later comparisons) are skipped in the
    /// Join walk and replayed by the owner after restore.
    Prime,
    /// Like Prime, but trailing aggregate literals are residuals. Used only
    /// by formula-factor continuations, so support admission is unchanged.
    /// Family evaluation stays in the owner's existing emit path.
    Continuations,
}

fn checked_rule(
    rule: &RuleIr,
    computation: &Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    kind: BodyKind,
) -> Result<bool, FormulaFailure> {
    counters.work(limits, rule.location)?;
    if !matches!(rule.head, HeadIr::Normal(Some(_)))
        || rule.body_variables != rule.variables
        || rule.body.is_empty()
    {
        return Ok(false);
    }
    let HeadIr::Normal(Some(head)) = rule.head else {
        unreachable!()
    };
    if !checked_body_kind(
        &rule.body,
        rule.body_variables,
        computation,
        limits,
        counters,
        rule.location,
        kind,
    )? {
        return Ok(false);
    }
    checked_pattern(
        head,
        rule.body_variables,
        computation,
        limits,
        counters,
        rule.location,
    )
}

/// One shared positive-flat body admission; source-specific callers establish
/// the nonempty body and unchanged scope before this traversal.
fn checked_body(
    body: &[LiteralIr],
    variables: usize,
    computation: &Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<bool, FormulaFailure> {
    checked_body_kind(
        body,
        variables,
        computation,
        limits,
        counters,
        location,
        BodyKind::PositiveAtoms,
    )
}

fn checked_body_kind(
    body: &[LiteralIr],
    variables: usize,
    computation: &Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
    kind: BodyKind,
) -> Result<bool, FormulaFailure> {
    if kind == BodyKind::Prime {
        return checked_prime_body(body, variables, computation, limits, counters, location);
    }
    if kind == BodyKind::Continuations {
        return checked_continuation_body(body, variables, computation, limits, counters, location);
    }
    for literal in body {
        counters.work(limits, location)?;
        match literal {
            LiteralIr::Atom(DefaultNegation::None, _) => {}
            LiteralIr::Compare(comparison) if kind == BodyKind::ProducerLeaves => {
                let (expressions, _) = comparison.operands();
                if super::evaluation::comparison_leaves_with(expressions, || {
                    counters.work(limits, location)
                })?
                .is_none()
                {
                    return Ok(false);
                }
            }
            _ => return Ok(false),
        }
    }
    for literal in body {
        if kind == BodyKind::ProducerLeaves {
            // This producer-only scan also visits comparisons without a
            // pattern read; charge every actual second-pass literal visit.
            counters.work(limits, location)?;
        }
        // Producer comparisons retain their original reached Join evaluation.
        // Only source patterns are authenticated by this existing second pass.
        if let LiteralIr::Atom(_, pattern) = literal
            && !checked_pattern(*pattern, variables, computation, limits, counters, location)?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn checked_prime_body(
    body: &[LiteralIr],
    variables: usize,
    computation: &Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<bool, FormulaFailure> {
    if prime::classify_body(body, variables, computation, limits, counters, location)?.is_none() {
        return Ok(false);
    }
    checked_residual_patterns(body, variables, computation, limits, counters, location)
}

fn checked_continuation_body(
    body: &[LiteralIr],
    variables: usize,
    computation: &Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<bool, FormulaFailure> {
    if prime::classify_continuation_body(body, variables, computation, limits, counters, location)?
        .is_none()
    {
        return Ok(false);
    }
    checked_residual_patterns(body, variables, computation, limits, counters, location)
}

/// Trailing residuals are admitted: classify already declined a generating
/// atom at or after the first residual.
fn checked_residual_patterns(
    body: &[LiteralIr],
    variables: usize,
    computation: &Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<bool, FormulaFailure> {
    for literal in body {
        counters.work(limits, location)?;
        let pattern = match literal {
            LiteralIr::Atom(_, pattern) => Some(*pattern),
            LiteralIr::PatternAtom(pattern) => Some(pattern.atom),
            _ => None,
        };
        if let Some(pattern) = pattern
            && !checked_pattern(pattern, variables, computation, limits, counters, location)?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn checked_pattern(
    pattern: crate::formula_support::components::Pattern,
    variables: usize,
    computation: &Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<bool, FormulaFailure> {
    counters.work(limits, location)?;
    let pattern = computation.static_pattern(pattern, limits, counters, location)?;
    let terms = pattern.terms();
    for column in 0..terms.len() {
        counters.work(limits, location)?;
        if let zetesis_core::TemplateTerm::Variable(slot) =
            terms.get(column).expect("checked column")
            && slot >= variables
        {
            return Ok(false);
        }
    }
    Ok(true)
}

/// One exact ordinary plan and its first occurrence's selected source-row order.
/// The retained descriptors borrow their original immutable source authority.
#[cfg(test)]
pub(super) struct RowPlan<'plan, 'source> {
    rule: &'source RuleIr,
    plan: order::Plan<'plan>,
    relations: &'source relations::Relations<'source>,
    rows: Buffer<usize>,
    _lease: StorageLease,
}

#[cfg(test)]
impl<'plan, 'source> RowPlan<'plan, 'source> {
    pub(super) fn prepare(
        rule: &'source RuleIr,
        variant: delta::Variant,
        support: &'plan Support<'source>,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        budget: &mut Budget,
        counters: &mut Counters,
    ) -> Result<Option<Self>, FormulaFailure>
    where
        'source: 'plan,
    {
        counters.work(limits, rule.location)?;
        if !checked_eligible(rule, computation, limits, counters)? {
            return Ok(None);
        }
        let mut join = Join::variant_rule(
            Ir::Source(rule),
            variant,
            support,
            None,
            budget,
            Context::new(computation, limits, counters, rule.location),
        )?;
        let Some(&first) = join.plan.patterns.first() else {
            return Ok(None);
        };
        join.prepare_probe(
            first,
            Context::new(computation, limits, counters, rule.location),
        )?;
        let rows = selected_rows(
            join.probes[0].as_ref().expect("prepared first probe"),
            computation,
            limits,
            counters,
            rule.location,
        )?;
        let (plan, mut lease) = take_plan(join);
        let retained = usize::try_from(plan.retained_bytes())
            .ok()
            .and_then(|bytes| bytes.checked_add(size_of::<Self>() - size_of::<Buffer<usize>>()))
            .ok_or_else(|| FormulaFailure::SupportRelation {
                error: zetesis_core::relation::Failure::Overflow,
                location: rule.location,
            })?;
        // The former cursor's other scratch has dropped. The row-ID Buffer
        // keeps its independent capacity lease; retain only this plan owner.
        lease.observe(retained, rule.location)?;
        computation.storage_observed(&lease, 0, retained, limits, counters, rule.location)?;
        Ok(Some(Self {
            rule,
            plan,
            relations: support.relations(),
            rows,
            _lease: lease,
        }))
    }

    fn descriptor(&self) -> cursor_rows::Descriptor<'_, 'source> {
        cursor_rows::Descriptor {
            rule: self.rule,
            plan: &self.plan,
            relations: self.relations,
            rows: self.rows.slice(),
            prime: false,
            continuations: false,
        }
    }

    pub(super) fn len(&self) -> usize {
        self.rows.len()
    }

    pub(super) fn location(&self) -> ProgramSite {
        self.rule.location
    }
}

/// The existing probe owns selection. This separate position observes its
/// exact order without advancing the original join's traversal state.
fn selected_rows(
    probe: &Probe<'_, '_>,
    computation: &Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<Buffer<usize>, FormulaFailure> {
    let mut rows = Buffer::new(computation, limits, counters, location)?;
    let mut position = 0;
    loop {
        // Indexed probes leave the traversal charge to their caller.
        counters.work(limits, location)?;
        let Some(row) = probe.next(&mut position, limits, counters, location)? else {
            break;
        };
        rows.push(row, computation, limits, counters, location)?;
    }
    Ok(rows)
}

/// Consuming the cursor closes the lifetime of every unreturned scratch
/// owner before its former aggregate lease is reduced by the coordinator.
#[cfg(test)]
fn take_plan<'plan>(join: Join<'plan, '_>) -> (order::Plan<'plan>, StorageLease) {
    let Join { plan, lease, .. } = join;
    let JoinPlan::Owned(plan) = plan else {
        unreachable!("variant_rule constructs one owned ordinary plan");
    };
    (plan, lease)
}

/// A bounded result retains canonical IDs only. Its row count also represents
/// successful zero-variable bindings; no caller may infer it from cell length.
/// On any worker error the incomplete result is dropped and never published.
#[cfg(test)]
pub(super) struct RowBatch {
    values: Buffer<TermKey>,
    rows: usize,
    width: usize,
    _lease: StorageLease,
}

#[cfg(test)]
impl RowBatch {
    pub(super) fn len(&self) -> usize {
        self.rows
    }

    /// Commit one successful block through the existing normal-head owner.
    /// The coordinator calls blocks in their leading-row order and performs no
    /// next-round publication while a task still borrows the immutable epoch.
    pub(super) fn publish(
        &self,
        rule: &RuleIr,
        computation: &mut Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
    ) -> Result<(), FormulaFailure> {
        let HeadIr::Normal(Some(pattern)) = rule.head else {
            unreachable!("the admitted task has one normal head");
        };
        assert_eq!(self.width, rule.variables);
        let mut binding = Binding::new(computation, limits, counters, rule.location)?;
        binding.extend_scope(self.width, computation, limits, counters, rule.location)?;
        let mut head = None;
        for row in 0..self.rows {
            for variable in 0..self.width {
                let key = &self.values.slice()[row * self.width + variable];
                binding.set(variable, key, limits, counters, rule.location)?;
            }
            if head.is_none() {
                head = Some(heads::Head::new(
                    pattern,
                    computation,
                    limits,
                    counters,
                    rule.location,
                )?);
            }
            head.as_ref().expect("prepared head").derive(
                &binding,
                computation,
                limits,
                counters,
                rule.location,
            )?;
        }
        Ok(())
    }
}

/// Run one disjoint block using the existing cursor and matching. The caller
/// supplies a distinct lookup capability, local control/accounting and an
/// already-partitioned allowance. Neither `Support` nor `Counters` crosses a
/// thread boundary; the immutable relations and ordinary plan are shared.
#[cfg(test)]
pub(super) fn execute<'plan, 'source>(
    prepared: &RowPlan<'plan, 'source>,
    block: Range<usize>,
    relations: &'source relations::Relations<'source>,
    lookup: TermLookup<'source>,
    limits: &FormulaLimits,
    budget: &mut Budget,
    counters: &mut Counters,
) -> Result<RowBatch, FormulaFailure>
where
    'source: 'plan,
{
    let location = prepared.rule.location;
    let rows = prepared
        .rows
        .slice()
        .get(block)
        .ok_or(FormulaFailure::SupportRelation {
            error: zetesis_core::relation::Failure::Owner,
            location,
        })?;
    let support = Support::indexed(relations, limits, counters, location)?;
    let mut computation = Computation::frozen(lookup, &support);
    let mut result_lease = computation.lease();
    let result_header = size_of::<RowBatch>() - size_of::<Buffer<TermKey>>();
    result_lease.observe(result_header, location)?;
    computation.storage_observed(&result_lease, 0, result_header, limits, counters, location)?;
    let mut result = RowBatch {
        values: Buffer::new(&computation, limits, counters, location)?,
        rows: 0,
        width: prepared.rule.variables,
        _lease: result_lease,
    };
    if rows.is_empty() {
        return Ok(result);
    }
    let prefix = Binding::new(&computation, limits, counters, location)?;
    let mut join = Join::with_plan(
        &prepared.rule.body,
        &prefix,
        prepared.rule.variables,
        &support,
        JoinPlan::Borrowed(&prepared.plan),
        None,
        Context::new(&computation, limits, counters, location),
    )?;
    join.configure_rule(Ir::Source(prepared.rule), limits, counters)?;
    let first = join.plan.patterns[0];
    join.resolve_probe_source(first, GroundingWork::new(limits, counters, location))?;
    join.probes[0] = Some(Probe::Indexed(delta::Rows::Posting(rows)));
    while let Some(binding) =
        join.next_support(None, &mut computation, limits, budget, counters, location)?
    {
        result
            .values
            .reserve(result.width, &computation, limits, counters, location)?;
        for variable in 0..result.width {
            counters.work(limits, location)?;
            let key = binding.key(variable, location)?;
            result
                .values
                .push(key, &computation, limits, counters, location)?;
        }
        counters.work(limits, location)?;
        result.rows =
            result
                .rows
                .checked_add(1)
                .ok_or_else(|| FormulaFailure::SupportRelation {
                    error: zetesis_core::relation::Failure::Overflow,
                    location,
                })?;
    }
    Ok(result)
}
