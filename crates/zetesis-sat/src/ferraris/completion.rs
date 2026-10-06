//! Exact, ordered completion separated from proposal ownership and publication.

use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

use rayon::prelude::*;
use zetesis_ferraris::{Interpretation, Theory};

use super::{BatchVerdict, Limits, Statistics};
use crate::ferraris::Decision;
use crate::search::{BoundedQuota, Budget, LocalQuota, Quota, SharedBudget, increment, storage};
use crate::{PreparedReduct, ReductWorkspace};

mod scratch;
use crate::{Cancellation, Incomplete, PhaseMeasurement, SearchPhaseTimings};
pub use scratch::CompletionScratch;

/// Accounting for the last entered completion attempt, including failed jobs.
/// Worker intervals may overlap; their sums are not coordinator wall time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CompletionStatistics {
    /// Configured maximum simultaneous exact queries (one for scalar execution).
    pub workers: usize,
    /// Maximum query concurrency selected at preflight (zero without residuals).
    /// Candidate work starts only after the retained storage is also admitted.
    pub effective_workers: usize,
    /// Minimum requested conservative envelope selected before allocation.
    pub requested_scratch_bytes: u64,
    /// Shared prepared owner plus actual worker-vector capacities and
    /// conservative transient/result storage. May exceed the ceiling on a post-
    /// reservation scratch refusal; no candidate work has then started.
    /// Allocator/table control overhead, stacks and device memory are excluded.
    /// Saturates at `u64::MAX` if the envelope is unrepresentable. Such an
    /// attempt fails; an earlier reservation error retains precedence.
    pub peak_scratch_bytes: u64,
    /// Candidate slots actually entered, including supplied certificates.
    pub candidates: usize,
    /// Entered slots that requested exact native membership.
    pub residuals: usize,
    /// Slots whose completed result was stable or a validated countermodel.
    /// These results are not published unless the entire batch commits.
    pub completed: usize,
    /// Slots that returned a typed incomplete result or an invalid certificate.
    pub failed: usize,
    /// Residual slots completed locally, whether or not the batch committed.
    pub residual_completed: usize,
    /// Residual slots returning incomplete or invalid-witness results.
    pub residual_failed: usize,
    /// Coordinator elapsed time, present only when search timing was enabled.
    pub elapsed: Option<Duration>,
    /// Sum of original-validation worker intervals, when timing was enabled.
    pub worker_original_validation: Option<PhaseMeasurement>,
    /// Sum of frozen-reduct worker intervals, when timing was enabled.
    pub worker_reduct: Option<PhaseMeasurement>,
}

/// A reusable exact membership executor for original candidate batches.
///
/// One worker uses the existing scalar path without creating a thread pool.
/// Multiple workers own independent query state and share the solve's cumulative
/// work and decision ceilings. All jobs join before accounting or publication.
/// Completed output order is independent of scheduling; a resource-limited run
/// need not perform the same subset of work under different schedules.
/// Shared work permits are granted to each query in bounded groups; unused
/// permits return on every query exit and joined accounting records only work
/// actually charged. A temporarily empty pool waits for outstanding grants,
/// polling cooperative control between bounded timed waits. Decision permits
/// remain individually shared. Neither policy gives a hard response deadline.
///
/// Completion admits all ordered result slots and a conservative query envelope
/// before allocating scratch. Concurrency shrinks to fit that requested
/// envelope; actual retained capacities are checked before any candidate work.
/// Allocation slack can still refuse a batch, which remains pending.
/// Slots reserved but unused remain charged until the joined attempt returns.
/// Worker/result scratch is released between calls; the enumeration retains
/// its shared prepared owner. This is not an RSS, allocator-overhead,
/// thread-stack, original-theory, candidate-cursor or GPU-memory limit.
#[derive(Debug)]
pub struct CompletionExecutor {
    pool: Option<rayon::ThreadPool>,
    last: Option<CompletionStatistics>,
    max_scratch_bytes: u64,
}

impl Default for CompletionExecutor {
    fn default() -> Self {
        Self {
            pool: None,
            last: None,
            max_scratch_bytes: Self::DEFAULT_SCRATCH_BYTES,
        }
    }
}

impl CompletionExecutor {
    /// Default aggregate logical scratch ceiling for bounded completion batches.
    pub const DEFAULT_SCRATCH_BYTES: u64 = 256 * 1024 * 1024;

    /// Requested parametric storage estimated from the original theory.
    ///
    /// Treats every node as an implication, bounded by `reduct_admission`.
    /// Includes one shared owner, one query and the ordered batch results;
    /// allocator slack is excluded. [`PreparedReduct::scratch_requirements`]
    /// instead uses the actual compiled shape and shared owner capacity.
    ///
    /// # Errors
    /// Refuses unrepresentable shape or byte arithmetic before allocation.
    pub fn scratch_requirements(
        theory: &Theory,
        limits: Limits,
        candidates: usize,
    ) -> Result<CompletionScratch, Incomplete> {
        scratch::requirements(theory, limits, candidates)
    }

    /// The separately configured logical completion-scratch ceiling.
    #[must_use]
    pub const fn scratch_limit(&self) -> u64 {
        self.max_scratch_bytes
    }

    /// Build a dedicated pool once, or select the scalar path for one worker.
    ///
    /// # Errors
    /// Returns the pool builder's error if worker threads cannot be created.
    pub fn new(workers: NonZeroUsize) -> Result<Self, rayon::ThreadPoolBuildError> {
        Self::with_scratch_limit(workers, Self::DEFAULT_SCRATCH_BYTES)
    }

    /// Build once with a separately bounded aggregate logical scratch allowance.
    /// A zero allowance admits no nonempty completion batch.
    ///
    /// # Errors
    /// Returns the pool builder's error if worker threads cannot be created.
    pub fn with_scratch_limit(
        workers: NonZeroUsize,
        max_scratch_bytes: u64,
    ) -> Result<Self, rayon::ThreadPoolBuildError> {
        Ok(Self {
            pool: if workers.get() == 1 {
                None
            } else {
                Some(
                    rayon::ThreadPoolBuilder::new()
                        .num_threads(workers.get())
                        .thread_name(|index| format!("zetesis-reduct-{index}"))
                        .build()?,
                )
            },
            last: None,
            max_scratch_bytes,
        })
    }

    /// Maximum simultaneous exact queries; the outer candidate cursor is serial.
    #[must_use]
    pub fn workers(&self) -> usize {
        self.pool
            .as_ref()
            .map_or(1, rayon::ThreadPool::current_num_threads)
    }

    /// The last entered executor attempt. Proposal/checker failures leave it
    /// unchanged. Scratch preflight or initial allocation failure records zero
    /// entered slots. A later output reservation failure retains the completed
    /// attempt's accounting without committing its results. `None` means this
    /// executor has never been entered.
    #[must_use]
    pub const fn last_statistics(&self) -> Option<CompletionStatistics> {
        self.last
    }

    pub(super) fn complete(
        &mut self,
        input: Input<'_>,
        budget: &mut Budget<'_>,
        statistics: &mut Statistics,
        accepted: &mut Vec<bool>,
        reduct: &mut crate::prepared_reduct::State,
    ) -> Result<(), Incomplete> {
        self.complete_with(
            input,
            budget,
            statistics,
            accepted,
            reduct,
            ReductWorkspace::reserve,
        )
    }

    fn complete_with(
        &mut self,
        input: Input<'_>,
        budget: &mut Budget<'_>,
        statistics: &mut Statistics,
        accepted: &mut Vec<bool>,
        reduct: &mut crate::prepared_reduct::State,
        mut reserve: impl FnMut(
            &mut ReductWorkspace,
            &PreparedReduct,
            u64,
            &Cancellation,
        ) -> Result<(), Incomplete>,
    ) -> Result<(), Incomplete> {
        self.last = None;
        let timed = statistics.phase_timings.is_some();
        let mut progress = CompletionStatistics {
            workers: self.workers(),
            worker_original_validation: timed.then(PhaseMeasurement::default),
            worker_reduct: timed.then(PhaseMeasurement::default),
            ..CompletionStatistics::default()
        };
        let started = timed.then(Instant::now);
        let result = (|| {
            let result_slots = scratch::results(input.candidates.len())?;
            result_slots.admit(0, self.max_scratch_bytes)?;
            let residuals = input
                .verdicts
                .iter()
                .filter(|v| **v == BatchVerdict::Residual)
                .count();
            if residuals != 0 {
                reduct.ensure(input.theory, input.limits, budget, statistics)?;
            }
            let input = Input {
                prepared: reduct.prepared(),
                ..input
            };
            let requirements = input.prepared.map_or(Ok(result_slots), |owner| {
                owner.scratch_requirements(input.candidates.len())
            })?;
            progress.peak_scratch_bytes = requirements.shared_bytes;
            let admission =
                requirements.admit(self.workers().min(residuals), self.max_scratch_bytes)?;
            progress.effective_workers = admission.0;
            progress.requested_scratch_bytes = admission.1;
            progress.peak_scratch_bytes = requirements.shared_bytes + requirements.result_bytes;
            accepted
                .try_reserve_exact(input.candidates.len())
                .map_err(|_| Incomplete::Allocation)?;
            let mut workspaces = storage(admission.0)?;
            let transient = input.prepared.map_or(0, scratch::transient);
            let mut peak = u128::from(requirements.shared_bytes)
                + u128::from(requirements.result_bytes)
                + workspaces.capacity() as u128 * std::mem::size_of::<ReductWorkspace>() as u128;
            record_peak(&mut progress, peak)?;
            for _ in 0..admission.0 {
                budget.cancellation.poll()?;
                let mut workspace = ReductWorkspace::default();
                // The region query has no prepared owner to reserve against;
                // its worker workspace holds the evaluation alone.
                let reservation = match input.prepared {
                    Some(owner) => reserve(
                        &mut workspace,
                        owner,
                        input.limits.max_reduct_bytes,
                        budget.cancellation,
                    ),
                    None => Ok(()),
                };
                // Every header was counted with the outer allocation. Even a
                // failed reservation can retain newly allocated query vectors.
                peak += workspace.retained_bytes() - std::mem::size_of::<ReductWorkspace>() as u128
                    + transient;
                record_reservation(&mut progress, peak, reservation)?;
                if progress.peak_scratch_bytes > self.max_scratch_bytes {
                    return Err(Incomplete::CompletionScratch);
                }
                workspaces.push(workspace);
            }
            if let Some(pool) = &self.pool
                && !workspaces.is_empty()
            {
                parallel(
                    pool,
                    input,
                    budget,
                    statistics,
                    accepted,
                    &mut progress,
                    &mut workspaces,
                )
            } else {
                scalar(
                    input,
                    budget,
                    statistics,
                    accepted,
                    &mut progress,
                    &mut workspaces,
                )
            }
        })();
        progress.elapsed = started.map(|start| start.elapsed());
        self.last = Some(progress);
        result
    }
}

fn record_peak(progress: &mut CompletionStatistics, peak: u128) -> Result<(), Incomplete> {
    let measured = u64::try_from(peak).map_err(|_| Incomplete::CounterOverflow);
    progress.peak_scratch_bytes = measured.unwrap_or(u64::MAX);
    measured.map(|_| ())
}

fn record_reservation(
    progress: &mut CompletionStatistics,
    peak: u128,
    reservation: Result<(), Incomplete>,
) -> Result<(), Incomplete> {
    let measured = record_peak(progress, peak);
    reservation?;
    measured
}

/// Borrows the immutable batch owner for the lifetime of a joined execution.
/// No candidate restriction, cursor, objective state or publication sink enters
/// a worker; only the original Theory and its matching interpretation do.
#[derive(Clone, Copy)]
pub(super) struct Input<'a> {
    pub(super) theory: &'a Theory,
    pub(super) candidates: &'a [Interpretation],
    pub(super) verdicts: &'a [BatchVerdict],
    pub(super) limits: Limits,
    /// The prepared reduct encoding, under the clause kernel.
    pub(super) prepared: Option<&'a PreparedReduct>,
    /// The region query, under the regions method, over the original index
    /// its owner lends; completion never builds one.
    pub(super) query: Option<super::ReductQuery<'a>>,
}

struct Outcome {
    result: Result<bool, Incomplete>,
    statistics: Statistics,
}

fn classify(
    input: Input<'_>,
    candidate: &Interpretation,
    verdict: BatchVerdict,
    budget: &mut Budget<'_, impl Quota>,
    statistics: &mut Statistics,
    workspace: &mut ReductWorkspace,
) -> Result<bool, Incomplete> {
    budget.cancellation.poll()?;
    match verdict {
        BatchVerdict::NoProperSubset => Ok(true),
        BatchVerdict::Refuted => Ok(false),
        BatchVerdict::NotModel => Err(Incomplete::InvalidWitness),
        BatchVerdict::Residual => {
            let verdict = match (input.prepared, input.query) {
                (Some(prepared), _) => {
                    prepared.check_with(candidate, workspace, input.limits, budget, statistics)?
                }
                (None, Some(query)) => {
                    let result = (|| {
                        let (truth, scratch) = workspace.evaluate(
                            candidate,
                            input.limits,
                            budget.cancellation,
                            statistics,
                        )?;
                        if !truth.is_model() {
                            return Err(Incomplete::InvalidWitness);
                        }
                        increment(&mut statistics.countermodel_queries)?;
                        query.check(
                            zetesis_ferraris::FrozenSubject::new(input.theory, truth.truth()),
                            candidate,
                            input.limits,
                            budget,
                            statistics,
                            scratch,
                        )
                    })();
                    workspace.finish_check(result, input.limits.max_reduct_bytes, statistics)?
                }
                (None, None) => return Err(Incomplete::InvalidWitness),
            };
            match Decision::from(verdict) {
                Decision::Stable => Ok(true),
                Decision::Refuted => Ok(false),
                Decision::Invalid => Err(Incomplete::InvalidWitness),
            }
        }
    }
}

fn scalar(
    input: Input<'_>,
    budget: &mut Budget<'_>,
    statistics: &mut Statistics,
    accepted: &mut Vec<bool>,
    progress: &mut CompletionStatistics,
    workspaces: &mut [ReductWorkspace],
) -> Result<(), Incomplete> {
    progress.worker_original_validation = None;
    progress.worker_reduct = None;
    let mut bounded = Budget {
        quota: BoundedQuota(LocalQuota),
        limits: budget.limits,
        cancellation: budget.cancellation,
        statistics: budget.statistics,
    };
    let mut unused = ReductWorkspace::default();
    let workspace = workspaces.first_mut().unwrap_or(&mut unused);
    let result = (|| {
        for (candidate, &verdict) in input.candidates.iter().zip(input.verdicts) {
            // Retain the existing scalar counters, work order and failure boundary.
            let result = classify(
                input,
                candidate,
                verdict,
                &mut bounded,
                statistics,
                workspace,
            );
            entered(progress, verdict, &result);
            accepted.push(result?);
        }
        Ok(())
    })();
    budget.statistics = bounded.statistics;
    result
}

fn parallel(
    pool: &rayon::ThreadPool,
    input: Input<'_>,
    budget: &mut Budget<'_>,
    statistics: &mut Statistics,
    accepted: &mut Vec<bool>,
    progress: &mut CompletionStatistics,
    workspaces: &mut [ReductWorkspace],
) -> Result<(), Incomplete> {
    let mut outcomes = storage(input.candidates.len())?;
    outcomes.resize_with(input.candidates.len(), || None);
    let shared = SharedBudget::new(budget.limits, budget.statistics);
    let cancellation = budget.cancellation;
    let timed = statistics.phase_timings.is_some();
    let chunk_size = outcomes.len().div_ceil(progress.effective_workers.max(1));
    pool.install(|| {
        outcomes
            .par_chunks_mut(chunk_size)
            .zip(workspaces.par_iter_mut())
            .enumerate()
            .for_each(|(chunk, (slots, workspace))| {
                for (offset, slot) in slots.iter_mut().enumerate() {
                    *slot = Some(run(
                        input,
                        chunk * chunk_size + offset,
                        &shared,
                        cancellation,
                        timed,
                        workspace,
                    ));
                }
            });
    });
    // Work is reserved before the operation. Read after join, including every
    // failed job, before choosing an error in original candidate order.
    shared.record(&mut budget.statistics);
    let mut failure = None;
    for (outcome, &verdict) in outcomes.into_iter().zip(input.verdicts) {
        let outcome = outcome.expect("joined indexed execution fills every owned slot");
        entered(progress, verdict, &outcome.result);
        let merged = merge(statistics, budget, &outcome.statistics);
        merge_timing(progress, outcome.statistics.phase_timings);
        if failure.is_none() {
            failure = merged
                .err()
                .or_else(|| outcome.result.as_ref().err().copied());
        }
        if let Ok(value) = outcome.result {
            accepted.push(value);
        }
    }
    failure.map_or(Ok(()), Err)
}

fn run(
    input: Input<'_>,
    index: usize,
    shared: &SharedBudget,
    cancellation: &Cancellation,
    timed: bool,
    workspace: &mut ReductWorkspace,
) -> Outcome {
    let mut budget = Budget {
        quota: BoundedQuota(shared.lease(cancellation)),
        limits: input.limits.search,
        cancellation,
        statistics: crate::SearchStatistics::default(),
    };
    let mut statistics = Statistics {
        phase_timings: timed.then(SearchPhaseTimings::default),
        ..Statistics::default()
    };
    let result = classify(
        input,
        &input.candidates[index],
        input.verdicts[index],
        &mut budget,
        &mut statistics,
        workspace,
    );
    statistics.search = budget.statistics;
    Outcome { result, statistics }
}

fn entered(
    progress: &mut CompletionStatistics,
    verdict: BatchVerdict,
    result: &Result<bool, Incomplete>,
) {
    progress.candidates += 1;
    progress.residuals += usize::from(verdict == BatchVerdict::Residual);
    progress.completed += usize::from(result.is_ok());
    progress.failed += usize::from(result.is_err());
    progress.residual_completed += usize::from(verdict == BatchVerdict::Residual && result.is_ok());
    progress.residual_failed += usize::from(verdict == BatchVerdict::Residual && result.is_err());
}

fn merge(
    statistics: &mut Statistics,
    budget: &mut Budget<'_>,
    worker: &Statistics,
) -> Result<(), Incomplete> {
    add(
        &mut statistics.countermodel_queries,
        worker.countermodel_queries,
    )?;
    add(&mut statistics.countermodels, worker.countermodels)?;
    add(
        &mut budget.statistics.propagations,
        worker.search.propagations,
    )?;
    add(&mut budget.statistics.conflicts, worker.search.conflicts)?;
    add(
        &mut statistics.reduct.original_work,
        worker.reduct.original_work,
    )?;
    add(
        &mut statistics.reduct.parameter_work,
        worker.reduct.parameter_work,
    )?;
    statistics.reduct.regions.add(worker.reduct.regions)?;
    statistics.reduct.peak_workspace_bytes = statistics
        .reduct
        .peak_workspace_bytes
        .max(worker.reduct.peak_workspace_bytes);
    Ok(())
}

fn add(counter: &mut u64, value: u64) -> Result<(), Incomplete> {
    *counter = counter
        .checked_add(value)
        .ok_or(Incomplete::CounterOverflow)?;
    Ok(())
}

fn merge_timing(progress: &mut CompletionStatistics, worker: Option<SearchPhaseTimings>) {
    if let Some(worker) = worker {
        for (target, source) in [
            (
                &mut progress.worker_original_validation,
                worker.original_validation,
            ),
            (&mut progress.worker_reduct, worker.reduct),
        ] {
            if let Some(target) = target {
                match (
                    target.calls.checked_add(source.calls),
                    target.elapsed.checked_add(source.elapsed),
                ) {
                    (Some(calls), Some(elapsed)) if !target.overflowed && !source.overflowed => {
                        target.calls = calls;
                        target.elapsed = elapsed;
                    }
                    _ => target.overflowed = true,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;

impl PreparedReduct {
    /// Exact shared owner capacity plus requested disjoint query/result slots.
    /// Uses the actual compiled CNF shape, preserving gate-sharing reductions.
    /// Actual worker capacities are still checked after reservation.
    ///
    /// # Errors
    /// Refuses unrepresentable byte arithmetic before allocating result slots.
    pub fn scratch_requirements(&self, candidates: usize) -> Result<CompletionScratch, Incomplete> {
        scratch::prepared(self, candidates)
    }
}
