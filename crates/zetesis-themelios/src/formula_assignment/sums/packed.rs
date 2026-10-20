//! One nonnegative packed interval inside the finite assignment-sum operation.
//!
//! A bit at s means that some optional subset has value s above the seed, so
//! the sum seed + s is reachable, with existence (not the number of witnesses)
//! retained. Descending shifts read the previous occurrence state, so repeated
//! weights still make separate contributions. The caller can enter once and
//! leave once; no input list or cache is retained.

#[cfg(test)]
mod tests;

use super::{Buffer, Computation, Context, FormulaFailure, FormulaResource, Numbers, Runs};
use super::{ceiling, overflow, tick};

const WORD_BITS: usize = u64::BITS as usize;

#[derive(Clone, Copy)]
pub(super) struct Plan {
    /// The largest reachable sum above the seed.
    maximum: usize,
    words: usize,
}

impl Plan {
    /// A packed route for a run that still starts at its `seed`, so every
    /// value lies at or above it.
    pub(super) fn for_runs(
        values: &Numbers,
        seed: i32,
        weight: i32,
        context: &mut Context<'_, &Computation<'_, '_>>,
    ) -> Result<Option<Self>, FormulaFailure> {
        if values.slice().first() != Some(&seed) {
            return Ok(None);
        }
        let last = *values.slice().last().expect("the seed is retained");
        let Some(plan) = Self::select(
            values.len(),
            i64::from(last) - i64::from(seed),
            seed,
            weight,
            context.work.location,
        )?
        else {
            return Ok(None);
        };
        // Entry temporarily retains the canonical run and the packed owner.
        // Conservatively keep both existing run leases in this proposal; the
        // unused spare is released before the selected allocation is attempted.
        let requested = size_of::<Buffer<u64>>() as u128 + 8 * plan.words as u128;
        Ok(fits_extra_bytes(requested, context)?.then_some(plan))
    }

    /// `maximum` is the largest sum above `seed` before adding `weight`. The
    /// largest resulting sum, `seed` plus the new maximum, must fit `i32`.
    fn select(
        count: usize,
        maximum: i64,
        seed: i32,
        weight: i32,
        location: crate::ProgramSite,
    ) -> Result<Option<Self>, FormulaFailure> {
        if weight < 0 {
            return Ok(None);
        }
        let maximum = maximum + i64::from(weight);
        if i32::try_from(i64::from(seed) + maximum).is_err() {
            return Err(overflow(location));
        }
        let maximum = usize::try_from(maximum).expect("nonnegative maximum");
        let words = (maximum + 1).div_ceil(WORD_BITS);
        let cells = count as u128;
        let word_count = words as u128;
        // Sorted merging visits M old values and K union values, then pushes K
        // outputs: at least 3M units since K >= M. Entry plus one update and
        // eventual enumeration takes at most M + 4W + K + 4 logical units,
        // excluding shared allocation-copy receipts. 4W+4 < M pays even for
        // the conversion when this is the final contribution. No lookahead.
        // Geometric word capacity is bounded by 2W requested cells; requiring
        // 16W <= 4M makes it no larger than the next merge run's minimum payload.
        if 4 * word_count + 4 >= cells || 16 * word_count > 4 * cells {
            return Ok(None);
        }
        // This is a work comparison, not an advance debit or a reason to
        // choose a more expensive fallback near the work ceiling. The selected
        // kernel charges its actual visits through the unchanged shared account.
        Ok(Some(Self { maximum, words }))
    }
}

// Read the remaining allowance only when a new allocation is proposed. Buffer
// still admits actual capacity and replacement overlap; a selected refusal is
// an error and never retries the more expensive representation.
fn fits_extra_bytes(
    requested: u128,
    context: &mut Context<'_, &Computation<'_, '_>>,
) -> Result<bool, FormulaFailure> {
    let lease = context.computation.lease();
    let available =
        context
            .computation
            .allowance(&lease, context.work.limits, context.work.location)?;
    Ok(requested <= available as u128)
}

struct Packed {
    words: Buffer<u64>,
    /// The sum every bit offset is taken from.
    seed: i32,
    maximum: usize,
    count: usize,
}

pub(super) fn finish(
    values: Numbers,
    seed: i32,
    first: i32,
    weights: impl IntoIterator<Item = i32>,
    plan: Plan,
    context: &mut Context<'_, &Computation<'_, '_>>,
) -> Result<Numbers, FormulaFailure> {
    let mut packed = Packed::from_values(values, seed, plan, context)?;
    packed.add(first, plan, context)?;
    let mut weights = weights.into_iter();
    for weight in weights.by_ref() {
        tick(context)?;
        let Some(plan) = packed.next_plan(weight, context)? else {
            let values = packed.into_numbers(context)?;
            let mut runs = Runs::from_values(values, context)?;
            // This occurrence already paid its dispatch/cancellation charge.
            runs.add(weight, context)?;
            return runs.finish(weights, context);
        };
        packed.add(weight, plan, context)?;
    }
    packed.into_numbers(context)
}

impl Packed {
    fn next_plan(
        &self,
        weight: i32,
        context: &mut Context<'_, &Computation<'_, '_>>,
    ) -> Result<Option<Plan>, FormulaFailure> {
        let maximum = i64::try_from(self.maximum).expect("admitted maximum");
        let Some(plan) = Plan::select(
            self.count,
            maximum,
            self.seed,
            weight,
            context.work.location,
        )?
        else {
            return Ok(None);
        };
        // Retained updates need no new lease or header. A replacement keeps
        // the already-accounted old words live, so only the requested new
        // payload is additional here. Growth within capacity needs no probe.
        if plan.words > self.words.capacity() && !fits_extra_bytes(8 * plan.words as u128, context)?
        {
            return Ok(None);
        }
        Ok(Some(plan))
    }

    fn from_values(
        values: Numbers,
        seed: i32,
        plan: Plan,
        context: &mut Context<'_, &Computation<'_, '_>>,
    ) -> Result<Self, FormulaFailure> {
        let above = |sum: i32| {
            usize::try_from(i64::from(sum) - i64::from(seed)).expect("sums at or above the seed")
        };
        let maximum = above(*values.slice().last().expect("the seed is retained"));
        let count = values.len();
        let mut words = Buffer::new(
            context.computation,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
        words.resize(
            plan.words,
            0,
            context.computation,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
        for &sum in values.iter() {
            tick(context)?;
            let sum = above(sum);
            words.slice_mut()[sum / WORD_BITS] |= 1 << (sum % WORD_BITS);
        }
        // The old canonical run and the new words overlap during conversion;
        // both leases remain live until this point, then the run is released.
        drop(values);
        Ok(Self {
            words,
            seed,
            maximum,
            count,
        })
    }

    fn add(
        &mut self,
        weight: i32,
        plan: Plan,
        context: &mut Context<'_, &Computation<'_, '_>>,
    ) -> Result<(), FormulaFailure> {
        let weight = usize::try_from(weight).expect("nonnegative packed contribution");
        if weight == 0 {
            return Ok(());
        }
        if plan.words > self.words.len() {
            self.words.resize(
                plan.words,
                0,
                context.computation,
                context.work.limits,
                context.work.counters,
                context.work.location,
            )?;
        }
        let old_words = (self.maximum + 1).div_ceil(WORD_BITS);
        zetesis_core::packed::union_shifted(
            self.words.slice_mut(),
            old_words,
            weight,
            &mut || tick(context),
        )?;
        let mut count = 0;
        for &word in self.words.iter() {
            tick(context)?;
            count += word.count_ones() as usize;
            let limit = context.work.limits.max_assignment_values as u128;
            // Preserve the first excess distinct-value count used by merging,
            // even when one packed word crosses several values at once.
            ceiling(
                FormulaResource::AssignmentValues,
                (count as u128).min(limit + 1),
                limit,
                context.work.location,
            )?;
        }
        self.maximum = plan.maximum;
        self.count = count;
        Ok(())
    }

    fn into_numbers(
        self,
        context: &mut Context<'_, &Computation<'_, '_>>,
    ) -> Result<Numbers, FormulaFailure> {
        let mut values = Buffer::new(
            context.computation,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
        values.reserve(
            self.count,
            context.computation,
            context.work.limits,
            context.work.counters,
            context.work.location,
        )?;
        for (index, &word) in self.words.iter().enumerate() {
            tick(context)?;
            let mut remaining = word;
            while remaining != 0 {
                let bit = remaining.trailing_zeros() as usize;
                let above = i64::try_from(index * WORD_BITS + bit).expect("an admitted offset");
                let sum = i32::try_from(i64::from(self.seed) + above)
                    .expect("every set bit is an admitted i32 sum");
                values.push(
                    sum,
                    context.computation,
                    context.work.limits,
                    context.work.counters,
                    context.work.location,
                )?;
                remaining &= remaining - 1;
            }
        }
        Ok(values)
    }
}
