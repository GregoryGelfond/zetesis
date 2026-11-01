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

mod observations;
mod scratch;

use super::regions::depth::{DepthWorkspace, QueryDepth};
use crate::{Cancellation, Incomplete, PhaseMeasurement, SearchPhaseTimings};
pub use observations::{
    RegionalObservations, RegionalQueryObservation, RegionalSlotObservation, RegionalSlotState,
};

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
    request: Option<(u64, u64)>,
    regional: Option<RegionalObservations>,
    max_scratch_bytes: u64,
}

impl Default for CompletionExecutor {
    fn default() -> Self {
        Self {
            pool: None,
            last: None,
            request: None,
            regional: None,
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
            request: None,
            regional: None,
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

    /// A regional result bit records absence in its supplied domain. Only
    /// the later regional pass can consume that result for publication.
    pub(super) fn complete(
        &mut self,
        input: Input<'_>,
        roots: Option<Vec<(usize, Region)>>,
        budget: &mut Budget<'_>,
        statistics: &mut Statistics,
        accepted: &mut Vec<bool>,
    ) -> Result<(), Incomplete> {
        self.last = None;
        if roots.is_none() {
            self.regional = None;
        }
        let mut depths = None;
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
            let (mut workspaces, peak) =
                prepare_workspaces(budget.cancellation, &mut progress, self.max_scratch_bytes)?;
            let observation = Attempt::prepare(
                self.regional.as_mut(),
                roots.as_deref(),
                input.candidates.len(),
                admitted,
                &mut depths,
            );
            if let Some(pool) = &self.pool
                && !workspaces.is_empty()
            {
                let join = Join {
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
                    join.execute(slots, observation)
                } else {
                    let mut slots = storage(input.candidates.len())?;
                    slots.resize_with(input.candidates.len(), || None);
                    join.execute(slots, observation)
                }
            } else {
                scalar(
                    input,
                    roots,
                    budget,
                    statistics,
                    accepted,
                    &mut progress,
                    (&mut workspaces, observation),
                )
            }
        })();
        if let Some(receipt) = &mut self.regional {
            receipt.finish(depths.as_deref());
        }
        progress.elapsed = started.map(|start| start.elapsed());
        self.last = Some(progress);
        result
    }
}

fn prepare_workspaces(
    cancellation: &Cancellation,
    progress: &mut CompletionStatistics,
    max_scratch_bytes: u64,
) -> Result<(Vec<ReductWorkspace>, u128), Incomplete> {
    let admitted = progress.effective_workers;
    let mut workspaces = storage(admitted)?;
    let mut peak = u128::from(progress.requested_scratch_bytes)
        + workspaces.capacity() as u128 * std::mem::size_of::<ReductWorkspace>() as u128;
    record_peak(progress, peak)?;
    for _ in 0..admitted {
        cancellation.poll()?;
        let workspace = ReductWorkspace::default();
        // Every header was counted with the outer allocation; a fresh
        // workspace holds its evaluation alone, which grows per query.
        peak += workspace.retained_bytes() - std::mem::size_of::<ReductWorkspace>() as u128;
        record_peak(progress, peak)?;
        if progress.peak_scratch_bytes > max_scratch_bytes {
            return Err(Incomplete::CompletionScratch);
        }
        workspaces.push(workspace);
    }
    Ok((workspaces, peak))
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

struct Attempt<'a> {
    rows: Option<&'a mut [Option<RegionalSlotObservation>]>,
    depths: Option<&'a mut [Option<DepthWorkspace>]>,
}

impl<'a> Attempt<'a> {
    /// Prepare optional per-chunk storage after the mandatory workspaces fit.
    /// The caller retains the depths for the width reduction after the join.
    fn prepare(
        receipt: Option<&'a mut RegionalObservations>,
        roots: Option<&[(usize, Region)]>,
        rows: usize,
        workers: usize,
        depths: &'a mut Option<Vec<Option<DepthWorkspace>>>,
    ) -> Self {
        let (Some(receipt), Some(roots)) = (receipt, roots) else {
            return Self {
                rows: None,
                depths: None,
            };
        };
        *depths = receipt.workers(rows, chunk_size(rows, workers), roots);
        Self {
            rows: receipt.slots.as_deref_mut(),
            depths: depths.as_deref_mut(),
        }
    }
}

struct Serve<'a> {
    workspace: &'a mut ReductWorkspace,
    slot: Option<&'a mut RegionalSlotObservation>,
    depth: Option<&'a mut DepthWorkspace>,
}

fn classify(
    input: Input<'_>,
    candidate: &Interpretation,
    verdict: BatchVerdict,
    root: QueryRoot,
    budget: &mut Budget<'_, impl Quota>,
    statistics: &mut Statistics,
    serve: Serve<'_>,
) -> Result<bool, Incomplete> {
    let mut serve = serve;
    let result = budget
        .cancellation
        .poll()
        .map_err(Incomplete::from)
        .and_then(|()| match verdict {
            BatchVerdict::NoProperSubset => Ok(true),
            BatchVerdict::Refuted => Ok(false),
            BatchVerdict::NotModel => Err(Incomplete::InvalidWitness),
            BatchVerdict::Residual => serve.query(input, candidate, root, budget, statistics),
        });
    if let Some(slot) = serve.slot
        && slot.state == RegionalSlotState::Unentered
    {
        slot.state = match result {
            Ok(true) => RegionalSlotState::NoWitnessInSuppliedRegion,
            Ok(false) => RegionalSlotState::ValidatedCountermodel,
            Err(stop) => RegionalSlotState::Stopped(stop),
        };
    }
    result
}

impl Serve<'_> {
    fn query(
        &mut self,
        input: Input<'_>,
        candidate: &Interpretation,
        root: QueryRoot,
        budget: &mut Budget<'_, impl Quota>,
        statistics: &mut Statistics,
    ) -> Result<bool, Incomplete> {
        let query = input.query.ok_or(Incomplete::InvalidWitness)?;
        let result = (|| {
            let evaluated = self.workspace.evaluate(
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
            let mut depth_counts = QueryDepth::default();
            let before = self.slot.as_ref().map(|_| {
                (
                    statistics.reduct.regions.regions,
                    budget.statistics.work,
                    statistics.phase_timings.map(|t| t.reduct),
                )
            });
            let result = match root {
                QueryRoot::Supplied(root) => query
                    .check_region(
                        &evaluated.truth,
                        root,
                        input.limits,
                        budget,
                        (statistics, evaluated.narrowing),
                        self.slot
                            .as_ref()
                            .map(|_| (&mut depth_counts, self.depth.as_deref_mut())),
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
            if let (Some(slot), Some((visits, work, old_timing))) = (&mut self.slot, before) {
                let host_time = old_timing
                    .zip(statistics.phase_timings)
                    .and_then(|(old, new)| {
                        (!old.overflowed && !new.reduct.overflowed)
                            .then(|| new.reduct.elapsed.checked_sub(old.elapsed))
                            .flatten()
                    });
                slot.query = Some(RegionalQueryObservation {
                    visited: statistics.reduct.regions.regions - visits,
                    attempted_splits: depth_counts.attempted_splits,
                    published_splits: depth_counts.published_splits,
                    deepest: depth_counts.deepest,
                    work: budget.statistics.work - work,
                    host_time,
                });
            }
            result
        })();

        let verdict =
            self.workspace
                .finish_check(result, input.limits.max_reduct_bytes, statistics)?;
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
    state: (&mut [ReductWorkspace], Attempt<'_>),
) -> Result<(), Incomplete> {
    let (workspaces, mut observation) = state;
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
                input,
                candidate,
                verdict,
                root,
                &mut local,
                statistics,
                Serve {
                    workspace,
                    slot: observation
                        .rows
                        .as_deref_mut()
                        .and_then(|rows| rows.get_mut(index)?.as_mut()),
                    depth: observation
                        .depths
                        .as_deref_mut()
                        .and_then(|w| w.first_mut()?.as_mut()),
                },
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

// Logical slot chunks are identical for scheduling and optional observations.
fn chunk_size(rows: usize, workers: usize) -> usize {
    rows.div_ceil(workers.max(1)).max(1)
}

impl Join<'_, '_, '_> {
    fn execute<J: Job>(
        self,
        mut outcomes: Vec<J>,
        observation: Attempt<'_>,
    ) -> Result<(), Incomplete> {
        let Self {
            pool,
            input,
            budget,
            statistics,
            accepted,
            progress,
            workspaces,
        } = self;
        let shared = SharedBudget::new(budget.limits, budget.statistics);
        let cancellation = budget.cancellation;
        let timed = statistics.phase_timings.is_some();
        let chunk_size = chunk_size(outcomes.len(), progress.effective_workers);
        let serve_chunk = |chunk: usize,
                           slots: &mut [J],
                           workspace: &mut ReductWorkspace,
                           mut rows: Option<&mut [Option<RegionalSlotObservation>]>,
                           mut depth: Option<&mut DepthWorkspace>| {
            for (offset, slot) in slots.iter_mut().enumerate() {
                slot.execute(|root| {
                    run(
                        input,
                        chunk * chunk_size + offset,
                        root,
                        &shared,
                        cancellation,
                        timed,
                        Serve {
                            workspace,
                            slot: rows.as_deref_mut().and_then(|rows| rows[offset].as_mut()),
                            depth: depth.as_deref_mut(),
                        },
                    )
                });
            }
        };
        pool.install(|| {
            let jobs = outcomes
                .par_chunks_mut(chunk_size)
                .zip(workspaces.par_iter_mut())
                .enumerate();
            if let Some(rows) = observation.rows {
                let observed = jobs.zip(rows.par_chunks_mut(chunk_size));
                if let Some(depths) = observation.depths {
                    observed.zip(depths.par_iter_mut()).for_each(
                        |(((i, (slots, w)), rows), depth)| {
                            serve_chunk(i, slots, w, Some(rows), depth.as_mut());
                        },
                    );
                } else {
                    observed.for_each(|((i, (slots, w)), rows)| {
                        serve_chunk(i, slots, w, Some(rows), None);
                    });
                }
            } else {
                jobs.for_each(|(i, (slots, w))| serve_chunk(i, slots, w, None, None));
            }
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
    serve: Serve<'_>,
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
        serve,
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
