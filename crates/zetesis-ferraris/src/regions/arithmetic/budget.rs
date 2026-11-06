//! Preparation accounting for algebraic source reads and bounded primitives.
//!
//! A primitive reserves its complete worst-case chunk before it executes and
//! settles only its actual logical work, including a typed failure. Optional
//! preparation exhaustion does not conceal cancellation or a refused global
//! reservation. Control is observed as every metered operation observes it:
//! by the reserved meter's account at every refill of `POLL_INTERVAL` permits,
//! by this budget before a refusal is reported, and where an operation begins
//! work it must not start once stopped. The existing narrowing meter retains
//! and refunds spare permits.

use zetesis_algebra::{Cancellation, Failure, LocalQuota, Meter, MeterBytes, Quota, Stop};

use crate::{AdmissionError, NodeView, Theory};

use super::work::{Phase, PreparationWork};

/// The largest admitted node-view path, including an ordinal cut's checks.
const VIEW_MAX_WORK: u64 = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BudgetError<E = Stop> {
    WorkDeclined,
    StorageDeclined { required: u128, limit: usize },
    Stopped(Stop),
    Refused(E),
    Source(AdmissionError),
}

impl<E> From<Stop> for BudgetError<E> {
    fn from(stop: Stop) -> Self {
        Self::Stopped(stop)
    }
}

impl<E> From<Failure<E>> for BudgetError<E> {
    fn from(error: Failure<E>) -> Self {
        match error {
            Failure::Stopped(stop) => Self::Stopped(stop),
            Failure::Refused(error) => Self::Refused(error),
        }
    }
}

pub(super) struct Budget<'a, 'q, Q: Quota + ?Sized = LocalQuota> {
    work: &'a mut Meter<'q, Q>,
    start: u64,
    cancellation: &'a Cancellation,
    max_work: u64,
    max_bytes: usize,
    peak_bytes: u128,
    phases: PreparationWork,
}

impl<'a, 'q, Q: Quota + ?Sized> Budget<'a, 'q, Q> {
    pub(super) fn new(
        work: &'a mut Meter<'q, Q>,
        max_work: u64,
        max_bytes: usize,
        cancellation: &'a Cancellation,
    ) -> Self {
        Self {
            start: work.charged(),
            work,
            cancellation,
            max_work,
            max_bytes,
            peak_bytes: 0,
            phases: PreparationWork::default(),
        }
    }

    /// Charge a bounded primitive immediately before its logical operation.
    /// Meter bookkeeping and cancellation polling do not charge themselves.
    pub(super) fn charge(&mut self, count: u64) -> Result<(), BudgetError<Q::Error>> {
        self.bounded(count, || ((), count))
    }

    pub(super) fn bounded<T>(
        &mut self,
        maximum: u64,
        operation: impl FnOnce() -> (T, u64),
    ) -> Result<T, BudgetError<Q::Error>> {
        if maximum > self.max_work.saturating_sub(self.work()) {
            return Err(self.declined(maximum));
        }
        match self.work.bounded(maximum, operation) {
            Ok(value) => Ok(value),
            Err(failure) => Err(self.refused(BudgetError::from(failure))),
        }
    }

    /// The optional allowance declines: the global reservation is still made
    /// first, so its refusal takes precedence, and a stop observed before
    /// either is reported in their place.
    #[cold]
    #[inline(never)]
    fn declined(&mut self, maximum: u64) -> BudgetError<Q::Error> {
        match self.work.bounded(maximum, || ((), 0)) {
            Ok(()) => self.refused(BudgetError::WorkDeclined),
            Err(failure) => self.refused(BudgetError::from(failure)),
        }
    }

    /// A refusal as reported: a stop observed first takes its place.
    #[cold]
    #[inline(never)]
    fn refused(&self, error: BudgetError<Q::Error>) -> BudgetError<Q::Error> {
        match self.cancellation.poll() {
            Err(stop) => BudgetError::Stopped(stop),
            Ok(()) => error,
        }
    }

    /// Observe control: a requested stop is reported before any further work.
    /// Out of line, so that the bounded charge it guards stays small enough to
    /// inline where it is hot.
    #[cold]
    #[inline(never)]
    pub(super) fn poll(&self) -> Result<(), BudgetError<Q::Error>> {
        self.cancellation.poll().map_err(BudgetError::Stopped)
    }

    /// Read through the canonical formula view of an admitted owner.
    /// A two-child connective is necessarily inline; admitted spans have at
    /// least three children. Thus the borrowed view identifies its actual
    /// path without another read of raw storage. Unsupported AR shapes still
    /// pay their complete view cost; deciding the grammar is the caller's work.
    pub(super) fn node<'t>(
        &mut self,
        theory: &'t Theory,
        id: usize,
    ) -> Result<NodeView<'t>, BudgetError<Q::Error>> {
        self.bounded(VIEW_MAX_WORK, || {
            let result = theory.view().node(id);
            let actual = match &result {
                Ok(NodeView::Atom(_)) => 4,
                Ok(NodeView::False | NodeView::Implies(..)) => 5,
                Ok(NodeView::And(children) | NodeView::Or(children)) => {
                    if children.len() == 2 {
                        5
                    } else {
                        12
                    }
                }
                Ok(NodeView::Threshold(_)) => 21,
                Ok(NodeView::ExtremumCut(_)) => 24,
                // An admitted owner cannot have an invalid arena or child.
                // Its only failing lookup is an absent ID: the fast and slow
                // range checks, followed by publication of the refusal tag.
                Err(_) => 3,
            };
            (result, actual)
        })?
        .map_err(BudgetError::Source)
    }

    /// Check the caller's total live requirement without allocating anything.
    /// The caller supplies both requested and actual-capacity observations.
    /// Peak includes a refused requirement, not only retained allocations.
    pub(super) fn admit_bytes(&mut self, required: u128) -> Result<(), BudgetError<Q::Error>> {
        self.cancellation.poll().map_err(BudgetError::Stopped)?;
        MeterBytes::observe_peak(required, self.max_bytes as u128, &mut self.peak_bytes).map_err(
            |_| BudgetError::StorageDeclined {
                required,
                limit: self.max_bytes,
            },
        )
    }

    pub(super) fn work(&self) -> u64 {
        self.work.charged() - self.start
    }

    /// Use the same work owner for a narrowing kernel that already meters
    /// its operations. Its performed prefix remains part of this budget.
    pub(super) fn meter_mut(&mut self) -> &mut Meter<'q, Q> {
        self.work
    }

    /// Observe a phase's own prefix, excluding work already attributed to its
    /// nested phases. Both successful and failed prefixes remain in receipts.
    pub(super) fn in_phase<T>(
        &mut self,
        phase: Phase,
        operation: impl FnOnce(&mut Self) -> Result<T, BudgetError<Q::Error>>,
    ) -> Result<T, BudgetError<Q::Error>> {
        let start = self.work();
        let nested = self.phases.total();
        let result = operation(self);
        let work = self.work() - start - (self.phases.total() - nested);
        self.phases.record(phase, work);
        result
    }

    pub(super) const fn phases(&self) -> PreparationWork {
        self.phases
    }

    pub(super) fn peak_bytes(&self) -> u128 {
        self.peak_bytes
    }

    /// Return all spare permits, including partial grants before a refusal.
    /// The charged-work receipt is unchanged and remains readable afterwards.
    pub(super) fn finish(&mut self) {
        self.work.refund();
    }
}
