//! Adaptive prefix depth for A′: smallest `d ≤ 3` with at least `PRIME_GROUPS`
//! prefix tuples, then the same data-defined group split as the d = 1 path.
//!
//! Support's 14-task path never enters this module. Formula emission may call
//! [`adapt`] so a short first probe can deepen; when `d = 1`, that call returns
//! the existing first-row list with no extra walk. A Prime rule whose first
//! selected probe already has at least [`PRIME_GROUPS`] rows, or that has only
//! one pattern, keeps that first-row list and incurs no extra walk or storage.
//! Prefix enumeration is charged work under the same limits as the serial join
//! prefix; t1 and t14 see the same geometry and the same charges. A resource
//! refusal while deepening falls back to the longest successful prefix list
//! (or to d = 1) rather than aborting the rule.

use super::{PRIME_GROUPS, classify_body};
use crate::expansion::Budget;
use crate::formula_ir::RuleIr;
use crate::formula_support::accounting::StopPrecedence;
use crate::formula_support::{
    Binding, Buffer, Computation, Context, Counters, GroundingWork, Ir, Join, JoinPlan, Support,
    order,
};
use crate::{FormulaFailure, FormulaLimits, ProgramSite};

/// Largest join depth considered when choosing a prefix split.
pub(crate) const MAX_PREFIX_DEPTH: usize = 3;

/// One accepted prefix of at most [`MAX_PREFIX_DEPTH`] source-row identities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PrefixIds {
    depth: u8,
    ids: [u32; MAX_PREFIX_DEPTH],
}

impl PrefixIds {
    pub(crate) fn packed(depth: u8, ids: [u32; MAX_PREFIX_DEPTH]) -> Self {
        debug_assert!(depth >= 1 && usize::from(depth) <= MAX_PREFIX_DEPTH);
        Self { depth, ids }
    }

    pub(crate) fn depth(self) -> usize {
        usize::from(self.depth)
    }

    pub(crate) fn get(self, index: usize) -> Option<usize> {
        (index < self.depth()).then_some(self.ids[index] as usize)
    }
}

/// The coordinator's prefix list: either the existing first-probe row stream
/// (`d = 1`) or a packed tape of deeper tuples.
pub(crate) enum Prefixes<'plan> {
    /// First selected source-row identities, in probe order. The 14-task path
    /// and Prime rules that already have enough first rows use this variant.
    First(&'plan [usize]),
    /// Flattened source-row identities, stride [`PrefixTape::depth`]. Packed
    /// as `u32` because A′ already declines indexes that do not fit `u32`.
    Deep(PrefixTape),
}

/// Packed prefix-tuple tape. `ids.len()` is a multiple of `depth`.
pub(crate) struct PrefixTape {
    depth: u8,
    ids: Buffer<u32>,
}

impl PrefixTape {
    fn new(
        depth: usize,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<Self, FormulaFailure> {
        if depth < 2 || depth > MAX_PREFIX_DEPTH {
            return Err(owner(location));
        }
        Ok(Self {
            depth: depth as u8,
            ids: Buffer::new(computation, limits, counters, location)?,
        })
    }

    fn push(
        &mut self,
        prefix: PrefixIds,
        computation: &Computation<'_, '_>,
        limits: &FormulaLimits,
        counters: &mut Counters,
        location: ProgramSite,
    ) -> Result<(), FormulaFailure> {
        if prefix.depth() != usize::from(self.depth) {
            return Err(owner(location));
        }
        for index in 0..prefix.depth() {
            self.ids.push(
                prefix.get(index).expect("depth checked") as u32,
                computation,
                limits,
                counters,
                location,
            )?;
        }
        Ok(())
    }

    pub(crate) fn depth(&self) -> usize {
        usize::from(self.depth)
    }

    pub(crate) fn len(&self) -> usize {
        self.ids.len() / self.depth()
    }

    pub(crate) fn tuple(&self, index: usize) -> Option<PrefixIds> {
        let depth = self.depth();
        let start = index.checked_mul(depth)?;
        let end = start.checked_add(depth)?;
        let slice = self.ids.slice().get(start..end)?;
        let mut packed = [0u32; MAX_PREFIX_DEPTH];
        packed[..depth].copy_from_slice(slice);
        Some(PrefixIds::packed(self.depth, packed))
    }
}

impl<'plan> Prefixes<'plan> {
    /// The existing first-probe list. No extra storage or walk.
    pub(crate) fn first(rows: &'plan [usize]) -> Self {
        Self::First(rows)
    }

    pub(crate) fn depth(&self) -> usize {
        match self {
            Self::First(_) => 1,
            Self::Deep(tape) => tape.depth(),
        }
    }

    /// Number of prefix tuples (or first-row identities when `d = 1`).
    pub(crate) fn len(&self) -> usize {
        match self {
            Self::First(rows) => rows.len(),
            Self::Deep(tape) => tape.len(),
        }
    }

    pub(crate) fn tuple(&self, index: usize) -> Option<PrefixIds> {
        match self {
            Self::First(rows) => {
                let id = u32::try_from(*rows.get(index)?).ok()?;
                Some(PrefixIds::packed(1, [id, 0, 0]))
            }
            Self::Deep(tape) => tape.tuple(index),
        }
    }
}

/// Choose the smallest `d ≤ 3` with at least [`PRIME_GROUPS`] tuples.
///
/// `first` is the d = 1 count; `deeper[i]` is the count at depth `i + 2`.
/// The last considered depth is `pattern_count.min(MAX_PREFIX_DEPTH)`. A
/// short list still uses that deepest count: grouping then yields one tuple
/// per group, the same rule as today's first-row split.
pub(super) fn chosen_depth(first: usize, deeper: &[usize], pattern_count: usize) -> usize {
    let cap = pattern_count.min(MAX_PREFIX_DEPTH).max(1);
    if first >= PRIME_GROUPS || cap == 1 {
        return 1;
    }
    for (offset, &count) in deeper.iter().enumerate() {
        let depth = offset + 2;
        if depth > cap {
            break;
        }
        if count >= PRIME_GROUPS {
            return depth;
        }
    }
    1
}

/// Build the A′ prefix list. Support calls this only for Prime-admitted rules.
/// Emission may also call it for already-admitted positive-flat streams.
///
/// Resource refusals while enumerating a deeper list keep the longest
/// successful list so the rule can still proceed, or fall back to d = 1.
pub(in crate::formula_support) fn adapt<'plan, 'source>(
    rule: &'source RuleIr,
    join_plan: &'plan order::Plan<'plan>,
    first: &'plan [usize],
    support: &'plan Support<'source>,
    budget: &mut Budget,
    context: Context<'_, &mut Computation<'_, 'source>>,
) -> Result<Prefixes<'plan>, FormulaFailure>
where
    'source: 'plan,
{
    adapt_with(rule, join_plan, first, support, budget, false, context)
}

pub(in crate::formula_support) fn adapt_with<'plan, 'source>(
    rule: &'source RuleIr,
    join_plan: &'plan order::Plan<'plan>,
    first: &'plan [usize],
    support: &'plan Support<'source>,
    budget: &mut Budget,
    continuations: bool,
    context: Context<'_, &mut Computation<'_, 'source>>,
) -> Result<Prefixes<'plan>, FormulaFailure>
where
    'source: 'plan,
{
    let patterns = join_plan.patterns.len();
    if first.len() >= PRIME_GROUPS || patterns <= 1 {
        return Ok(Prefixes::first(first));
    }
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location,
            },
    } = context;
    match counters.without_observation(|counters| {
        enumerate(
            rule,
            join_plan,
            first,
            support,
            2,
            budget,
            computation,
            limits,
            counters,
            location,
            continuations,
        )
    }) {
        Ok(at_two) if chosen_depth(first.len(), &[at_two.len()], patterns) == 2 => {
            Ok(Prefixes::Deep(at_two))
        }
        Ok(_) if patterns < 3 => Ok(Prefixes::first(first)),
        Ok(at_two) => match counters.without_observation(|counters| {
            enumerate(
                rule,
                join_plan,
                first,
                support,
                3,
                budget,
                computation,
                limits,
                counters,
                location,
                continuations,
            )
        }) {
            Ok(at_three)
                if chosen_depth(first.len(), &[at_two.len(), at_three.len()], patterns) == 3 =>
            {
                Ok(Prefixes::Deep(at_three))
            }
            Ok(_) => Ok(Prefixes::first(first)),
            Err(error) if error.is_resource_refusal() => Ok(Prefixes::first(first)),
            Err(error) => Err(error),
        },
        Err(error) if error.is_resource_refusal() => Ok(Prefixes::first(first)),
        Err(error) => Err(error),
    }
}

fn enumerate<'plan, 'source>(
    rule: &'source RuleIr,
    join_plan: &'plan order::Plan<'plan>,
    first: &'plan [usize],
    support: &'plan Support<'source>,
    depth: usize,
    budget: &mut Budget,
    computation: &mut Computation<'_, 'source>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
    continuations: bool,
) -> Result<PrefixTape, FormulaFailure>
where
    'source: 'plan,
{
    let mut tape = PrefixTape::new(depth, computation, limits, counters, location)?;
    let prefix = Binding::new(computation, limits, counters, location)?;
    let mut join = Join::with_plan(
        &rule.body,
        &prefix,
        rule.variables,
        support,
        JoinPlan::Borrowed(join_plan),
        None,
        Context::new(computation, limits, counters, location),
    )?;
    join.configure_rule(Ir::Source(rule), limits, counters)?;
    let classified = if continuations {
        super::classify_continuation_body(
            &rule.body,
            rule.body_variables,
            computation,
            limits,
            counters,
            location,
        )?
    } else {
        classify_body(
            &rule.body,
            rule.body_variables,
            computation,
            limits,
            counters,
            location,
        )?
    };
    let skip = match classified {
        Some(class) => class.skip,
        None => return Err(owner(location)),
    };
    join.skip_residuals(skip);
    join.install_first_posting(first, GroundingWork::new(limits, counters, location))?;
    while let Some(ids) = join.next_prefix(
        depth,
        budget,
        Context::new(computation, limits, counters, location),
    )? {
        tape.push(ids, computation, limits, counters, location)?;
    }
    Ok(tape)
}

fn owner(location: ProgramSite) -> FormulaFailure {
    FormulaFailure::SupportRelation {
        error: zetesis_core::relation::Failure::Owner,
        location,
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_PREFIX_DEPTH, PRIME_GROUPS, chosen_depth};

    #[test]
    fn a_wide_first_probe_stays_at_depth_one() {
        assert_eq!(chosen_depth(PRIME_GROUPS, &[200, 400], 3), 1);
        assert_eq!(chosen_depth(PRIME_GROUPS + 10, &[10], 2), 1);
    }

    #[test]
    fn a_single_pattern_cannot_deepen() {
        assert_eq!(chosen_depth(3, &[], 1), 1);
        assert_eq!(chosen_depth(0, &[], 1), 1);
    }

    #[test]
    fn the_smallest_sufficient_depth_is_chosen() {
        assert_eq!(chosen_depth(10, &[PRIME_GROUPS], 3), 2);
        assert_eq!(chosen_depth(10, &[PRIME_GROUPS - 1, PRIME_GROUPS], 3), 3);
        assert_eq!(chosen_depth(10, &[PRIME_GROUPS, PRIME_GROUPS + 8], 3), 2);
    }

    #[test]
    fn a_short_list_stays_at_depth_one() {
        // No depth reaches PRIME_GROUPS, so the split stays the existing
        // first-row stream rather than paying a suffix walk for a short tape.
        assert_eq!(chosen_depth(4, &[12], 2), 1);
        assert_eq!(chosen_depth(4, &[12, 20], 3), 1);
        assert_eq!(chosen_depth(4, &[12], 3), 1);
        assert_eq!(chosen_depth(10, &[PRIME_GROUPS - 1], 2), 1);
    }

    #[test]
    fn depth_is_capped_at_three() {
        assert_eq!(MAX_PREFIX_DEPTH, 3);
        // A count at depth 4 is ignored even when it would reach K.
        assert_eq!(chosen_depth(2, &[8, 20, 80], 5), 1);
        assert_eq!(chosen_depth(2, &[8, PRIME_GROUPS, 80], 5), 3);
    }
}
