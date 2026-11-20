//! One ordered task boundary shares existing source accounts and publication.
//!
//! Worker allowances partition the parent's remaining resources. The shared
//! catalog is counted once; retained worker output owns a conservative fraction
//! of the remaining byte allowance until ordered publication has finished.

use std::ops::Range;

use super::{Computation, Context, Counters, GroundingWork, StorageLease, Support};
use super::{cursor_rows::CursorRows, witness_rows};
use crate::formula_ir::RuleIr;
use crate::formula_support::accounting::StopPrecedence;
use crate::formula_support::parallel_executor::{Capture, Executor, Outcome, TASKS, settle};
use crate::formula_support::{Accounting, Buffer, Join};
use crate::grounding_observer::Work;
use crate::{ExpansionLimits, ExpansionUsage, FormulaFailure, FormulaLimits, FormulaResource};
use crate::{ProgramSite, expansion::Budget};

struct Block {
    rows: Range<usize>,
    formula: FormulaLimits,
    expansion: ExpansionLimits,
}

#[derive(Default)]
struct Receipt {
    work: u64,
    substitutions: u64,
    expansion: ExpansionUsage,
    observed: crate::GroundingWork,
}

/// Every receipt is already settled. The reservation stays live while consumers
/// inspect or move successful values; it does not claim measured concurrent RSS.
pub(crate) struct Completed<T> {
    slots: Buffer<Option<Capture<T, FormulaFailure, Receipt>>>,
    reserved: usize,
    lease: StorageLease,
}

impl<T> Completed<T> {
    pub(crate) fn results(&self) -> impl Iterator<Item = &T> {
        self.slots
            .iter()
            .map(|slot| match slot.as_ref().expect("joined slot").outcome() {
                Outcome::Returned(Ok(value)) => value,
                _ => unreachable!("all outcomes checked before completion"),
            })
    }

    /// Move the entire conservative worker reservation into this parent's
    /// existing lease. Consumers with immutable stored limits can then publish
    /// against the unchanged ceiling. No reservation is released early and no
    /// allocation or measured physical peak is claimed by this operation.
    pub(crate) fn attach_reservation(
        &mut self,
        context: Context<'_, &Computation<'_, '_>>,
    ) -> Result<(), FormulaFailure> {
        let Context {
            computation,
            work:
                GroundingWork {
                    limits,
                    counters,
                    location,
                },
        } = context;
        counters.work(limits, location)?;
        let retained =
            zetesis_algebra::Meter::check_count(self.lease.bytes(), self.reserved, usize::MAX)
                .map_err(|_| FormulaFailure::Limit {
                    resource: FormulaResource::SupportBytes,
                    observed: self.lease.bytes() as u128 + self.reserved as u128,
                    limit: usize::MAX as u128,
                    location,
                })?;
        self.lease.reserve(retained, location)?;
        // This checks admission only, deliberately not storage_observed.
        computation.allowance(&self.lease, limits, location)?;
        self.reserved = 0;
        Ok(())
    }

    pub(crate) fn results_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.slots.slice_mut().iter_mut().map(|slot| {
            match slot.as_mut().expect("joined slot").outcome_mut() {
                Outcome::Returned(Ok(value)) => value,
                _ => unreachable!("all outcomes checked before completion"),
            }
        })
    }

    pub(crate) fn publication_limits(&self, limits: &FormulaLimits) -> FormulaLimits {
        FormulaLimits {
            max_support_bytes: limits.max_support_bytes - self.reserved,
            ..*limits
        }
    }
}

/// The callback captures only immutable source descriptors. Fresh accounting and
/// observation banks are constructed inside each worker, never sent as `Rc`s.
pub(crate) fn run<T: Send>(
    executor: &Executor,
    width: usize,
    budget: &mut Budget,
    context: Context<'_, &Computation<'_, '_>>,
    kernel: impl Fn(
        Range<usize>,
        &FormulaLimits,
        &mut Budget,
        &mut Counters,
    ) -> Result<T, FormulaFailure>
    + Sync,
) -> Result<Completed<T>, FormulaFailure> {
    run_grouped(executor, width, TASKS, budget, context, kernel)
}

pub(crate) fn run_grouped<T: Send>(
    executor: &Executor,
    width: usize,
    groups: usize,
    budget: &mut Budget,
    context: Context<'_, &Computation<'_, '_>>,
    kernel: impl Fn(
        Range<usize>,
        &FormulaLimits,
        &mut Budget,
        &mut Counters,
    ) -> Result<T, FormulaFailure>
    + Sync,
) -> Result<Completed<T>, FormulaFailure> {
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location,
            },
    } = context;
    let mut completed = completed(computation, limits, counters, location)?;
    let mut blocks = blocks(width, groups, computation, limits, counters, location)?;
    for _ in blocks.iter() {
        completed
            .slots
            .push(None, computation, limits, counters, location)?;
    }
    completed.reserved = grants(
        &mut blocks,
        budget,
        Context::new(computation, limits, counters, location),
    )?;
    let cancel = counters.cancellation().cloned();
    let scalar_cancel = budget.cancellation().cloned();
    let observed = counters.observed.enabled();
    let execute_block = |block: &Block| {
        execute(
            block,
            cancel.as_ref(),
            scalar_cancel.clone(),
            observed,
            location,
            &kernel,
        )
    };
    if groups > TASKS {
        executor.run_prime(blocks.slice(), completed.slots.slice_mut(), execute_block);
    } else {
        executor.run(blocks.slice(), completed.slots.slice_mut(), execute_block);
    }
    let repeated = partitioned_refusals(&completed);
    // Even an interrupted settlement visits all receipts; no completed task's
    // work or scalar traffic can disappear behind an earlier stop.
    let settlement = settle(completed.slots.slice_mut(), |receipt| {
        absorb(receipt, budget, limits, counters, location)
    });
    if settlement.is_ok() {
        repeat(
            &mut completed,
            &blocks,
            repeated,
            budget,
            limits,
            counters,
            &kernel,
        );
    }
    arbitrate(&mut completed, settlement, location)?;
    Ok(completed)
}

/// A worker refuses against its partition of the remaining resources, not the
/// source's ceiling. The leading such refusals, those before any panic or
/// other failure that takes precedence by order, are repeated once every
/// receipt, theirs included, has settled.
fn partitioned_refusals<T>(completed: &Completed<T>) -> Range<usize> {
    fn outcome<T>(
        slot: Option<&Capture<T, FormulaFailure, Receipt>>,
    ) -> &Outcome<T, FormulaFailure> {
        slot.expect("joined slot").pending_outcome()
    }
    let slots = completed.slots.slice();
    let Some(first) = slots
        .iter()
        .position(|slot| !matches!(outcome(slot.as_ref()), Outcome::Returned(Ok(_))))
    else {
        return 0..0;
    };
    let end = slots[first..]
        .iter()
        .position(|slot| {
            !matches!(outcome(slot.as_ref()), Outcome::Returned(Err(error)) if error.is_resource_refusal())
        })
        .map_or(slots.len(), |offset| first + offset);
    first..end
}

/// Repeat partition-refused blocks in order on the source's own accounts, so a
/// refusal reports the source's ceiling and its cumulative demand, and a source
/// within its ceiling is never refused by how the remainder was partitioned.
/// The refused attempt's actual work stays charged. The first repeated failure
/// ends the repetition; arbitration reports it by order.
fn repeat<T>(
    completed: &mut Completed<T>,
    blocks: &Buffer<Block>,
    repeated: Range<usize>,
    budget: &mut Budget,
    limits: &FormulaLimits,
    counters: &mut Counters,
    kernel: &impl Fn(
        Range<usize>,
        &FormulaLimits,
        &mut Budget,
        &mut Counters,
    ) -> Result<T, FormulaFailure>,
) {
    let count = blocks.len();
    let retained = completed.reserved;
    for index in repeated {
        let block = &blocks.slice()[index];
        let extra = retained / count + usize::from(index < retained % count);
        // The block keeps its shared catalog bytes and may use the whole
        // retained allowance; the other half of the remainder still bounds
        // what earlier blocks retain.
        let formula = FormulaLimits {
            max_work: limits.max_work,
            max_substitutions: limits.max_substitutions,
            max_support_bytes: block.formula.max_support_bytes - extra + retained,
            ..*limits
        };
        let mut state = (&mut *budget, &mut *counters);
        let capture = Capture::run(
            &mut state,
            |(budget, counters)| kernel(block.rows.clone(), &formula, budget, counters),
            |_| Receipt::default(),
        );
        let slot = &mut completed.slots.slice_mut()[index];
        *slot = Some(capture);
        let settled = settle(std::slice::from_mut(slot), |_| Ok::<(), FormulaFailure>(()));
        debug_assert!(settled.is_ok());
        if !matches!(
            slot.as_ref().expect("repeated slot").outcome(),
            Outcome::Returned(Ok(_))
        ) {
            return;
        }
    }
}

fn completed<T>(
    computation: &Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<Completed<T>, FormulaFailure> {
    let mut lease = computation.lease();
    let header = size_of::<Completed<T>>()
        - size_of::<Buffer<Option<Capture<T, FormulaFailure, Receipt>>>>();
    lease.observe(header, location)?;
    computation.storage_observed(&lease, 0, header, limits, counters, location)?;
    Ok(Completed {
        slots: Buffer::new(computation, limits, counters, location)?,
        reserved: 0,
        lease,
    })
}

fn blocks(
    width: usize,
    groups: usize,
    computation: &Computation<'_, '_>,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<Buffer<Block>, FormulaFailure> {
    let count = super::prime::group_count(width, groups);
    let mut blocks = Buffer::new(computation, limits, counters, location)?;
    blocks.reserve(count, computation, limits, counters, location)?;
    for index in 0..count {
        let rows = super::prime::group_range(width, groups, index);
        blocks.push(
            Block {
                rows,
                formula: *limits,
                expansion: ExpansionLimits::default(),
            },
            computation,
            limits,
            counters,
            location,
        )?;
    }
    debug_assert_eq!(super::prime::group_start(width, groups, count), width);
    Ok(blocks)
}

fn grants(
    blocks: &mut Buffer<Block>,
    budget: &Budget,
    context: Context<'_, &Computation<'_, '_>>,
) -> Result<usize, FormulaFailure> {
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location,
            },
    } = context;
    let count = blocks.len();
    if count == 0 {
        return Ok(0);
    }
    // Charge the actual fixed descriptor updates before dividing work, so the
    // parent does not consume any worker's already-issued allowance afterward.
    counters.charge_work(count as u128, limits, location)?;
    let live = computation.query_support().live_bytes();
    crate::formula::ceiling(
        FormulaResource::SupportBytes,
        live as u128,
        limits.max_support_bytes as u128,
        location,
    )?;
    let retained = (limits.max_support_bytes - live) / 2;
    let shared = computation.query_support().relations().current_bytes();
    let work = limits.max_work - counters.accounting.work;
    crate::formula::ceiling(
        FormulaResource::Substitutions,
        u128::from(counters.accounting.substitutions),
        u128::from(limits.max_substitutions),
        location,
    )?;
    let substitutions = limits.max_substitutions - counters.accounting.substitutions;
    let share = |value: u64, index: usize| {
        value / count as u64
            + u64::from(
                index
                    < usize::try_from(value % count as u64)
                        .expect("remainder is less than task count"),
            )
    };
    for (index, block) in blocks.slice_mut().iter_mut().enumerate() {
        let extra = retained / count + usize::from(index < retained % count);
        block.formula = FormulaLimits {
            max_work: share(work, index),
            max_substitutions: share(substitutions, index),
            max_support_bytes: shared + extra,
            ..*limits
        };
        block.expansion = budget.worker_limits(count, index);
    }
    Ok(retained)
}

fn execute<T>(
    block: &Block,
    cancellation: Option<&zetesis_algebra::Cancellation>,
    scalar_cancel: Option<zetesis_algebra::Cancellation>,
    observed: bool,
    location: ProgramSite,
    kernel: &impl Fn(
        Range<usize>,
        &FormulaLimits,
        &mut Budget,
        &mut Counters,
    ) -> Result<T, FormulaFailure>,
) -> Capture<T, FormulaFailure, Receipt> {
    let mut state = (
        Accounting::default(),
        Budget::new(block.expansion, usize::MAX).with_cancellation(scalar_cancel),
        Work::worker(observed),
    );
    Capture::run(
        &mut state,
        |(accounting, budget, observed)| {
            accounting.controlled(cancellation, observed.clone(), location, |counters| {
                kernel(block.rows.clone(), &block.formula, budget, counters)
            })
        },
        |(accounting, budget, observed)| Receipt {
            work: accounting.work,
            substitutions: accounting.substitutions,
            expansion: budget.usage(),
            observed: observed.snapshot(),
        },
    )
}

fn absorb(
    mut receipt: Receipt,
    budget: &mut Budget,
    limits: &FormulaLimits,
    counters: &mut Counters,
    location: ProgramSite,
) -> Result<(), FormulaFailure> {
    let work = counters.absorb(u128::from(receipt.work), limits, location);
    // This uses the existing substitution owner, including its shared receipt.
    let substitutions = counters.absorb_substitutions(receipt.substitutions, limits, location);
    let expansion = budget
        .absorb(receipt.expansion, location)
        .map_err(FormulaFailure::from);
    // Worker-local maxima do not establish a simultaneous global peak. The
    // separate retained allowance bounds admission; it is not an observation.
    receipt.observed.support_peak_bytes = None;
    counters.observed.merge(&receipt.observed);
    work.and(substitutions).and(expansion)
}

fn arbitrate<T>(
    completed: &mut Completed<T>,
    settlement: Result<(), FormulaFailure>,
    location: ProgramSite,
) -> Result<(), FormulaFailure> {
    // Panics are never source conclusions. All receipts have already settled.
    if let Some(index) = completed.slots.iter().position(|slot| {
        matches!(
            slot.as_ref().expect("joined slot").outcome(),
            Outcome::Panicked(_)
        )
    }) {
        let capture = completed.slots.slice_mut()[index]
            .take()
            .expect("joined slot");
        let Outcome::Panicked(payload) = capture.into_outcome() else {
            unreachable!()
        };
        std::panic::resume_unwind(payload);
    }
    if let Some(index) = completed.slots.iter().position(|slot| {
        matches!(
            slot.as_ref().expect("joined slot").outcome(),
            Outcome::Returned(Err(_))
        )
    }) {
        let capture = completed.slots.slice_mut()[index]
            .take()
            .expect("joined slot");
        let Outcome::Returned(Err(error)) = capture.into_outcome() else {
            unreachable!()
        };
        return Err(
            match settlement.err().and_then(|error| error.interruption()) {
                Some(stop) if error.is_resource_refusal() => error.outranked_by(stop, location),
                _ => error,
            },
        );
    }
    settlement
}

/// Preserve the actual cursor and ordinary projection owner. Only a completely
/// admitted positive-flat row stream enters this task boundary; a decline keeps
/// the original cursor available to the serial implementation.
pub(in crate::formula_support) fn derive<'source>(
    executor: &Executor,
    (index, rule): (usize, &'source RuleIr),
    outer: &mut Join<'_, 'source>,
    support: &Support<'source>,
    budget: &mut Budget,
    context: Context<'_, &mut Computation<'_, 'source>>,
) -> Result<bool, FormulaFailure> {
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location,
            },
    } = context;
    if computation.pending_reader().is_none() {
        return Ok(false);
    }
    let Some(rows) = CursorRows::prepare(rule, outer, computation, limits, counters)? else {
        return Ok(false);
    };
    let plan = witness_rows::WitnessPlan::new(
        rows.descriptor(),
        support.relations(),
        support,
        computation,
        limits,
        counters,
    )?;
    if plan.prime() {
        let remaining = limits
            .max_support_bytes
            .saturating_sub(computation.query_support().live_bytes());
        // Until residual replay records integer columns, reserve only packed
        // occurrence coordinates. I16 is the default integer width when a
        // later column is added with a fitting range.
        if super::prime::admit_reservation(
            plan.len(),
            plan.pattern_count(),
            plan.max_row_index(),
            0,
            0,
            0,
            remaining,
        )
        .is_none()
        {
            return Ok(false);
        }
    }
    let completed = stage_witnesses(
        executor,
        &plan,
        budget,
        Context::new(&*computation, limits, counters, location),
    )?;
    let publication = completed.publication_limits(limits);
    witness_rows::publish(
        &plan,
        completed.results(),
        support,
        computation,
        &publication,
        counters,
        index,
    )?;
    Ok(true)
}

/// The pending-read borrow ends after every task joins, before any consumer can
/// publish through the original writer. This path stages occurrences only.
pub(crate) fn stage_witnesses<'plan, 'source: 'plan>(
    executor: &Executor,
    plan: &'plan witness_rows::WitnessPlan<'plan, 'source>,
    budget: &mut Budget,
    context: Context<'_, &Computation<'_, 'source>>,
) -> Result<Completed<witness_rows::Batch<'plan, 'source>>, FormulaFailure> {
    let Context {
        computation,
        work:
            GroundingWork {
                limits,
                counters,
                location,
            },
    } = context;
    let append = computation
        .pending_reader()
        .ok_or(FormulaFailure::SupportRelation {
            error: zetesis_core::relation::Failure::Owner,
            location,
        })?;
    run_grouped(
        executor,
        plan.len(),
        plan.groups(),
        budget,
        Context::new(computation, limits, counters, location),
        |block, limits, budget, counters| {
            witness_rows::execute_borrowed(plan, block, append, limits, budget, counters)
        },
    )
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod reservation_tests;
