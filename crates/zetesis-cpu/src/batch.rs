//! Bounded submissions to one explicitly owned Rayon pool.

use std::fmt;
use std::num::NonZeroUsize;
use std::sync::{Mutex, TryLockError};

use rayon::prelude::*;
use zetesis_core::{GroundProgram, Program, Seed, SeedView};

use crate::{
    Cancellation, Check, Limits, PreparationLimits, PreparationStatistics, StaticCheck, Stop,
    check_static_view,
};

mod prepared;

/// A fixed worker pool with an explicit maximum admitted batch size. Each
/// invocation is synchronous; no unbounded background submission queue exists.
pub struct BatchOracle {
    pool: rayon::ThreadPool,
    max_candidates: usize,
    max_closure_bytes: usize,
    preparation_limits: PreparationLimits,
    admission: Mutex<prepared::Cache>,
}

impl BatchOracle {
    /// Default named storage reservation across simultaneously active closures.
    /// Every assigned worker is admitted at the full per-closure allowance, so
    /// the workers share this ceiling: four at the default 128 MiB, or a
    /// smaller allowance each for more workers.
    pub const DEFAULT_CLOSURE_BYTES: usize = 536_870_912;

    /// Build an owned worker pool; zero worker or queue sizes are unrepresentable.
    ///
    /// # Errors
    /// Returns the typed Rayon pool-construction failure.
    pub fn new(workers: NonZeroUsize, max_candidates: NonZeroUsize) -> Result<Self, BatchError> {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers.get())
            .build()
            .map_err(BatchError::Pool)?;
        Ok(Self {
            pool,
            max_candidates: max_candidates.get(),
            max_closure_bytes: Self::DEFAULT_CLOSURE_BYTES,
            preparation_limits: PreparationLimits::default(),
            admission: Mutex::new(prepared::Cache::default()),
        })
    }

    /// Set the collective storage allowance for independent lazy closures.
    ///
    /// Admission counts idle retained workspaces and the assigned active
    /// workspace allowances, each reduced by the shared preparation's retained
    /// bytes; the cache's own header is bookkeeping outside this ceiling, so
    /// `workers * max_closure_bytes` is what the ceiling must hold. Retained
    /// capacity under tighter candidate limits remains counted until a refused
    /// check discards it.
    /// Static and shared-round execution retain
    /// their separate storage limits. Input seeds, completed returned models,
    /// allocator overhead and worker stacks are excluded; this is not RSS.
    #[must_use]
    pub fn with_closure_storage_limit(mut self, bytes: usize) -> Self {
        self.max_closure_bytes = bytes;
        self
    }

    /// Set independent immutable query-preparation bounds. Existing preparation
    /// can be reused for the exact program; work is charged only when built.
    /// Cached preparation bytes must still satisfy a changed byte allowance.
    #[must_use]
    pub fn with_preparation_limits(mut self, limits: PreparationLimits) -> Self {
        self.preparation_limits = limits;
        self
    }

    /// Actual preparation and assigned-owner reuse receipts. This read neither
    /// prepares a program nor starts oracle work. Reused owners are assigned
    /// slots retained from an earlier submission, not successful candidate counts.
    ///
    /// # Errors
    /// Returns Busy or Poisoned when the shared admission owner is unavailable,
    /// or a checked named-storage sum cannot be represented.
    pub fn query_statistics(&self) -> Result<QueryStatistics, BatchError> {
        self.admission
            .try_lock()
            .map_err(|error| admission_error(&error))?
            .statistics()
    }

    /// Check a bounded slice and return results in input order. Limits apply
    /// independently per candidate; cancellation and deadline are shared.
    ///
    /// # Errors
    /// Refuses the submission before work if its size exceeds the bound or a
    /// concurrent caller already occupies this pool. The pool owns no waiting
    /// batch queue.
    pub fn check_batch(
        &self,
        program: &Program,
        seeds: &[Seed],
        limits: Limits,
        cancellation: &Cancellation,
    ) -> Result<Vec<Result<Check, Stop>>, BatchError> {
        self.check_batch_views(
            program,
            seeds.par_iter().map(Seed::view),
            limits,
            cancellation,
        )
    }

    /// Check indexed candidate views on this owned pool, preserving input order.
    /// A slice's `par_iter().map(Seed::view)` or
    /// `par_iter().map(SeedSelection::view)` borrows its owners without a
    /// temporary view vector or seed materialization. Limits and admission are
    /// identical to [`Self::check_batch`]. Each of at most `workers` contiguous
    /// ranges uses one exclusive persistent workspace; Rayon can steal ranges,
    /// but candidates inside a range run sequentially. Uneven candidate costs
    /// can therefore balance differently from per-candidate work stealing.
    ///
    /// # Errors
    /// Refuses over-capacity or occupied submissions before oracle work.
    /// Preparation failures are batch errors; per-candidate stops remain ordered
    /// item results. Empty submissions do not prepare a program.
    pub fn check_batch_views<'seed>(
        &self,
        program: &Program,
        seeds: impl IndexedParallelIterator<Item = SeedView<'seed>>,
        limits: Limits,
        cancellation: &Cancellation,
    ) -> Result<Vec<Result<Check, Stop>>, BatchError> {
        if seeds.len() > self.max_candidates {
            return Err(BatchError::Capacity {
                limit: self.max_candidates,
                actual: seeds.len(),
            });
        }
        let mut cache = self
            .admission
            .try_lock()
            .map_err(|error| admission_error(&error))?;
        cache.begin_submission();
        let length = seeds.len();
        let active = length.min(self.pool.current_num_threads());
        if active == 0 {
            cache.admit(0, limits, self.max_closure_bytes)?;
            return Ok(Vec::new());
        }
        cache.prepare(
            program,
            active,
            self.preparation_limits,
            self.max_closure_bytes,
            cancellation,
        )?;
        cache.admit(active, limits, self.max_closure_bytes)?;
        let execution = cache.execution(length, active, limits, cancellation);
        Ok(self.pool.install(|| seeds.with_producer(execution)))
    }

    /// Share source traversal across ordered candidate occurrences, evaluating
    /// each immutable chunk on this oracle's owned pool. Source limits are
    /// collective; world evaluation has its own work ceiling. No static
    /// grounding, retry, or independent-oracle fallback occurs.
    ///
    /// # Errors
    /// Admission uses the same nonblocking slot as the independent methods.
    /// Any source or world failure invalidates the entire batch and retains
    /// separate source/world progress through [`crate::lazy::shared::Error`].
    pub fn check_shared(
        &self,
        program: &Program,
        seeds: &[Seed],
        limits: crate::lazy::shared::Limits,
        selection: crate::lazy::SourceSelection,
        cancellation: &Cancellation,
    ) -> Result<crate::lazy::shared::Batch, crate::lazy::shared::Error> {
        self.check_shared_views(
            program,
            seeds.iter().map(Seed::view),
            limits,
            selection,
            cancellation,
        )
    }

    /// Share source traversal over borrowed candidate views. Cloned iterators
    /// must preserve length, order and identities, as in
    /// [`crate::lazy::check_with_views`]. This allocates no temporary seed/view
    /// collection and shares the admission slot of [`Self::check_shared`].
    ///
    /// # Errors
    /// Returns the same admission or incomplete-batch failures and progress as
    /// [`Self::check_shared`].
    pub fn check_shared_views<'seed>(
        &self,
        program: &Program,
        seeds: impl ExactSizeIterator<Item = SeedView<'seed>> + Clone,
        limits: crate::lazy::shared::Limits,
        selection: crate::lazy::SourceSelection,
        cancellation: &Cancellation,
    ) -> Result<crate::lazy::shared::Batch, crate::lazy::shared::Error> {
        use crate::lazy::shared::Error;
        if seeds.len() > self.max_candidates {
            return Err(Error::Admission(BatchError::Capacity {
                limit: self.max_candidates,
                actual: seeds.len(),
            }));
        }
        let _admission = self.admission.try_lock().map_err(|error| {
            Error::Admission(match error {
                TryLockError::WouldBlock => BatchError::Busy,
                TryLockError::Poisoned(_) => BatchError::Poisoned,
            })
        })?;
        crate::lazy::shared::check(&self.pool, program, seeds, limits, selection, cancellation)
    }

    /// Check a bounded slice against an explicitly compiled graph, preserving
    /// input order. Limits apply independently per candidate; cancellation and
    /// deadline are shared. Uses the same admission slot as [`Self::check_batch`].
    ///
    /// # Errors
    /// Refuses batches over capacity or concurrent submissions before any work.
    /// Individual static oracle stops remain distinct per-candidate results.
    pub fn check_static_batch(
        &self,
        graph: &GroundProgram,
        seeds: &[Seed],
        limits: Limits,
        cancellation: &Cancellation,
    ) -> Result<Vec<Result<StaticCheck, Stop>>, BatchError> {
        self.check_static_batch_views(
            graph,
            seeds.par_iter().map(Seed::view),
            limits,
            cancellation,
        )
    }

    /// Check indexed borrowed candidates against the explicit static graph.
    /// Ordered collection and the owned execution pool are unchanged from
    /// [`Self::check_static_batch`]; no seed payload is materialized.
    ///
    /// # Errors
    /// Refuses over-capacity or occupied submissions before oracle work.
    pub fn check_static_batch_views<'seed>(
        &self,
        graph: &GroundProgram,
        seeds: impl IndexedParallelIterator<Item = SeedView<'seed>>,
        limits: Limits,
        cancellation: &Cancellation,
    ) -> Result<Vec<Result<StaticCheck, Stop>>, BatchError> {
        if seeds.len() > self.max_candidates {
            return Err(BatchError::Capacity {
                limit: self.max_candidates,
                actual: seeds.len(),
            });
        }
        let _admission = self.admission.try_lock().map_err(|error| match error {
            TryLockError::WouldBlock => BatchError::Busy,
            TryLockError::Poisoned(_) => BatchError::Poisoned,
        })?;
        Ok(self.pool.install(|| {
            seeds
                .map(|seed| check_static_view(graph, seed, limits, cancellation))
                .collect()
        }))
    }
}

/// Actual immutable preparation and fixed workspace ownership receipts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueryStatistics {
    /// Currently retained completed preparation, absent before preparation or
    /// after its cache is retired.
    pub preparation: Option<PreparationStatistics>,
    /// Completed preparation builds over this oracle's lifetime.
    pub preparation_builds: u128,
    /// Persistent slots, including idle slots from a previously larger batch.
    pub retained_workspaces: usize,
    /// Slots assigned by the latest independent submission acquiring admission.
    /// Zero when empty or refused during preparation/reservation.
    pub active_workspaces: usize,
    /// Assigned slots retained from an earlier submission. Zero when no slots
    /// were assigned by the latest independent submission acquiring admission.
    pub reused_workspaces: usize,
    /// Actual named cache envelope: retained workspaces and spare slot
    /// capacity, excluding the cache's own header, returned results and
    /// source payload.
    pub retained_bytes: u128,
    /// Collective active/idle/preparation envelope of the latest independent
    /// submission acquiring admission. Capacity and busy refusals cannot update
    /// these receipts because they do not acquire the cache.
    /// This reserves configured capacity; it is not an observed allocation peak.
    /// Zero means no successful reservation or the latest admission was refused.
    pub reserved_bytes: u128,
}

/// A batch was not submitted; individual oracle stops are separate results.
#[derive(Debug)]
pub enum BatchError {
    /// The owned worker pool could not be created.
    Pool(rayon::ThreadPoolBuildError),
    /// Another batch currently owns this pool's admission slot.
    Busy,
    /// A previous panic poisoned the batch admission state.
    Poisoned,
    /// Immutable query or workspace preparation stopped before candidate work.
    Preparation(Stop),
    /// Simultaneously active independent closures cannot reserve their limits.
    ClosureStorage {
        /// Shared preparation, idle cache and assigned active-owner envelope.
        required: u128,
        /// Collective named-storage allowance.
        limit: u128,
    },
    /// A submitted slice exceeded the explicit batch bound.
    Capacity {
        /// Maximum admitted candidates.
        limit: usize,
        /// Submitted candidates.
        actual: usize,
    },
}

impl fmt::Display for BatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pool(error) => error.fmt(f),
            Self::Busy => f.write_str("another batch currently occupies the owned pool"),
            Self::Poisoned => f.write_str("the owned pool's admission state is poisoned"),
            Self::Preparation(stop) => write!(f, "query preparation stopped: {stop}"),
            Self::ClosureStorage { required, limit } => {
                write!(
                    f,
                    "closure storage reservation {required} exceeds byte limit {limit}"
                )
            }
            Self::Capacity { limit, actual } => {
                write!(f, "batch of {actual} exceeds capacity {limit}")
            }
        }
    }
}

impl std::error::Error for BatchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Pool(error) => Some(error),
            Self::Preparation(stop) => Some(stop),
            Self::Capacity { .. } | Self::ClosureStorage { .. } | Self::Busy | Self::Poisoned => {
                None
            }
        }
    }
}

fn admission_error<T>(error: &TryLockError<T>) -> BatchError {
    match error {
        TryLockError::WouldBlock => BatchError::Busy,
        TryLockError::Poisoned(_) => BatchError::Poisoned,
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;
    use std::sync::mpsc::sync_channel;
    use std::thread;
    use std::time::Duration;

    use zetesis_core::{
        AdmissionLimits, Atom, AtomPattern, GroundProgram, Model, Predicate, Program, Seed,
        StaticLimits, Template,
    };

    use super::{BatchError, BatchOracle};
    use crate::{Cancellation, Limits};

    pub(super) fn fixture() -> (GroundProgram, Vec<Seed>) {
        let [a, b, c] = ["a", "b", "c"]
            .map(|name| AtomPattern::new(Predicate::new(name, 0).unwrap(), vec![]).unwrap());
        let program = Program::new(
            vec![
                Template::new(Some(a.clone()), vec![], vec![], vec![b.clone()], vec![]),
                Template::new(Some(b.clone()), vec![], vec![], vec![a.clone()], vec![]),
                Template::new(Some(c), vec![a.clone()], vec![], vec![], vec![]),
                Template::new(None, vec![a, b], vec![], vec![], vec![]),
            ],
            AdmissionLimits::default(),
        )
        .unwrap();
        let seeds = [vec!["b"], vec![], vec!["a"], vec!["a", "b"]].map(|names| {
            Seed::new(
                &program,
                names
                    .iter()
                    .map(|name| Atom::new(Predicate::new(*name, 0).unwrap(), vec![]).unwrap()),
            )
            .unwrap()
        });
        (
            GroundProgram::compile(&program, StaticLimits::default()).unwrap(),
            seeds.into(),
        )
    }

    fn model(names: &[&str]) -> Model {
        Model::new(
            names
                .iter()
                .map(|name| Atom::new(Predicate::new(*name, 0).unwrap(), vec![]).unwrap()),
        )
        .unwrap()
    }

    fn exact_results(pool: &BatchOracle, graph: &GroundProgram, seeds: &[Seed]) {
        let lazy = pool
            .check_batch(
                graph.program(),
                seeds,
                Limits::default(),
                &Cancellation::default(),
            )
            .expect("lazy admission is available");
        let dense = pool
            .check_static_batch(graph, seeds, Limits::default(), &Cancellation::default())
            .expect("static admission is available");
        let expected = [
            (model(&["b"]), true, false, false),
            (model(&["a", "b", "c"]), false, true, true),
            (model(&["a", "c"]), true, false, false),
            (model(&[]), false, false, true),
        ];
        assert_eq!(lazy.len(), expected.len());
        assert_eq!(dense.len(), expected.len());
        for ((lazy, dense), (closure, accepted, constraint, mismatch)) in
            lazy.into_iter().zip(dense).zip(expected)
        {
            let lazy = lazy.expect("complete lazy reduct check");
            let dense = dense.expect("complete static reduct check");
            assert_eq!(*lazy.closure(), closure);
            assert_eq!(
                graph.model_from_words(dense.closure_words()).unwrap(),
                closure
            );
            assert_eq!((lazy.accepted(), dense.accepted()), (accepted, accepted));
            assert_eq!(
                (lazy.constraint_violated(), dense.constraint_violated()),
                (constraint, constraint)
            );
            assert_eq!(
                (lazy.seed_mismatch(), dense.seed_mismatch()),
                (mismatch, mismatch)
            );
        }
    }

    #[test]
    fn lazy_and_static_share_busy_admission_and_recover_exactly() {
        let (graph, seeds) = fixture();
        let pool = BatchOracle::new(
            NonZeroUsize::new(2).unwrap(),
            NonZeroUsize::new(seeds.len()).unwrap(),
        )
        .unwrap();
        exact_results(&pool, &graph, &seeds);
        let admission = pool.admission.lock().unwrap();
        thread::scope(|scope| {
            let (sender, receiver) = sync_channel(1);
            let (pool, graph, seeds) = (&pool, &graph, &seeds);
            let worker = scope.spawn(move || {
                let cancelled = Cancellation::default();
                cancelled.cancel();
                let limits = Limits {
                    max_work: 0,
                    max_derived_atoms: 0,
                    ..Limits::default()
                };
                let lazy = pool.check_batch(graph.program(), seeds, limits, &cancelled);
                let dense = pool.check_static_batch(graph, seeds, limits, &cancelled);
                sender.send((lazy, dense)).unwrap();
            });
            let completed = receiver.recv_timeout(Duration::from_secs(5));
            // Release before joining or asserting: a regression to blocking lock()
            // must fail this test instead of leaving its worker deadlocked.
            drop(admission);
            worker
                .join()
                .expect("submission worker completes after release");
            let (lazy, dense) = completed.expect("both submissions return while occupied");
            assert!(matches!(lazy, Err(BatchError::Busy)));
            assert!(matches!(dense, Err(BatchError::Busy)));
        });
        exact_results(&pool, &graph, &seeds);
    }

    #[test]
    fn shared_false_gates_preserve_ordered_mismatches() {
        let (graph, seeds) = fixture();
        let pool = BatchOracle::new(
            NonZeroUsize::new(2).unwrap(),
            NonZeroUsize::new(seeds.len()).unwrap(),
        )
        .unwrap();
        let expected = [
            (model(&["b"]), true, false, false),
            (model(&["a", "b", "c"]), false, true, true),
            (model(&["a", "c"]), true, false, false),
            (model(&[]), false, false, true),
        ];
        for selection in [
            crate::lazy::SourceSelection::Union,
            crate::lazy::SourceSelection::Worlds,
        ] {
            let batch = pool
                .check_shared(
                    graph.program(),
                    &seeds,
                    crate::lazy::shared::Limits::default(),
                    selection,
                    &Cancellation::default(),
                )
                .unwrap();
            assert_eq!(batch.checks.len(), expected.len());
            for (check, (closure, accepted, constraint, mismatch)) in
                batch.checks.iter().zip(&expected)
            {
                assert_eq!(check.closure(), closure);
                assert_eq!(check.accepted(), *accepted);
                assert_eq!(check.constraint_violated(), *constraint);
                assert_eq!(check.seed_mismatch(), *mismatch);
            }
        }
    }

    #[test]
    fn occupied_pool_refuses_shared_submission_without_waiting() {
        let (graph, seeds) = fixture();
        let pool = BatchOracle::new(
            NonZeroUsize::new(2).unwrap(),
            NonZeroUsize::new(seeds.len()).unwrap(),
        )
        .unwrap();
        let admission = pool.admission.lock().unwrap();
        thread::scope(|scope| {
            let (sender, receiver) = sync_channel(1);
            let (pool, graph, seeds) = (&pool, &graph, &seeds);
            let worker = scope.spawn(move || {
                let result = pool.check_shared(
                    graph.program(),
                    seeds,
                    crate::lazy::shared::Limits::default(),
                    crate::lazy::SourceSelection::Union,
                    &Cancellation::default(),
                );
                sender.send(result).unwrap();
            });
            let completed = receiver.recv_timeout(Duration::from_secs(5));
            // Release before joining: a blocking-lock regression must fail,
            // rather than leave the test process deadlocked.
            drop(admission);
            worker.join().unwrap();
            assert!(matches!(
                completed.unwrap(),
                Err(crate::lazy::shared::Error::Admission(BatchError::Busy))
            ));
        });
        assert!(
            pool.check_shared(
                graph.program(),
                &seeds,
                crate::lazy::shared::Limits::default(),
                crate::lazy::SourceSelection::Union,
                &Cancellation::default()
            )
            .is_ok()
        );
    }
}

#[cfg(test)]
mod reuse_tests;
