//! Bounded tasks use one pool while the canonical coordinator stays in place.
//!
//! The caller owns admission, descriptors, slots and their storage leases. Each
//! worker returns its existing operation's receipt, including on unwind. The
//! caller settles every receipt before inspecting outcomes or publishing. This
//! executor neither implements a second meter nor determines source semantics.

use std::any::Any;
use std::num::NonZeroUsize;
use std::panic::{AssertUnwindSafe, catch_unwind};
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

use rayon::{Scope, ThreadPool, ThreadPoolBuildError, ThreadPoolBuilder};

/// Logical task geometry is independent of the matched physical worker count.
pub(super) const TASKS: usize = 14;

pub(crate) struct Executor {
    pool: ThreadPool,
    #[cfg(test)]
    submissions: AtomicUsize,
    #[cfg(test)]
    active: AtomicUsize,
    #[cfg(test)]
    peak: AtomicUsize,
}

/// Private comparison configuration, fixed in each frozen executable. This is
/// not the solver's search-worker setting or a proposed portable default.
#[cfg(not(test))]
const PHYSICAL_WORKERS: usize = 14;

/// The owner spans one entire preparation, including support and emission.
/// It never enters retained per-answer checks. Initialization failure retains
/// the actual foreign error and selects serial execution before cursor claim.
pub(crate) struct SourceExecution {
    executor: Option<Result<Executor, ThreadPoolBuildError>>,
}

impl SourceExecution {
    pub(crate) fn new() -> Self {
        #[cfg(not(test))]
        let workers = NonZeroUsize::new(PHYSICAL_WORKERS);
        #[cfg(test)]
        let workers = TEST_WORKERS.with(|count| NonZeroUsize::new(count.get()));
        Self {
            executor: workers.map(Executor::new),
        }
    }

    pub(crate) fn executor(&self) -> Option<&Executor> {
        self.executor
            .as_ref()
            .and_then(|result| result.as_ref().ok())
    }
}

#[cfg(test)]
impl Drop for SourceExecution {
    fn drop(&mut self) {
        LAST_SOURCE_TASKS.with(|count| {
            count.set(
                self.executor()
                    .map_or(0, |executor| executor.submissions.load(Ordering::Relaxed)),
            );
        });
    }
}

#[cfg(test)]
thread_local! {
    static TEST_WORKERS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static LAST_SOURCE_TASKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn with_source_workers<T>(
    workers: NonZeroUsize,
    action: impl FnOnce() -> T,
) -> (T, usize) {
    struct Reset(usize, usize);
    impl Drop for Reset {
        fn drop(&mut self) {
            TEST_WORKERS.with(|count| count.set(self.0));
            LAST_SOURCE_TASKS.with(|count| count.set(self.1));
        }
    }
    let _reset = Reset(
        TEST_WORKERS.with(|count| count.replace(workers.get())),
        LAST_SOURCE_TASKS.with(|count| count.replace(0)),
    );
    let value = action();
    (value, LAST_SOURCE_TASKS.with(std::cell::Cell::get))
}

#[cfg(test)]
struct Active<'a>(&'a Executor);
#[cfg(test)]
impl Drop for Active<'_> {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::Relaxed);
    }
}

impl Executor {
    /// Once per preparation owner. Failure precedes cursor claim and may select
    /// the ordinary serial path under the proposed private admission contract.
    pub(super) fn new(workers: NonZeroUsize) -> Result<Self, ThreadPoolBuildError> {
        Self::with_factory(workers, |workers| {
            ThreadPoolBuilder::new().num_threads(workers.get()).build()
        })
    }

    fn with_factory(
        workers: NonZeroUsize,
        build: impl FnOnce(NonZeroUsize) -> Result<ThreadPool, ThreadPoolBuildError>,
    ) -> Result<Self, ThreadPoolBuildError> {
        build(workers).map(|pool| Self {
            pool,
            #[cfg(test)]
            submissions: AtomicUsize::new(0),
            #[cfg(test)]
            active: AtomicUsize::new(0),
            #[cfg(test)]
            peak: AtomicUsize::new(0),
        })
    }

    /// This closure is not `Send`: `Rc`-bearing canonical owners stay on the caller.
    /// Only explicitly spawned worker closures must satisfy Rayon's `Send` bound.
    fn in_place<'scope, F, R>(&self, action: F) -> R
    where
        F: FnOnce(&Scope<'scope>) -> R,
    {
        self.pool.in_place_scope(action)
    }

    /// All inputs and output slots are admitted before submission. Logical order
    /// comes from the slots, never from completion order. `run` must use `Capture`
    /// around all charged work; its construction and receipt extraction must not
    /// panic. Unexpected wrapper panics keep Rayon's ordinary joined unwind.
    pub(super) fn run<T, R, F>(&self, tasks: &[T], slots: &mut [Option<R>], run: F)
    where
        T: Sync,
        R: Send,
        F: Fn(&T) -> R + Sync,
    {
        assert!(tasks.len() <= TASKS);
        assert_eq!(tasks.len(), slots.len());
        assert!(slots.iter().all(Option::is_none));
        let (Some((first, tasks)), Some((first_slot, slots))) =
            (tasks.split_first(), slots.split_first_mut())
        else {
            return;
        };
        // The coordinator runs the first task itself instead of waiting idle,
        // so a one-task batch never enters the pool.
        if tasks.is_empty() {
            *first_slot = Some(Self::tracked(self, || run(first)));
            return;
        }
        self.in_place(|scope| {
            let run = &run;
            for (task, slot) in tasks.iter().zip(slots.iter_mut()) {
                scope.spawn(move |_| {
                    *slot = Some(Self::tracked(self, || run(task)));
                });
            }
            *first_slot = Some(Self::tracked(self, || run(first)));
        });
    }

    /// Run one task, recording it in the test receipts.
    fn tracked<R>(executor: &Self, task: impl FnOnce() -> R) -> R {
        #[cfg(test)]
        let _active = {
            executor.submissions.fetch_add(1, Ordering::Relaxed);
            let active = executor.active.fetch_add(1, Ordering::Relaxed) + 1;
            executor.peak.fetch_max(active, Ordering::Relaxed);
            Active(executor)
        };
        #[cfg(not(test))]
        let _ = executor;
        task()
    }
}

/// A panic is retained for settlement, then resumed by the coordinator. It is
/// never a typed source refusal, empty result or completed semantic operation.
pub(super) enum Outcome<T, E> {
    Returned(Result<T, E>),
    Panicked(Box<dyn Any + Send>),
}

/// The operation owns its existing accounting state; this wrapper only orders
/// unwind capture before receipt extraction. It adds no resource arithmetic.
pub(super) struct Capture<T, E, R> {
    outcome: Outcome<T, E>,
    receipt: Option<R>,
}

impl<T, E, R> Capture<T, E, R> {
    pub(super) fn run<S>(
        state: &mut S,
        operation: impl FnOnce(&mut S) -> Result<T, E>,
        receipt: impl FnOnce(&S) -> R,
    ) -> Self {
        let outcome = match catch_unwind(AssertUnwindSafe(|| operation(state))) {
            Ok(result) => Outcome::Returned(result),
            Err(panic) => Outcome::Panicked(panic),
        };
        Self {
            outcome,
            receipt: Some(receipt(state)),
        }
    }

    /// Consume only after the batch's complete settlement pass succeeds or has
    /// recorded its failure. Outputs stay owned until that decision is made.
    pub(super) fn into_outcome(self) -> Outcome<T, E> {
        assert!(self.receipt.is_none(), "operation receipt is not settled");
        self.outcome
    }

    pub(super) fn outcome(&self) -> &Outcome<T, E> {
        assert!(self.receipt.is_none(), "operation receipt is not settled");
        &self.outcome
    }

    pub(super) fn outcome_mut(&mut self) -> &mut Outcome<T, E> {
        assert!(self.receipt.is_none(), "operation receipt is not settled");
        &mut self.outcome
    }
}

/// Visit every completed attempt in logical order, retaining the first typed
/// settlement failure while still recording subsequent actual receipts. The
/// callback uses the existing source account; this function adds no meter.
pub(super) fn settle<T, E, R, F>(
    slots: &mut [Option<Capture<T, E, R>>],
    mut receipt: impl FnMut(R) -> Result<(), F>,
) -> Result<(), F> {
    let mut first = None;
    for slot in slots {
        let value = slot.as_mut().expect("joined task wrote its result slot");
        let actual = value.receipt.take().expect("receipt settles exactly once");
        if let Err(error) = receipt(actual)
            && first.is_none()
        {
            first = Some(error);
        }
    }
    first.map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests;
