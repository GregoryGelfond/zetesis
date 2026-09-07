//! Bounded submissions to one explicitly owned Rayon pool.

use std::fmt;
use std::num::NonZeroUsize;
use std::sync::{Mutex, TryLockError};

use rayon::prelude::*;
use zetesis_core::{GroundProgram, Program, Seed};

use crate::{Check, Control, Limits, StaticCheck, Stop, check, check_static};

/// A fixed worker pool with an explicit maximum admitted batch size. Each
/// invocation is synchronous; no unbounded background submission queue exists.
pub struct BatchOracle {
    pool: rayon::ThreadPool,
    max_candidates: usize,
    admission: Mutex<()>,
}

impl BatchOracle {
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
            admission: Mutex::new(()),
        })
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
        control: &Control,
    ) -> Result<Vec<Result<Check, Stop>>, BatchError> {
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
                .par_iter()
                .map(|seed| check(program, seed, limits, control))
                .collect()
        }))
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
        control: &Control,
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
                .par_iter()
                .map(|seed| check_static(graph, seed, limits, control))
                .collect()
        }))
    }
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
            Self::Capacity { .. } | Self::Busy | Self::Poisoned => None,
        }
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
    use crate::{Control, Limits};

    fn fixture() -> (GroundProgram, Vec<Seed>) {
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
        let seeds = [vec!["b"], vec![], vec!["a"], vec!["a", "b"]]
            .map(|names| Seed::new(&program, model(&names).atoms().iter().cloned()).unwrap());
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
    }

    fn exact_results(pool: &BatchOracle, graph: &GroundProgram, seeds: &[Seed]) {
        let lazy = pool
            .check_batch(
                graph.program(),
                seeds,
                Limits::default(),
                &Control::default(),
            )
            .expect("lazy admission is available");
        let dense = pool
            .check_static_batch(graph, seeds, Limits::default(), &Control::default())
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
                let cancelled = Control::default();
                cancelled.cancel();
                let limits = Limits {
                    max_work: 0,
                    max_derived_atoms: 0,
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
}
