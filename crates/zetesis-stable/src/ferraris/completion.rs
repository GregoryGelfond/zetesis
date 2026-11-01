//! Exact, ordered completion separated from proposal ownership and publication.

use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

use rayon::prelude::*;
use zetesis_algebra::region::Region;
use zetesis_ferraris::Interpretation;

use super::{BatchVerdict, Limits, Statistics};
use crate::budget::{Budget, LocalQuota, Quota, SharedBudget, increment, storage};
use crate::ferraris::Decision;
use crate::reduct::ReductWorkspace;
use crate::timing::{self, Phase};

mod scratch;
use crate::{Cancellation, Incomplete, PhaseMeasurement, SearchPhaseTimings};

/// Accounting for the last entered completion attempt, including failed jobs.
/// Worker intervals may overlap; their sums are not coordinator wall time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CompletionStatistics {
    /// Configured maximum simultaneous exact queries (one for scalar execution).
    pub workers: usize,
    /// Maximum query concurrency selected at preflight (zero without residuals).
    /// Candidate work starts only after the retained storage is also admitted.
    pub effective_workers: usize,
    /// Storage requested before allocation: the ordered results. Query
    /// workspaces are measured as they are created.
    pub requested_scratch_bytes: u64,
    /// Actual worker-workspace capacities and the ordered result storage.
    /// May exceed the ceiling on a scratch refusal; no candidate work has then
    /// started.
    /// Allocator/table control overhead, stacks and device memory are excluded.
    /// Saturates at `u64::MAX` if the envelope is unrepresentable; such an
    /// attempt fails.
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
/// Completion admits all ordered result slots before allocating scratch, and
/// each worker workspace as it is created; actual retained capacities are
/// checked before any candidate work, and each query's workspace is bounded by
/// `Limits::max_reduct_bytes`. Allocation slack can still refuse a batch, which
/// remains pending. Slots reserved but unused remain charged until the joined
/// attempt returns. Worker/result scratch is released between calls. This is
/// not an RSS, allocator-overhead, thread-stack, original-theory or GPU-memory
/// limit.
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
    pub(crate) const DEFAULT_SCRATCH_BYTES: u64 = 256 * 1024 * 1024;

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

    /// Maximum simultaneous exact queries; candidate proposal stays serial.
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
    ) -> Result<(), Incomplete> {
        self.execute(input, None, budget, statistics, accepted)
    }

    /// A regional result bit records absence in its supplied domain. Only
    /// the later regional pass can consume that result for publication.
    pub(super) fn execute(
        &mut self,
        input: Input<'_>,
        roots: Option<Vec<(usize, Region)>>,
        budget: &mut Budget<'_>,
        statistics: &mut Statistics,
        accepted: &mut Vec<bool>,
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
            let mut result_bytes = scratch::results(input.candidates.len())?;
            scratch::admit(result_bytes, 0, self.max_scratch_bytes)?;
            let residuals = input
                .verdicts
                .iter()
                .filter(|v| **v == BatchVerdict::Residual)
                .count();
            let regional_join = roots.is_some() && self.pool.is_some() && residuals != 0;
            if regional_join {
                result_bytes = scratch::regional_results(input.candidates.len())?;
            }
            let admitted = scratch::admit(
                result_bytes,
                self.workers().min(residuals),
                self.max_scratch_bytes,
            )?;
            progress.effective_workers = admitted;
            progress.requested_scratch_bytes = result_bytes;
            progress.peak_scratch_bytes = result_bytes;
            accepted
                .try_reserve_exact(input.candidates.len())
                .map_err(|_| Incomplete::Allocation)?;
            let mut workspaces = storage(admitted)?;
            let mut peak = u128::from(result_bytes)
                + workspaces.capacity() as u128 * std::mem::size_of::<ReductWorkspace>() as u128;
            record_peak(&mut progress, peak)?;
            for _ in 0..admitted {
                budget.cancellation.poll()?;
                let workspace = ReductWorkspace::default();
                // Every header was counted with the outer allocation; a fresh
                // workspace holds its evaluation alone, which grows per query.
                peak += workspace.retained_bytes() - std::mem::size_of::<ReductWorkspace>() as u128;
                record_peak(&mut progress, peak)?;
                if progress.peak_scratch_bytes > self.max_scratch_bytes {
                    return Err(Incomplete::CompletionScratch);
                }
                workspaces.push(workspace);
            }
            if let Some(pool) = &self.pool
                && !workspaces.is_empty()
            {
                let mut join = Join {
                    pool,
                    input,
                    budget,
                    statistics,
                    accepted,
                    progress: &mut progress,
                    workspaces: &mut workspaces,
                };
                if let Some(roots) = roots {
                    let mut slots = storage(input.candidates.len())?;
                    let extra = (slots.capacity() - input.candidates.len()) as u128
                        * size_of::<RegionalJob>() as u128;
                    record_peak(join.progress, peak + extra)?;
                    if peak + extra > u128::from(self.max_scratch_bytes) {
                        return Err(Incomplete::CompletionScratch);
                    }
                    slots.resize_with(input.candidates.len(), || RegionalJob::waiting(None));
                    for (index, root) in roots {
                        slots[index] = RegionalJob::waiting(Some(root));
                    }
                    join.execute(slots)
                } else {
                    let mut slots = storage(input.candidates.len())?;
                    slots.resize_with(input.candidates.len(), || None);
                    join.execute(slots)
                }
            } else {
                scalar(
                    input,
                    roots,
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

/// Borrows the immutable batch owner for the lifetime of a joined execution.
/// No candidate restriction, cursor, objective state or publication sink enters
/// a worker; only the original Theory and its matching interpretation do.
#[derive(Clone, Copy)]
pub(super) struct Input<'a> {
    pub(super) candidates: &'a [Interpretation],
    pub(super) verdicts: &'a [BatchVerdict],
    pub(super) limits: Limits,
    /// The region query, over the original index its owner lends; completion
    /// never builds one.
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
    root: QueryRoot,
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
            let verdict = match input.query {
                Some(query) => {
                    let result = (|| {
                        let evaluated = workspace.evaluate(
                            candidate,
                            input.limits,
                            budget.cancellation,
                            statistics,
                        )?;
                        if !evaluated.truth.is_model() {
                            return Err(Incomplete::InvalidWitness);
                        }
                        let started = timing::start(statistics.phase_timings.as_ref());
                        increment(&mut statistics.countermodel_queries)?;
                        let result = match root {
                            QueryRoot::Supplied(root) => query
                                .check_region(
                                    &evaluated.truth,
                                    root,
                                    input.limits,
                                    budget,
                                    (statistics, evaluated.narrowing),
                                )
                                .map(Checked::Regional),
                            QueryRoot::Unneeded => return Err(Incomplete::InvalidWitness),
                            QueryRoot::Full => query
                                .check(
                                    &evaluated.truth,
                                    input.limits,
                                    budget,
                                    statistics,
                                    evaluated.narrowing,
                                )
                                .map(Checked::Whole),
                        };
                        timing::finish(&mut statistics.phase_timings, Phase::Reduct, started);
                        result
                    })();
                    workspace.finish_check(result, input.limits.max_reduct_bytes, statistics)?
                }
                None => return Err(Incomplete::InvalidWitness),
            };
            match verdict {
                Checked::Regional(witness) => Ok(witness.is_none()),
                Checked::Whole(check) => match Decision::from(check) {
                    Decision::Stable => Ok(true),
                    Decision::Refuted => Ok(false),
                    Decision::Invalid => Err(Incomplete::InvalidWitness),
                },
            }
        }
    }
}

// A regional terminal carries no query. If it ever reaches a residual
// classifier, refuse the inconsistent job without substituting a full walk.
enum QueryRoot {
    Full,
    Supplied(Region),
    Unneeded,
}

enum Checked {
    Whole(crate::Check),
    Regional(Option<Interpretation>),
}

fn scalar(
    input: Input<'_>,
    roots: Option<Vec<(usize, Region)>>,
    budget: &mut Budget<'_>,
    statistics: &mut Statistics,
    accepted: &mut Vec<bool>,
    progress: &mut CompletionStatistics,
    workspaces: &mut [ReductWorkspace],
) -> Result<(), Incomplete> {
    progress.worker_original_validation = None;
    progress.worker_reduct = None;
    let mut local = Budget {
        quota: LocalQuota,
        limits: budget.limits,
        cancellation: budget.cancellation,
        statistics: budget.statistics,
    };
    let mut unused = ReductWorkspace::default();
    let workspace = workspaces.first_mut().unwrap_or(&mut unused);
    let mut roots = roots.map(Vec::into_iter);
    let result = (|| {
        for (index, (candidate, &verdict)) in
            input.candidates.iter().zip(input.verdicts).enumerate()
        {
            let root = match (&mut roots, verdict) {
                (None, _) => QueryRoot::Full,
                (Some(rows), BatchVerdict::Residual) => {
                    let (slot, root) = rows.next().ok_or(Incomplete::InvalidWitness)?;
                    if slot != index {
                        return Err(Incomplete::InvalidWitness);
                    }
                    QueryRoot::Supplied(root)
                }
                (Some(_), _) => QueryRoot::Unneeded,
            };
            // Retain the existing scalar counters, work order and failure boundary.
            let result = classify(
                input, candidate, verdict, root, &mut local, statistics, workspace,
            );
            entered(progress, verdict, &result);
            accepted.push(result?);
        }
        Ok(())
    })();
    budget.statistics = local.statistics;
    result
}

/// The legacy job buffer keeps its exact existing representation. A regional
/// slot uses the same allocation for its incoming root and joined receipt.
trait Job: Send {
    fn execute(&mut self, run: impl FnOnce(QueryRoot) -> Outcome);
    fn into_outcome(self) -> Outcome;
}

impl Job for Option<Outcome> {
    fn execute(&mut self, run: impl FnOnce(QueryRoot) -> Outcome) {
        *self = Some(run(QueryRoot::Full));
    }

    fn into_outcome(self) -> Outcome {
        self.expect("joined indexed execution fills every owned slot")
    }
}

// Only the small job state changes variant. The one statistics header is
// retained by its ordered slot and initialized when the worker finishes.
struct RegionalJob {
    stage: RegionalStage,
    statistics: Option<Statistics>,
}

enum RegionalStage {
    Pending(Option<Region>),
    Finished(Result<bool, Incomplete>),
}

impl RegionalJob {
    fn waiting(root: Option<Region>) -> Self {
        Self {
            stage: RegionalStage::Pending(root),
            statistics: None,
        }
    }
}

impl Job for RegionalJob {
    fn execute(&mut self, run: impl FnOnce(QueryRoot) -> Outcome) {
        let RegionalStage::Pending(root) = &mut self.stage else {
            unreachable!("joined slot was served twice")
        };
        let outcome = run(root.take().map_or(QueryRoot::Unneeded, QueryRoot::Supplied));
        self.statistics = Some(outcome.statistics);
        self.stage = RegionalStage::Finished(outcome.result);
    }

    fn into_outcome(self) -> Outcome {
        let RegionalStage::Finished(result) = self.stage else {
            unreachable!("joined region was not served")
        };
        Outcome {
            result,
            statistics: self
                .statistics
                .expect("joined regional job recorded its statistics"),
        }
    }
}

struct Join<'a, 'b, 'c> {
    pool: &'a rayon::ThreadPool,
    input: Input<'b>,
    budget: &'a mut Budget<'c>,
    statistics: &'a mut Statistics,
    accepted: &'a mut Vec<bool>,
    progress: &'a mut CompletionStatistics,
    workspaces: &'a mut [ReductWorkspace],
}

impl Join<'_, '_, '_> {
    fn execute<J: Job>(&mut self, mut outcomes: Vec<J>) -> Result<(), Incomplete> {
        let input = self.input;
        let budget = &mut *self.budget;
        let statistics = &mut *self.statistics;
        let progress = &mut *self.progress;
        let workspaces = &mut *self.workspaces;
        let accepted = &mut *self.accepted;
        let pool = self.pool;
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
                        slot.execute(|root| {
                            run(
                                input,
                                chunk * chunk_size + offset,
                                root,
                                &shared,
                                cancellation,
                                timed,
                                workspace,
                            )
                        });
                    }
                });
        });
        // Work is reserved before the operation. Read after join, including every
        // failed job, before choosing an error in original candidate order.
        shared.record(&mut budget.statistics);
        let mut failure = None;
        for (outcome, &verdict) in outcomes.into_iter().zip(input.verdicts) {
            let outcome = outcome.into_outcome();
            entered(progress, verdict, &outcome.result);
            let merged = merge(statistics, &outcome.statistics);
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
}

fn run(
    input: Input<'_>,
    index: usize,
    root: QueryRoot,
    shared: &SharedBudget,
    cancellation: &Cancellation,
    timed: bool,
    workspace: &mut ReductWorkspace,
) -> Outcome {
    let mut budget = Budget {
        quota: shared.lease(cancellation),
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
        root,
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

fn merge(statistics: &mut Statistics, worker: &Statistics) -> Result<(), Incomplete> {
    add(
        &mut statistics.countermodel_queries,
        worker.countermodel_queries,
    )?;
    add(&mut statistics.countermodels, worker.countermodels)?;
    add(
        &mut statistics.reduct.original_work,
        worker.reduct.original_work,
    )?;
    add(
        &mut statistics.reduct.frozen_positive_attempts,
        worker.reduct.frozen_positive_attempts,
    )?;
    add(
        &mut statistics.reduct.frozen_positive_completed,
        worker.reduct.frozen_positive_completed,
    )?;
    add(
        &mut statistics.reduct.frozen_positive_preparation_work,
        worker.reduct.frozen_positive_preparation_work,
    )?;
    add(
        &mut statistics.reduct.frozen_positive_work,
        worker.reduct.frozen_positive_work,
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
