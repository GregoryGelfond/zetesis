//! Bounded attainable-sum refutation after ordinary and lattice propagation.
//!
//! Each undecided grouped condition is independently optional in this
//! relaxation. Its subset sums overapproximate the sums of compatible models,
//! even when conditions are correlated. An empty intersection with the required
//! interval refutes the region; a nonempty intersection establishes nothing.
//!
//! Only offsets below the exclusive upper bound are queried. Positive
//! contributions cannot bring an omitted larger sum or unused tail bit back
//! into that interval.
//! Ascending contributions stop as soon as one retained sum meets the interval:
//! the remaining optional contributions cannot remove that witness.
//! The shifted bits supply that witness directly, without rereading the mask.
//! A witness may leave a partial shift; the whole scratch is then discarded.
//!
//! This is optional pruning, bounded by the existing grouped row's payload.
//! Its requested mask uses no more bytes than that payload, and the attempt gets
//! one logical visit per u64 word of the same payload. Local exhaustion declines
//! the optimization; every actual visit also consumes ordinary narrowing work.
//! Allocation, global work refusal, cancellation and deadlines remain typed
//! failures. No condition-dependent result or proof is retained between calls.

use zetesis_algebra::{Failure, Quota};

use crate::regions::{Closure, Count, Meter, Narrower, Step, bit};
use zetesis_algebra::{Stop, words::Words};

use super::{Row, State};

const WORD_BITS: usize = u64::BITS as usize;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
struct Plan {
    lower: usize,
    maximum: usize,
    words: usize,
    visits: usize,
}

impl Plan {
    /// A representation choice, not an advance debit of unvisited work.
    fn for_row(row: &Row, state: State) -> Option<Self> {
        if !row.has_nonunit_coefficient() || !state.has_residual_interval() {
            return None;
        }
        let lower = usize::try_from(state.lower.checked_sub(state.known)?).ok()?;
        let maximum =
            usize::try_from(state.upper?.checked_sub(state.known)?.checked_sub(1)?).ok()?;
        let words = (maximum / WORD_BITS).checked_add(1)?;
        let bytes = row.terms.len().checked_mul(size_of::<(usize, i128)>())?;
        let visits = bytes / size_of::<u64>();
        // Keep the conservative admission margin beyond zeroing: allocation,
        // the seed, one coefficient, one shifted source word and a spare visit.
        // This is not an advance charge; only actual visits consume the budget.
        (words.checked_add(5)? <= visits).then_some(Self {
            lower,
            maximum,
            words,
            visits,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
enum AttemptFailure<E> {
    Declined,
    Stopped(Failure<E>),
}

impl<E> From<Stop> for AttemptFailure<E> {
    fn from(stop: Stop) -> Self {
        Self::Stopped(Failure::Stopped(stop))
    }
}

impl<E> From<Failure<E>> for AttemptFailure<E> {
    fn from(failure: Failure<E>) -> Self {
        Self::Stopped(failure)
    }
}

struct Allowance<'a, 'q, Q: Quota + ?Sized> {
    remaining: u64,
    work: &'a mut Meter<'q, Q>,
}

impl<Q: Quota + ?Sized> Allowance<'_, '_, Q> {
    fn tick(&mut self) -> Result<(), AttemptFailure<Q::Error>> {
        if self.work.tick_within(&mut self.remaining)? {
            Ok(())
        } else {
            Err(AttemptFailure::Declined)
        }
    }
}

impl<W: Count, C: crate::regions::ReadingClass> Closure<'_, W, C> {
    /// Called only when the preceding residual check left all queues drained.
    /// No deductions are published by this operation.
    pub(in crate::regions) fn threshold_possible_sums<Q: Quota + ?Sized>(
        &mut self,
        index: &Narrower,
        row: usize,
        work: &mut Meter<'_, Q>,
    ) -> Result<Step, Failure<Q::Error>> {
        let data = &index.thresholds.rows[row];
        let Some(plan) = Plan::for_row(data, self.known.thresholds[row]) else {
            return Ok(Step::Unchanged);
        };
        debug_assert!(
            self.lists.nodes.is_empty()
                && self.lists.threshold_rows.is_empty()
                && self.lists.heads.is_empty(),
            "possible sums require drained mandatory events"
        );
        let result = attempt(
            data,
            &self.known.sure,
            &self.known.never,
            &mut self.lists.possible_sums,
            plan,
            &mut Allowance {
                remaining: plan.visits as u64,
                work,
            },
        );
        self.lists.possible_sums.storage_mut().clear();
        match result {
            Ok(step) => Ok(step),
            Err(AttemptFailure::Declined) => Ok(Step::Unchanged),
            Err(AttemptFailure::Stopped(stop)) => Err(stop),
        }
    }
}

fn attempt<Q: Quota + ?Sized>(
    row: &Row,
    sure: &[u64],
    never: &[u64],
    words: &mut Words<u64>,
    plan: Plan,
    allowance: &mut Allowance<'_, '_, Q>,
) -> Result<Step, AttemptFailure<Q::Error>> {
    prepare(words, plan, allowance)?;
    let mut maximum = 0;
    for term in row.terms.iter().rev() {
        allowance.tick()?;
        let (condition, weight) = *term;
        // The remaining coefficients are at least this large. Their positive
        // shifts cannot add a sum inside the retained interval.
        if weight > plan.maximum as i128 {
            break;
        }
        if bit(sure, condition) || bit(never, condition) {
            continue;
        }
        let weight = usize::try_from(weight).map_err(|_| Stop::InvalidProgram)?;
        let source_words = maximum / WORD_BITS + 1;
        // Zero is below lower, and each earlier shift found no interval bit.
        // Only this translated copy can supply the first witness. It merely
        // declines optional pruning, never proves a compatible model. A hit
        // may leave the shift incomplete; no later code consumes that scratch.
        if zetesis_algebra::words::union_shifted_until_hit(
            words,
            source_words,
            weight,
            plan.lower..=plan.maximum,
            &mut || allowance.tick(),
        )? {
            return Ok(Step::Unchanged);
        }
        maximum = maximum.saturating_add(weight).min(plan.maximum);
    }
    // Every open contribution fitting the interval was considered. No allowed
    // bit appeared, so even the independent-condition relaxation has no model.
    Ok(Step::Contradiction)
}

fn prepare<Q: Quota + ?Sized>(
    words: &mut Words<u64>,
    plan: Plan,
    allowance: &mut Allowance<'_, '_, Q>,
) -> Result<(), AttemptFailure<Q::Error>> {
    words.storage_mut().clear();
    if words.capacity() < plan.words {
        allowance.tick()?;
        // These values have no authority outside one attempt. Releasing an
        // undersized allocation first avoids old/replacement payload overlap.
        *words = Words::new(Vec::new());
        words
            .storage_mut()
            .try_reserve_exact(plan.words)
            .map_err(|_| Stop::Allocation)?;
        // Admission uses actual capacity, including allocator over-allocation.
        if words.capacity() > plan.visits {
            *words = Words::new(Vec::new());
            return Err(AttemptFailure::Declined);
        }
    }
    for _ in 0..plan.words {
        allowance.tick()?;
        words.storage_mut().push(0);
    }
    allowance.tick()?;
    words[0] = 1;
    Ok(())
}
