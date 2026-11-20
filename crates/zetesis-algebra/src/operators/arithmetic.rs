//! Checked integer kernels used by Bind and Filter.
//!
//! Operations follow the scalar contract of the pinned evaluator: overflow
//! and undefined quotients or remainders refuse a row rather than wrap. Loops
//! over columnar slices stay simple so the compiler can auto-vectorize them.
//! Hand-written SIMD is not used here.

use super::template::{Arithmetic, Comparison, Rel, Term};

/// Work recorded while enumerating Bind and Filter, for dual-option tests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EnumerationStats {
    /// Domain tuples and generated candidates visited, including failures.
    pub candidates: u64,
    /// Bindings that passed every residual test.
    pub retained: u64,
    /// Inclusive-range and relational comparisons.
    pub integer_comparisons: u64,
    /// Checked arithmetic evaluations, including undefined attempts.
    pub expression_evaluations: u64,
}

pub(super) fn remainder(left: i32, right: i32) -> Option<i32> {
    if right == 0 {
        return None;
    }
    left.checked_rem(right)
}

pub(super) fn quotient(left: i32, right: i32) -> Option<i32> {
    if right == 0 {
        return None;
    }
    left.checked_div(right)
}

pub(super) fn evaluate_arithmetic(op: Arithmetic, row: &[i32]) -> Option<bool> {
    match op {
        Arithmetic::RemainderZero { dividend, divisor } => {
            Some(remainder(row[dividend], row[divisor])? == 0)
        }
        Arithmetic::Quotient {
            dividend,
            divisor,
            quot,
        } => Some(quotient(row[dividend], row[divisor])? == row[quot]),
        Arithmetic::Plus {
            left,
            right,
            result,
        } => Some(row[left].checked_add(row[right])? == row[result]),
        Arithmetic::Times {
            left,
            right,
            result,
        } => Some(row[left].checked_mul(row[right])? == row[result]),
        Arithmetic::Abs { input, result } => Some(row[input].checked_abs()? == row[result]),
    }
}

pub(super) fn evaluate_comparison(comparison: Comparison, row: &[i32]) -> bool {
    let Comparison::Rel {
        left,
        relation,
        right,
    } = comparison;
    let left = term(left, row);
    let right = term(right, row);
    match relation {
        Rel::Eq => left == right,
        Rel::Ne => left != right,
        Rel::Lt => left < right,
        Rel::Le => left <= right,
        Rel::Gt => left > right,
        Rel::Ge => left >= right,
    }
}

fn term(term: Term, row: &[i32]) -> i32 {
    match term {
        Term::Slot(slot) => row[slot],
        Term::Const(value) => value,
    }
}

/// First multiple of `divisor` that is at least `lo`, if it fits in `i32`.
pub(super) fn first_multiple(divisor: i32, lo: i32) -> Option<i32> {
    let step = divisor.checked_abs()?;
    if step == 0 {
        return None;
    }
    let lo = i64::from(lo);
    let step = i64::from(step);
    let rem = lo.rem_euclid(step);
    let first = if rem == 0 { lo } else { lo + (step - rem) };
    i32::try_from(first).ok()
}

/// Append every multiple of `divisor` in `lo..=hi` to `out`.
pub(super) fn multiples_in_range(divisor: i32, lo: i32, hi: i32, out: &mut Vec<i32>) {
    let Some(step) = divisor.checked_abs().filter(|&step| step > 0) else {
        return;
    };
    let Some(mut value) = first_multiple(divisor, lo) else {
        return;
    };
    while value <= hi {
        out.push(value);
        match value.checked_add(step) {
            Some(next) => value = next,
            None => break,
        }
    }
}

/// How many multiples of `divisor` lie in `lo..=hi`, without materializing them.
pub(super) fn multiples_count(divisor: i32, lo: i32, hi: i32) -> u64 {
    if lo > hi {
        return 0;
    }
    let Some(step) = divisor.checked_abs().filter(|&step| step > 0) else {
        return 0;
    };
    let Some(first) = first_multiple(divisor, lo) else {
        return 0;
    };
    if first > hi {
        return 0;
    }
    let span = i64::from(hi) - i64::from(first);
    // first <= hi and step > 0, so both quantities are non-negative.
    span.cast_unsigned() / u64::from(step.cast_unsigned()) + 1
}

/// Whether `value` is a multiple of `divisor` in `lo..=hi`, matching
/// [`multiples_in_range`].
pub(super) fn is_multiple_in_range(divisor: i32, lo: i32, hi: i32, value: i32) -> bool {
    if value < lo || value > hi {
        return false;
    }
    let Some(step) = divisor.checked_abs().filter(|&step| step > 0) else {
        return false;
    };
    i64::from(value).rem_euclid(i64::from(step)) == 0
}

/// Append every divisor of `value` that lies in `lo..=hi`.
pub(super) fn divisors_in_range(value: i32, lo: i32, hi: i32, out: &mut Vec<i32>) {
    if value == 0 {
        // 0 rem y = 0 for every y != 0; the finite domain is the whole range
        // excluding zero.
        append_range_excluding_zero(lo, hi, out);
        return;
    }
    let Some(abs) = value.checked_abs() else {
        return;
    };
    let mut factor = 1i32;
    while factor.saturating_mul(factor) <= abs {
        if abs % factor == 0 {
            consider_divisor(factor, lo, hi, out);
            consider_divisor(-factor, lo, hi, out);
            let pair = abs / factor;
            if pair != factor {
                consider_divisor(pair, lo, hi, out);
                consider_divisor(-pair, lo, hi, out);
            }
        }
        factor = match factor.checked_add(1) {
            Some(next) => next,
            None => break,
        };
    }
}

fn consider_divisor(value: i32, lo: i32, hi: i32, out: &mut Vec<i32>) {
    if value >= lo && value <= hi {
        out.push(value);
    }
}

fn append_range_excluding_zero(lo: i32, hi: i32, out: &mut Vec<i32>) {
    let mut value = lo;
    loop {
        if value != 0 {
            out.push(value);
        }
        if value == hi {
            break;
        }
        match value.checked_add(1) {
            Some(next) => value = next,
            None => break,
        }
    }
}

/// Possible values of `dividend` such that `dividend / divisor = quot`.
pub(super) fn dividends_for_quotient(
    divisor: i32,
    quot: i32,
    lo: i32,
    hi: i32,
    out: &mut Vec<i32>,
) {
    if divisor == 0 {
        return;
    }
    let Some(base) = quot.checked_mul(divisor) else {
        return;
    };
    // Integer division truncates toward zero, so the preimage is the `divisor`
    // long interval that starts at `base` and steps toward zero's opposite
    // side: for divisor > 0, [base, base + divisor - 1] or the negative
    // symmetric when quot and divisor are signed.
    let Some(step) = divisor.checked_abs().filter(|&step| step > 0) else {
        return;
    };
    // For toward-zero division, n/d = q iff n is in the truncated interval
    // of length |d| starting at q*d and moving away from zero if q != 0,
    // or the open interval around zero when q == 0.
    let (start, end) = if quot == 0 {
        // n/d = 0 iff |n| < |d|.
        let last = step - 1;
        (-last, last)
    } else if divisor > 0 {
        if quot > 0 {
            (base, base.saturating_add(step - 1))
        } else {
            (base.saturating_sub(step - 1), base)
        }
    } else if quot > 0 {
        (base.saturating_sub(step - 1), base)
    } else {
        (base, base.saturating_add(step - 1))
    };
    let lo_bound = start.max(lo);
    let hi_bound = end.min(hi);
    let mut value = lo_bound;
    while value <= hi_bound {
        out.push(value);
        match value.checked_add(1) {
            Some(next) => value = next,
            None => break,
        }
    }
}
