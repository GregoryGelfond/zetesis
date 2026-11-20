//! A′ parallel admission: worker-pure literals, packed transport, 56 groups.
//!
//! Workers see only worker-pure literals on interned snapshot columns. The
//! coordinator replays residuals and publishes, in a geometry that depends on
//! the data rather than `--threads`. Coordinate and integer columns are packed
//! to the narrowest width the observed range allows; a range that does not fit
//! `u32` / `i32` declines to serial instead of storing an unpacked `usize`.

use super::{Computation, Counters, FormulaFailure, FormulaLimits, ProgramSite};
use crate::formula_ir::{LiteralIr, Operation};
use crate::formula_support::evaluation::comparison_leaves_with;
use themelios_program::program::DefaultNegation;

pub(in crate::formula_support) use crate::formula_support::parallel_executor::PRIME_GROUPS;

/// Role of one body literal on the A′ path.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum LiteralRole {
    /// Safe to evaluate in a worker on the immutable epoch, without interning.
    WorkerPure,
    /// Evaluated only by the owner, in plan order, after staged rows exist.
    Residual,
    /// No correct A′ replay yet; the rule stays on the serial path.
    Serial,
}

/// Packed width for occurrence coordinates. `U16` is the default when it fits.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum CoordWidth {
    U16,
    U32,
}

/// Packed width for staged computed integers. `I16` is the default when it fits.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum IntWidth {
    I16,
    I32,
}

/// Classification of one rule body for A′ admission.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct Classification {
    pub(super) worker_pure: usize,
    pub(super) residual: usize,
    pub(super) integer_binds: usize,
}

/// Choose the narrowest coordinate width that can address `max_index`.
/// `None` means the range does not fit `u32` and the rule must decline.
pub(super) fn coord_width(max_index: usize) -> Option<CoordWidth> {
    if max_index <= usize::from(u16::MAX) {
        Some(CoordWidth::U16)
    } else if max_index <= u32::MAX as usize {
        Some(CoordWidth::U32)
    } else {
        None
    }
}

/// Choose the narrowest signed width that can hold every value in `min..=max`.
pub(super) fn int_width(min: i32, max: i32) -> IntWidth {
    let lo = i32::from(i16::MIN);
    let hi = i32::from(i16::MAX);
    if min >= lo && max <= hi {
        IntWidth::I16
    } else {
        IntWidth::I32
    }
}

/// Bytes for one packed occurrence at `width` (position + source index).
pub(super) fn coord_bytes(width: CoordWidth) -> usize {
    match width {
        CoordWidth::U16 => 2 * size_of::<u16>(),
        CoordWidth::U32 => 2 * size_of::<u32>(),
    }
}

/// Bytes for one packed integer at `width`.
pub(super) fn int_bytes(width: IntWidth) -> usize {
    match width {
        IntWidth::I16 => size_of::<i16>(),
        IntWidth::I32 => size_of::<i32>(),
    }
}

/// Contiguous nonempty groups of `width` items, at most `groups` of them.
///
/// The count and the bounds depend only on `width` and `groups`, never on the
/// host thread count. A short input yields one item per group. An empty input
/// yields no groups.
pub(super) fn group_count(width: usize, groups: usize) -> usize {
    if groups == 0 { 0 } else { width.min(groups) }
}

/// Start offset (inclusive) of group `index` in a `group_count(width, groups)`
/// partition. `index` must be in `0..=count`.
pub(super) fn group_start(width: usize, groups: usize, index: usize) -> usize {
    let count = group_count(width, groups);
    debug_assert!(index <= count);
    if count == 0 {
        return 0;
    }
    let length = width / count;
    let extra = width % count;
    let leading = index.min(extra);
    leading * (length + 1) + index.saturating_sub(extra) * length
}

/// Half-open range of group `index`.
pub(super) fn group_range(width: usize, groups: usize, index: usize) -> std::ops::Range<usize> {
    group_start(width, groups, index)..group_start(width, groups, index + 1)
}

/// Conservative staged-row reservation: half of the remaining support budget,
/// already the coordinator's retained grant, must cover `rows` packed rows.
/// Returns whether the A′ path may claim the work. A `false` result is a
/// decline to serial, never a budget error.
pub(super) fn reservation_fits(
    rows: usize,
    per_row: usize,
    remaining_support_bytes: usize,
) -> bool {
    let cap = remaining_support_bytes / 2;
    rows.saturating_mul(per_row) <= cap
}

/// Packed transport plan for one A′ staging run. `None` declines to serial:
/// either a coordinate does not fit `u32`, or the conservative half-budget
/// reservation would not cover a worst-case suffix of `patterns` occurrences
/// plus `integer_columns` computed integers per completed row.
pub(super) fn admit_reservation(
    rows: usize,
    patterns: usize,
    max_index: usize,
    integer_columns: usize,
    integer_min: i32,
    integer_max: i32,
    remaining_support_bytes: usize,
) -> Option<(CoordWidth, IntWidth)> {
    let coords = coord_width(max_index)?;
    let ints = int_width(integer_min, integer_max);
    let per_row = coord_bytes(coords)
        .saturating_mul(patterns)
        .saturating_add(int_bytes(ints).saturating_mul(integer_columns));
    if reservation_fits(rows, per_row, remaining_support_bytes) {
        Some((coords, ints))
    } else {
        None
    }
}

/// Classify a body for A′. `None` means decline to serial. Work is charged
/// once per literal and once per inspected expression node or pattern slot.
pub(super) fn classify_body(
    body: &[LiteralIr],
    variables: usize,
    computation: &Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<Option<Classification>, FormulaFailure> {
    if body.is_empty() {
        return Ok(None);
    }
    let words = variables.div_ceil(64);
    let mut bound = vec![0u64; words];
    let mut worker_pure = 0;
    let mut residual = 0;
    let mut integer_binds = 0;
    for literal in body {
        counters.work(limits, location)?;
        match literal_role(literal, &bound, variables, limits, counters, location)? {
            LiteralRole::Serial => return Ok(None),
            LiteralRole::Residual => residual += 1,
            LiteralRole::WorkerPure => {
                worker_pure += 1;
                record_worker_bindings(
                    literal,
                    &mut bound,
                    variables,
                    computation,
                    limits,
                    counters,
                    location,
                )?;
                if let LiteralIr::Bind { .. } = literal {
                    integer_binds += 1;
                }
            }
        }
    }
    if worker_pure == 0 {
        return Ok(None);
    }
    Ok(Some(Classification {
        worker_pure,
        residual,
        integer_binds,
    }))
}

fn literal_role(
    literal: &LiteralIr,
    bound: &[u64],
    variables: usize,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<LiteralRole, FormulaFailure> {
    match literal {
        LiteralIr::Atom(DefaultNegation::None, _) | LiteralIr::PatternAtom(_) => {
            Ok(LiteralRole::WorkerPure)
        }
        LiteralIr::Atom(DefaultNegation::Not, _) => Ok(LiteralRole::WorkerPure),
        LiteralIr::Atom(DefaultNegation::NotNot, _) => Ok(LiteralRole::Residual),
        LiteralIr::Compare(comparison) | LiteralIr::ArgumentCheck(comparison) => {
            let (expressions, _) = comparison.operands();
            if comparison_leaves_with(expressions, || counters.work(limits, location))?.is_some() {
                Ok(LiteralRole::WorkerPure)
            } else {
                // Non-total leaves may construct terms; replay is not yet wired.
                Ok(LiteralRole::Serial)
            }
        }
        LiteralIr::Bind { target, value } => {
            if *target < variables
                && integer_bind(value, bound, variables, limits, counters, location)?
            {
                Ok(LiteralRole::WorkerPure)
            } else if *target < variables {
                Ok(LiteralRole::Residual)
            } else {
                Ok(LiteralRole::Serial)
            }
        }
        LiteralIr::AnonymousAtom(_, _)
        | LiteralIr::TupleCompare(_, _, _)
        | LiteralIr::Guard(_)
        | LiteralIr::HeadGuard(_)
        | LiteralIr::Conditional(_)
        | LiteralIr::Aggregate(_)
        | LiteralIr::Range { .. } => Ok(LiteralRole::Serial),
    }
}

/// A bind is worker-pure only when every node is numeric (no constructor) and
/// every variable is already bound by a worker-pure literal.
fn integer_bind(
    value: &crate::formula_ir::Expression,
    bound: &[u64],
    variables: usize,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<bool, FormulaFailure> {
    if value.nodes.is_empty() {
        return Ok(false);
    }
    for node in &value.nodes {
        counters.work(limits, location)?;
        match node {
            Operation::Constant(_) => {}
            Operation::Variable(slot) => {
                if *slot >= variables || !bit_is_set(bound, *slot) {
                    return Ok(false);
                }
            }
            Operation::Unary(_, operand) | Operation::Absolute(operand) => {
                if *operand >= value.nodes.len() {
                    return Ok(false);
                }
            }
            Operation::Binary(_, left, right) => {
                if *left >= value.nodes.len() || *right >= value.nodes.len() {
                    return Ok(false);
                }
            }
            Operation::Constructor(_) => return Ok(false),
        }
    }
    Ok(true)
}

fn record_worker_bindings(
    literal: &LiteralIr,
    bound: &mut [u64],
    variables: usize,
    computation: &Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<(), FormulaFailure> {
    match literal {
        LiteralIr::Atom(DefaultNegation::None, pattern) => {
            let pattern = computation.static_pattern(*pattern, limits, counters, location)?;
            let terms = pattern.terms();
            bind_term_slots(
                terms.len(),
                |column| match terms.get(column) {
                    Some(zetesis_core::TemplateTerm::Variable(slot)) => Some(slot),
                    _ => None,
                },
                bound,
                variables,
                limits,
                counters,
                location,
            )?;
        }
        LiteralIr::PatternAtom(pattern) => {
            let flat = computation.static_pattern(pattern.atom, limits, counters, location)?;
            let terms = flat.terms();
            bind_term_slots(
                terms.len(),
                |column| match terms.get(column) {
                    Some(zetesis_core::TemplateTerm::Variable(slot)) => Some(slot),
                    _ => None,
                },
                bound,
                variables,
                limits,
                counters,
                location,
            )?;
            for argument in &pattern.arguments {
                counters.work(limits, location)?;
                for node in &argument.nodes {
                    if let crate::formula_pattern::PatternNode::Slot(slot) = node
                        && *slot < variables
                    {
                        set_bit(bound, *slot);
                    }
                }
            }
        }
        LiteralIr::Bind { target, .. } if *target < variables => set_bit(bound, *target),
        _ => {}
    }
    Ok(())
}

fn set_bit(bits: &mut [u64], slot: usize) {
    let word = slot / 64;
    if word < bits.len() {
        bits[word] |= 1 << (slot % 64);
    }
}

fn bit_is_set(bits: &[u64], slot: usize) -> bool {
    let word = slot / 64;
    word < bits.len() && bits[word] & (1 << (slot % 64)) != 0
}

fn bind_term_slots(
    arity: usize,
    mut variable: impl FnMut(usize) -> Option<usize>,
    bound: &mut [u64],
    variables: usize,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<(), FormulaFailure> {
    for column in 0..arity {
        counters.work(limits, location)?;
        if let Some(slot) = variable(column)
            && slot < variables
        {
            set_bit(bound, slot);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
