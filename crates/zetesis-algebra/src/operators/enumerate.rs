//! Domain enumeration for Bind, with filter-only and generating schedules.

use std::collections::HashSet;

use super::arithmetic::{
    EnumerationStats, dividends_for_quotient, divisors_in_range, evaluate_arithmetic,
    evaluate_comparison, is_multiple_in_range, multiples_count, multiples_in_range, remainder,
};
use super::relation::Bindings;
use super::template::{Arithmetic, Comparison, DependentRange, Schedule, Template};

pub(super) fn rows<E>(
    template: &Template,
    account: &mut impl FnMut(u64) -> Result<(), E>,
    stats: &mut EnumerationStats,
) -> Result<Bindings, E> {
    let width = template.width();
    let mut bindings = Bindings::empty(width);
    if width == 0 {
        account(1)?;
        bindings.push_empty();
        stats.candidates = 1;
        stats.retained = 1;
        return Ok(bindings);
    }
    let plan = Plan::new(template);
    let mut row = vec![0i32; width];
    let mut bound = vec![false; width];
    Engine {
        plan: &plan,
        row: &mut row,
        bound: &mut bound,
        bindings: &mut bindings,
        account,
        stats,
    }
    .walk()?;
    Ok(bindings)
}

struct Engine<'a, E, A>
where
    A: FnMut(u64) -> Result<(), E>,
{
    plan: &'a Plan<'a>,
    row: &'a mut [i32],
    bound: &'a mut [bool],
    bindings: &'a mut Bindings,
    account: &'a mut A,
    stats: &'a mut EnumerationStats,
}

struct Plan<'a> {
    ranges: Vec<(usize, i32, i32)>,
    facts: Vec<(usize, &'a [i32])>,
    /// Same columns as `facts`, as sets, so a generated candidate can be
    /// tested for membership without treating the fact hull as a dense range.
    fact_sets: Vec<(usize, HashSet<i32>)>,
    dependent: Vec<DependentRange>,
    generated: Vec<Generated>,
    residual_arithmetic: Vec<Arithmetic>,
    residual_comparisons: &'a [Comparison],
    residual_bounds: Vec<AffineBound>,
}

/// Residual `slot <= hi_slot + hi_delta` kept when a dependent range is lifted.
#[derive(Clone, Copy)]
struct AffineBound {
    slot: usize,
    hi_slot: usize,
    hi_delta: i32,
}

#[derive(Clone, Copy)]
enum Generated {
    Remainder {
        dest: usize,
        src: usize,
        lo: i32,
        hi: i32,
    },
    Divisors {
        dest: usize,
        src: usize,
        lo: i32,
        hi: i32,
    },
    Plus {
        dest: usize,
        left: usize,
        right: usize,
    },
    Times {
        dest: usize,
        left: usize,
        right: usize,
    },
    Abs {
        dest: usize,
        src: usize,
    },
    AbsInv {
        dest: usize,
        src: usize,
        lo: i32,
        hi: i32,
    },
    Quotient {
        dest: usize,
        dividend: usize,
        divisor: usize,
    },
    Dividend {
        dest: usize,
        quot: usize,
        divisor: usize,
        lo: i32,
        hi: i32,
    },
}

impl<'a> Plan<'a> {
    fn new(template: &'a Template) -> Self {
        let mut ranges: Vec<(usize, i32, i32)> = template
            .ranges()
            .iter()
            .map(|range| (range.slot, range.lo, range.hi))
            .collect();
        let mut dependent = template.dependent().to_vec();
        let mut residual_bounds = Vec::new();
        if template.schedule() == Schedule::Generating {
            residual_bounds = normalize_dependent(template, &mut ranges, &mut dependent);
        }
        let facts: Vec<(usize, &[i32])> = template
            .facts()
            .iter()
            .map(|fact| (fact.slot, fact.values.as_slice()))
            .collect();
        let fact_sets: Vec<(usize, HashSet<i32>)> = facts
            .iter()
            .map(|&(slot, values)| (slot, values.iter().copied().collect()))
            .collect();
        let mut generated = Vec::new();
        let mut residual_arithmetic = Vec::new();
        if template.schedule() == Schedule::Generating {
            for &op in template.arithmetic() {
                if let Some(step) = generate_step(op, &ranges, &facts) {
                    generated.push(step);
                } else {
                    residual_arithmetic.push(op);
                }
            }
        } else {
            residual_arithmetic.extend_from_slice(template.arithmetic());
        }
        Self {
            ranges,
            facts,
            fact_sets,
            dependent,
            generated,
            residual_arithmetic,
            residual_comparisons: template.comparisons(),
            residual_bounds,
        }
    }
}

fn domain_bounds(
    ranges: &[(usize, i32, i32)],
    facts: &[(usize, &[i32])],
    slot: usize,
) -> Option<(i32, i32)> {
    if let Some(&(_, lo, hi)) = ranges.iter().find(|range| range.0 == slot) {
        return Some((lo, hi));
    }
    let values = facts.iter().find(|fact| fact.0 == slot)?.1;
    let lo = *values.iter().min()?;
    let hi = *values.iter().max()?;
    Some((lo, hi))
}

fn has_domain(ranges: &[(usize, i32, i32)], facts: &[(usize, &[i32])], slot: usize) -> bool {
    domain_bounds(ranges, facts, slot).is_some()
}

fn normalize_dependent(
    template: &Template,
    ranges: &mut Vec<(usize, i32, i32)>,
    dependent: &mut Vec<DependentRange>,
) -> Vec<AffineBound> {
    let facts: Vec<(usize, &[i32])> = template
        .facts()
        .iter()
        .map(|fact| (fact.slot, fact.values.as_slice()))
        .collect();
    let existing = ranges.clone();
    let mut bounds = Vec::new();
    dependent.retain(|item| {
        let Some((_, parent_hi)) = domain_bounds(&existing, &facts, item.hi_slot) else {
            return true;
        };
        let Some(outer) = parent_hi.checked_add(item.hi_delta) else {
            return true;
        };
        let mut hi = outer;
        if should_tighten_half(template, item) && parent_hi > 0 {
            hi = hi.min(parent_hi / 2);
        }
        ranges.push((item.slot, item.lo, hi));
        bounds.push(AffineBound {
            slot: item.slot,
            hi_slot: item.hi_slot,
            hi_delta: item.hi_delta,
        });
        false
    });
    bounds
}

fn should_tighten_half(template: &Template, item: &DependentRange) -> bool {
    item.hi_delta < 0
        && template.arithmetic().iter().any(|op| {
            matches!(
                op,
                Arithmetic::RemainderZero { dividend, divisor }
                    if *dividend == item.hi_slot && *divisor == item.slot
            )
        })
}

fn generate_step(
    op: Arithmetic,
    ranges: &[(usize, i32, i32)],
    facts: &[(usize, &[i32])],
) -> Option<Generated> {
    match op {
        Arithmetic::RemainderZero { dividend, divisor } => {
            let dest_bounds = domain_bounds(ranges, facts, dividend);
            let src_bounds = domain_bounds(ranges, facts, divisor);
            if src_bounds.is_some() {
                let (lo, hi) = dest_bounds?;
                Some(Generated::Remainder {
                    dest: dividend,
                    src: divisor,
                    lo,
                    hi,
                })
            } else if dest_bounds.is_some() {
                let (lo, hi) = domain_bounds(ranges, facts, divisor)?;
                Some(Generated::Divisors {
                    dest: divisor,
                    src: dividend,
                    lo,
                    hi,
                })
            } else {
                None
            }
        }
        Arithmetic::Plus {
            left,
            right,
            result,
        } => generate_binary(left, right, result, ranges, facts, true),
        Arithmetic::Times {
            left,
            right,
            result,
        } => generate_binary(left, right, result, ranges, facts, false),
        Arithmetic::Abs { input, result } => {
            if has_domain(ranges, facts, input) && !has_domain(ranges, facts, result) {
                Some(Generated::Abs {
                    dest: result,
                    src: input,
                })
            } else if has_domain(ranges, facts, result) {
                let (lo, hi) = domain_bounds(ranges, facts, input)?;
                Some(Generated::AbsInv {
                    dest: input,
                    src: result,
                    lo,
                    hi,
                })
            } else if has_domain(ranges, facts, input) {
                let (lo, hi) = domain_bounds(ranges, facts, result)?;
                let _ = (lo, hi);
                Some(Generated::Abs {
                    dest: result,
                    src: input,
                })
            } else {
                None
            }
        }
        Arithmetic::Quotient {
            dividend,
            divisor,
            quot,
        } => {
            if has_domain(ranges, facts, dividend) && has_domain(ranges, facts, divisor) {
                Some(Generated::Quotient {
                    dest: quot,
                    dividend,
                    divisor,
                })
            } else if has_domain(ranges, facts, quot) && has_domain(ranges, facts, divisor) {
                let (lo, hi) = domain_bounds(ranges, facts, dividend)?;
                Some(Generated::Dividend {
                    dest: dividend,
                    quot,
                    divisor,
                    lo,
                    hi,
                })
            } else {
                None
            }
        }
    }
}

fn generate_binary(
    left: usize,
    right: usize,
    result: usize,
    ranges: &[(usize, i32, i32)],
    facts: &[(usize, &[i32])],
    plus: bool,
) -> Option<Generated> {
    let have_l = has_domain(ranges, facts, left);
    let have_r = has_domain(ranges, facts, right);
    let have_z = has_domain(ranges, facts, result);
    if have_l && have_r {
        return Some(if plus {
            Generated::Plus {
                dest: result,
                left,
                right,
            }
        } else {
            Generated::Times {
                dest: result,
                left,
                right,
            }
        });
    }
    let _ = have_z;
    None
}

impl<E, A> Engine<'_, E, A>
where
    A: FnMut(u64) -> Result<(), E>,
{
    fn walk(&mut self) -> Result<(), E> {
        if let Some(step) = ready_generated(self.plan, self.bound) {
            return self.generate(step);
        }
        if let Some(item) = ready_dependent(self.plan, self.bound) {
            let Some(hi) = self.row[item.hi_slot].checked_add(item.hi_delta) else {
                return Ok(());
            };
            return self.fill_range(item.slot, item.lo, hi);
        }
        if let Some((slot, lo, hi)) = next_range(self.plan, self.bound) {
            return self.fill_range(slot, lo, hi);
        }
        if let Some((slot, values)) = next_facts(self.plan, self.bound) {
            for &value in values {
                self.bind_and_descend(slot, value)?;
            }
            return Ok(());
        }
        if !self.bound.iter().all(|flag| *flag)
            && self.plan.ranges.is_empty()
            && self.plan.facts.is_empty()
        {
            // Unmentioned slots stay zero and are treated as bound so an empty
            // positive body with only residual tests still has a carrier.
            for flag in self.bound.iter_mut() {
                *flag = true;
            }
            return self.walk();
        }
        if !self.bound.iter().all(|flag| *flag) {
            return Ok(());
        }
        self.finish_row()
    }

    fn fill_range(&mut self, slot: usize, lo: i32, hi: i32) -> Result<(), E> {
        if lo > hi {
            return Ok(());
        }
        let mut value = lo;
        loop {
            self.bind_and_descend(slot, value)?;
            if value == hi {
                break;
            }
            match value.checked_add(1) {
                Some(next) => value = next,
                None => break,
            }
        }
        Ok(())
    }

    fn bind_and_descend(&mut self, slot: usize, value: i32) -> Result<(), E> {
        self.row[slot] = value;
        self.bound[slot] = true;
        self.walk()?;
        self.bound[slot] = false;
        Ok(())
    }

    fn generate(&mut self, step: Generated) -> Result<(), E> {
        let dest = generated_dest(step);
        if let Some(facts) = self
            .plan
            .facts
            .iter()
            .find(|fact| fact.0 == dest)
            .map(|fact| fact.1)
        {
            return self.generate_into_facts(step, dest, facts);
        }
        let mut values = Vec::new();
        collect_generated(step, self.row, &mut values, self.stats);
        for value in values {
            self.bind_and_descend(dest, value)?;
        }
        Ok(())
    }

    /// A fact column is a set, not the interval between its least and greatest
    /// values. Generating multiples through that hull would visit (and could
    /// retain) integers the relation does not contain, so the cheaper of
    /// filtering the facts or membership-testing the generated span is used.
    fn generate_into_facts(
        &mut self,
        step: Generated,
        dest: usize,
        facts: &[i32],
    ) -> Result<(), E> {
        let estimate = generated_estimate(step, self.row);
        if (facts.len() as u64) <= estimate {
            for &value in facts {
                if fact_satisfies(step, self.row, value) {
                    self.bind_and_descend(dest, value)?;
                }
            }
            return Ok(());
        }
        let mut values = Vec::new();
        collect_generated(step, self.row, &mut values, self.stats);
        let set = self
            .plan
            .fact_sets
            .iter()
            .find(|facts| facts.0 == dest)
            .map(|facts| &facts.1);
        for value in values {
            if set.is_some_and(|set| set.contains(&value)) {
                self.bind_and_descend(dest, value)?;
            }
        }
        Ok(())
    }

    fn finish_row(&mut self) -> Result<(), E> {
        self.stats.candidates = self.stats.candidates.saturating_add(1);
        (self.account)(1)?;
        for &op in &self.plan.residual_arithmetic {
            self.stats.expression_evaluations = self.stats.expression_evaluations.saturating_add(1);
            (self.account)(1)?;
            match evaluate_arithmetic(op, self.row) {
                Some(true) => {}
                Some(false) | None => return Ok(()),
            }
        }
        for &bound in &self.plan.residual_bounds {
            self.stats.integer_comparisons = self.stats.integer_comparisons.saturating_add(1);
            (self.account)(1)?;
            let Some(limit) = self.row[bound.hi_slot].checked_add(bound.hi_delta) else {
                return Ok(());
            };
            if self.row[bound.slot] > limit {
                return Ok(());
            }
        }
        for &comparison in self.plan.residual_comparisons {
            self.stats.integer_comparisons = self.stats.integer_comparisons.saturating_add(1);
            (self.account)(1)?;
            if !evaluate_comparison(comparison, self.row) {
                return Ok(());
            }
        }
        self.stats.retained = self.stats.retained.saturating_add(1);
        (self.account)(1)?;
        self.bindings.push_row(self.row);
        Ok(())
    }
}

fn generated_dest(step: Generated) -> usize {
    match step {
        Generated::Remainder { dest, .. }
        | Generated::Divisors { dest, .. }
        | Generated::Plus { dest, .. }
        | Generated::Times { dest, .. }
        | Generated::Abs { dest, .. }
        | Generated::AbsInv { dest, .. }
        | Generated::Quotient { dest, .. }
        | Generated::Dividend { dest, .. } => dest,
    }
}

fn generated_ready(step: Generated, bound: &[bool]) -> bool {
    if bound[generated_dest(step)] {
        return false;
    }
    match step {
        Generated::Remainder { src, .. }
        | Generated::Divisors { src, .. }
        | Generated::Abs { src, .. }
        | Generated::AbsInv { src, .. } => bound[src],
        Generated::Plus { left, right, .. }
        | Generated::Times { left, right, .. }
        | Generated::Quotient {
            dividend: left,
            divisor: right,
            ..
        }
        | Generated::Dividend {
            quot: left,
            divisor: right,
            ..
        } => bound[left] && bound[right],
    }
}

fn ready_generated(plan: &Plan<'_>, bound: &[bool]) -> Option<Generated> {
    plan.generated
        .iter()
        .copied()
        .find(|step| generated_ready(*step, bound))
}

fn ready_dependent(plan: &Plan<'_>, bound: &[bool]) -> Option<DependentRange> {
    plan.dependent
        .iter()
        .copied()
        .find(|item| bound[item.hi_slot] && !bound[item.slot])
}

fn next_range(plan: &Plan<'_>, bound: &[bool]) -> Option<(usize, i32, i32)> {
    plan.ranges
        .iter()
        .copied()
        .find(|range| !bound[range.0] && !is_generated_dest(plan, range.0))
}

fn next_facts<'a>(plan: &'a Plan<'a>, bound: &[bool]) -> Option<(usize, &'a [i32])> {
    plan.facts
        .iter()
        .copied()
        .find(|fact| !bound[fact.0] && !is_generated_dest(plan, fact.0))
}

fn is_generated_dest(plan: &Plan<'_>, slot: usize) -> bool {
    plan.generated
        .iter()
        .any(|step| generated_dest(*step) == slot)
}

fn generated_estimate(step: Generated, row: &[i32]) -> u64 {
    match step {
        Generated::Remainder { src, lo, hi, .. } => multiples_count(row[src], lo, hi),
        Generated::Divisors { src, lo, hi, .. } => {
            if row[src] == 0 {
                range_len_excluding_zero(lo, hi)
            } else {
                divisor_estimate(row[src])
            }
        }
        Generated::Dividend {
            lo, hi, divisor, ..
        } => {
            // One dividend per multiple of the divisor in the span, at most.
            multiples_count(row[divisor], lo, hi)
        }
        Generated::AbsInv { .. } => 2,
        Generated::Plus { .. }
        | Generated::Times { .. }
        | Generated::Abs { .. }
        | Generated::Quotient { .. } => 1,
    }
}

fn range_len_excluding_zero(lo: i32, hi: i32) -> u64 {
    if lo > hi {
        return 0;
    }
    let mut count = (i64::from(hi) - i64::from(lo) + 1).cast_unsigned();
    if lo <= 0 && hi >= 0 {
        count = count.saturating_sub(1);
    }
    count
}

fn divisor_estimate(value: i32) -> u64 {
    let Some(abs) = value.checked_abs() else {
        return 4;
    };
    // At most four signed images of each factor up to sqrt(|value|).
    let root = u64::from(abs.cast_unsigned()).isqrt() + 1;
    root.saturating_mul(4)
}

fn fact_satisfies(step: Generated, row: &[i32], value: i32) -> bool {
    match step {
        Generated::Remainder { src, lo, hi, .. } => is_multiple_in_range(row[src], lo, hi, value),
        Generated::Divisors { src, lo, hi, .. } => {
            if value < lo || value > hi || value == 0 {
                return false;
            }
            if row[src] == 0 {
                return true;
            }
            remainder(row[src], value) == Some(0)
        }
        Generated::Plus { left, right, .. } => row[left].checked_add(row[right]) == Some(value),
        Generated::Times { left, right, .. } => row[left].checked_mul(row[right]) == Some(value),
        Generated::Abs { src, .. } => row[src].checked_abs() == Some(value),
        Generated::AbsInv { src, lo, hi, .. } => {
            let expected = row[src];
            if value < lo || value > hi || expected < 0 {
                return false;
            }
            value == expected || Some(value) == expected.checked_neg()
        }
        Generated::Quotient {
            dividend, divisor, ..
        } => super::arithmetic::quotient(row[dividend], row[divisor]) == Some(value),
        Generated::Dividend {
            quot,
            divisor,
            lo,
            hi,
            ..
        } => {
            if value < lo || value > hi {
                return false;
            }
            super::arithmetic::quotient(value, row[divisor]) == Some(row[quot])
        }
    }
}

fn collect_generated(
    step: Generated,
    row: &[i32],
    out: &mut Vec<i32>,
    stats: &mut EnumerationStats,
) {
    match step {
        Generated::Remainder { src, lo, hi, .. } => {
            stats.expression_evaluations = stats.expression_evaluations.saturating_add(1);
            multiples_in_range(row[src], lo, hi, out);
        }
        Generated::Divisors { src, lo, hi, .. } => {
            stats.expression_evaluations = stats.expression_evaluations.saturating_add(1);
            divisors_in_range(row[src], lo, hi, out);
        }
        Generated::Plus { left, right, .. } => {
            stats.expression_evaluations = stats.expression_evaluations.saturating_add(1);
            if let Some(value) = row[left].checked_add(row[right]) {
                out.push(value);
            }
        }
        Generated::Times { left, right, .. } => {
            stats.expression_evaluations = stats.expression_evaluations.saturating_add(1);
            if let Some(value) = row[left].checked_mul(row[right]) {
                out.push(value);
            }
        }
        Generated::Abs { src, .. } => {
            stats.expression_evaluations = stats.expression_evaluations.saturating_add(1);
            if let Some(value) = row[src].checked_abs() {
                out.push(value);
            }
        }
        Generated::AbsInv { src, lo, hi, .. } => {
            stats.expression_evaluations = stats.expression_evaluations.saturating_add(1);
            // `src` holds `|x|`. A negative image is unsatisfiable.
            let value = row[src];
            if value >= 0 && value >= lo && value <= hi {
                out.push(value);
            }
            if value >= 0
                && let Some(neg) = value.checked_neg()
                && neg >= lo
                && neg <= hi
                && neg != value
            {
                out.push(neg);
            }
        }
        Generated::Quotient {
            dividend, divisor, ..
        } => {
            stats.expression_evaluations = stats.expression_evaluations.saturating_add(1);
            if let Some(value) = super::arithmetic::quotient(row[dividend], row[divisor]) {
                out.push(value);
            }
        }
        Generated::Dividend {
            quot,
            divisor,
            lo,
            hi,
            ..
        } => {
            stats.expression_evaluations = stats.expression_evaluations.saturating_add(1);
            dividends_for_quotient(row[divisor], row[quot], lo, hi, out);
        }
    }
}
