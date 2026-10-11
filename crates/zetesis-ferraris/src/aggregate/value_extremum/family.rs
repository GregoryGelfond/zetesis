//! Native ordinal cuts over one complete typed value row.

use zetesis_core::{Value, catalog::TermRef};
use zetesis_cpu::Cancellation;

use super::{ValueExtremumElement, ranked};
use crate::aggregate::extremum::comparison_root;
use crate::aggregate::lower::{Builder, Destination, transaction_value, validate_prefix};
use crate::{
    AggregateComparison, AggregateError, AggregateErrorKind, AggregateExtremum,
    AggregateFamilyBuild, AggregateFamilyLimits, AggregateProfile, ExtremumCutRule, ExtremumRow,
    FormulaNodes, NodeView,
};

/// One typed comparison against the family's unchanged complete tuple row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValueExtremumGuard<V = Value> {
    /// Aggregate comparison, including reduct-sensitive not-equal.
    pub comparison: AggregateComparison,
    /// Bound interpreted in ASP term order, never storage order.
    pub bound: V,
}

/// One typed-family outcome and its actual auxiliary capacity receipt.
///
/// Auxiliary bytes describe family preparation and returned roots separately
/// from graph storage and the aggregate's logical work statistics.
#[derive(Debug)]
#[must_use]
pub struct ValueExtremumFamilyAttempt {
    /// Ordered roots on success, or the original typed failure with completed
    /// work accounting. A failed result has rolled back the entire graph suffix.
    pub result: Result<AggregateFamilyBuild, AggregateError>,
    /// Peak actual scratch/output vector bytes, including all vector headers
    /// and capacity from failed reservations. Zero means no auxiliary allocation
    /// was attempted; a zero-capacity attempt still reports its vector headers.
    /// Borrowed inputs and DAG storage belong to separate owners and are excluded.
    pub auxiliary_peak_bytes: u128,
}

/// Checked requested auxiliary storage for one typed extremum family.
///
/// Preparation owns dictionary indexes (E + 2), conditions (E), u64 ranks (E),
/// and two cut rules per guard (2G). These cells share the state ceiling; root
/// cells obey the separate guard ceiling. The dictionary depends only on the
/// value row, includes both endpoints and preserves full ASP term order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValueExtremumFamilySpace {
    states: usize,
    bytes: usize,
}
impl ValueExtremumFamilySpace {
    /// Checked requested peak before selecting shared preparation. Includes all
    /// five vector headers and requested capacities, excluding borrowed input
    /// and graph storage. Empty guard families allocate nothing. Allocator excess
    /// is reported by [`ValueExtremumFamilyAttempt::auxiliary_peak_bytes`].
    #[must_use]
    pub fn new(elements: usize, guards: usize) -> Option<Self> {
        if guards == 0 {
            return Some(Self {
                states: 0,
                bytes: 0,
            });
        }
        let dictionary = elements.checked_add(2)?;
        let cuts = guards.checked_mul(2)?;
        let states = dictionary
            .checked_add(elements.checked_mul(2)?)?
            .checked_add(cuts)?;
        let bytes = dictionary
            .checked_add(elements)?
            .checked_add(guards)?
            .checked_mul(size_of::<usize>())?
            .checked_add(elements.checked_mul(size_of::<u64>())?)?
            .checked_add(cuts.checked_mul(size_of::<ExtremumCutRule>())?)?
            .checked_add(size_of::<Preparation>())?;
        Some(Self { states, bytes })
    }

    /// Peak temporary cells charged against `AggregateLimits::max_states`.
    #[must_use]
    pub const fn states(self) -> usize {
        self.states
    }

    /// Requested auxiliary bytes, including roots while preparation is live.
    #[must_use]
    pub const fn storage_bytes(self) -> usize {
        self.bytes
    }
}

/// Append typed min/max guards over one shared, guard-independent ordinal row.
///
/// The complete value dictionary includes #inf and #sup and is sorted using
/// charged ASP term comparisons. Equal dictionary values share a rank; every
/// complete tuple occurrence retains its condition and rank, including repeated
/// condition IDs. Two native inclusive/strict cuts are published per guard.
/// Not-equal remains the inclusive-to-strict implication, and each cut and
/// wrapper retains its own frozen truth gate for arbitrary inner worlds.
///
/// The caller coalesces equal complete tuples and establishes row independence
/// from the changing guard. Borrowed term authorities stay unchanged. Preparation
/// uses O(E log E + G log E) comparisons and O(E + G) auxiliary cells. All
/// conditions are validated even with no guards. Work, state, guard and graph
/// ceilings apply to the whole family; refusal rolls back the entire suffix.
/// Scalar extrema retain their separate zero-state compilation contract.
///
/// # Errors
/// Returns typed input, guard, state, allocation, work, node, operand, arithmetic
/// or control failures, with actual auxiliary capacity on every outcome.
pub fn append_value_extremum_family_refs(
    nodes: &mut FormulaNodes,
    elements: &[ValueExtremumElement<TermRef<'_>>],
    extremum: AggregateExtremum,
    guards: &[ValueExtremumGuard<TermRef<'_>>],
    limits: AggregateFamilyLimits,
    cancellation: &Cancellation,
) -> ValueExtremumFamilyAttempt {
    append(
        Destination::retained(nodes.transaction()),
        elements,
        extremum,
        guards,
        limits,
        cancellation,
    )
}

pub(in crate::aggregate) fn append(
    destination: Destination<'_>,
    elements: &[ValueExtremumElement<TermRef<'_>>],
    extremum: AggregateExtremum,
    guards: &[ValueExtremumGuard<TermRef<'_>>],
    limits: AggregateFamilyLimits,
    cancellation: &Cancellation,
) -> ValueExtremumFamilyAttempt {
    let mut auxiliary_peak_bytes = 0;
    let result = transaction_value(destination, limits.aggregate, cancellation, |builder| {
        compile(
            builder,
            elements,
            extremum,
            guards,
            limits,
            &mut auxiliary_peak_bytes,
        )
    })
    .map(|(roots, statistics)| {
        AggregateFamilyBuild::new(roots, AggregateProfile::Extremum, statistics)
    });
    ValueExtremumFamilyAttempt {
        result,
        auxiliary_peak_bytes,
    }
}

fn compile(
    builder: &mut Builder<'_, '_>,
    elements: &[ValueExtremumElement<TermRef<'_>>],
    extremum: AggregateExtremum,
    guards: &[ValueExtremumGuard<TermRef<'_>>],
    limits: AggregateFamilyLimits,
    auxiliary_peak_bytes: &mut u128,
) -> Result<Vec<usize>, AggregateErrorKind> {
    let prefix = validate_prefix(builder)?;
    for (index, element) in elements.iter().enumerate() {
        builder.tick()?;
        if index >= limits.aggregate.max_elements {
            return Err(AggregateErrorKind::ElementLimit);
        }
        if element.condition >= prefix {
            return Err(AggregateErrorKind::InvalidCondition { element: index });
        }
    }
    if guards.len() > limits.max_guards {
        return Err(AggregateErrorKind::GuardLimit);
    }
    if guards.is_empty() {
        return Ok(Vec::new());
    }
    let space = ValueExtremumFamilySpace::new(elements.len(), guards.len())
        .ok_or(AggregateErrorKind::ArithmeticOverflow)?;
    builder.states(space.states())?;
    let mut preparation = Preparation::default();
    preparation.reserve(builder, elements.len(), guards.len(), auxiliary_peak_bytes)?;
    let rank_count = preparation.classify(builder, elements, extremum, guards)?;
    let cut_nodes = builder.extremum_family(
        ExtremumRow {
            conditions: &preparation.conditions,
            ranks: &preparation.ranks,
            rank_count,
        },
        &preparation.cuts,
    )?;
    let falsum = builder.push(NodeView::False)?;
    for (index, guard) in guards.iter().enumerate() {
        builder.tick()?;
        let inclusive = cut_nodes.start + index * 2;
        let root = comparison_root(
            builder,
            extremum,
            guard.comparison,
            inclusive,
            inclusive + 1,
            falsum,
        )?;
        builder.tick()?;
        preparation.roots.push(root);
    }
    Ok(preparation.roots)
}

#[derive(Default)]
struct Preparation {
    dictionary: Vec<usize>,
    conditions: Vec<usize>,
    ranks: Vec<u64>,
    cuts: Vec<ExtremumCutRule>,
    roots: Vec<usize>,
}
impl Preparation {
    fn classify(
        &mut self,
        builder: &mut Builder<'_, '_>,
        elements: &[ValueExtremumElement<TermRef<'_>>],
        extremum: AggregateExtremum,
        guards: &[ValueExtremumGuard<TermRef<'_>>],
    ) -> Result<u64, AggregateErrorKind> {
        for index in 0..elements.len() + 2 {
            builder.tick()?;
            self.dictionary.push(index);
        }
        for element in elements {
            builder.tick()?;
            self.conditions.push(element.condition);
            builder.tick()?;
            self.ranks.push(0);
        }
        let distinct = ranked::prepare(builder, &mut self.dictionary, &mut self.ranks, elements)?;
        for guard in guards {
            let lower = ranked::insertion(
                builder,
                guard.bound,
                &self.dictionary[..distinct],
                elements,
                false,
            )?;
            let upper = ranked::insertion(
                builder,
                guard.bound,
                &self.dictionary[..distinct],
                elements,
                true,
            )?;
            let (inclusive, strict) = match extremum {
                AggregateExtremum::Min => (
                    ExtremumCutRule::Prefix(upper),
                    ExtremumCutRule::Prefix(lower),
                ),
                AggregateExtremum::Max => (
                    ExtremumCutRule::Suffix(lower),
                    ExtremumCutRule::Suffix(upper),
                ),
            };
            builder.tick()?;
            self.cuts.push(inclusive);
            builder.tick()?;
            self.cuts.push(strict);
        }
        u64::try_from(distinct).map_err(|_| AggregateErrorKind::ArithmeticOverflow)
    }

    fn reserve(
        &mut self,
        builder: &mut Builder<'_, '_>,
        elements: usize,
        guards: usize,
        peak: &mut u128,
    ) -> Result<(), AggregateErrorKind> {
        let reserved = self.dictionary.try_reserve_exact(elements + 2);
        self.observe(builder, peak, reserved)?;
        let reserved = self.conditions.try_reserve_exact(elements);
        self.observe(builder, peak, reserved)?;
        let reserved = self.ranks.try_reserve_exact(elements);
        self.observe(builder, peak, reserved)?;
        let reserved = self.cuts.try_reserve_exact(guards * 2);
        self.observe(builder, peak, reserved)?;
        let reserved = self.roots.try_reserve_exact(guards);
        self.observe(builder, peak, reserved)
    }

    fn observe(
        &self,
        builder: &mut Builder<'_, '_>,
        peak: &mut u128,
        reserved: Result<(), std::collections::TryReserveError>,
    ) -> Result<(), AggregateErrorKind> {
        let bytes = size_of::<Self>() as u128
            + (self.dictionary.capacity() as u128
                + self.conditions.capacity() as u128
                + self.roots.capacity() as u128)
                * size_of::<usize>() as u128
            + self.ranks.capacity() as u128 * size_of::<u64>() as u128
            + self.cuts.capacity() as u128 * size_of::<ExtremumCutRule>() as u128;
        *peak = (*peak).max(bytes);
        let states = self.dictionary.capacity() as u128
            + self.conditions.capacity() as u128
            + self.ranks.capacity() as u128
            + self.cuts.capacity() as u128;
        builder.allocated_states(usize::try_from(states).unwrap_or(usize::MAX));
        reserved.map_err(|_| AggregateErrorKind::Allocation)?;
        let states = usize::try_from(states).map_err(|_| AggregateErrorKind::StateLimit)?;
        builder.states(states)
    }
}

#[cfg(test)]
mod tests;
