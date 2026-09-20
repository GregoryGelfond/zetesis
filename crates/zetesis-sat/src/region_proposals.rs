//! Parallel production of classical candidates, with membership left to the caller.
//!
//! The frontier contains disjoint regions of the original candidate space. A
//! worker removes one region, applies the same readings as the scalar traversal,
//! then refutes it, returns its two children, or emits its classical leaf. No
//! worker checks answer-set membership. A bounded round reserves one output slot
//! per active region, so completed candidates plus active regions never exceed
//! the batch allowance. A split or refutation returns that reservation.
//!
//! All workers join before the next stage runs. Their work leases then settle
//! into the coordinator's cumulative budget, including failed attempts. Retained
//! frontier regions supply the next round; a stop never establishes coverage.
//! Candidate order depends on scheduling, while identity and coverage do not.

use std::num::NonZeroUsize;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use rayon::prelude::*;
use zetesis_cpu::regions::{Narrowing, Region};
use zetesis_ferraris::{Interpretation, Knowledge, Narrower, Producers, Theory};

use super::regions::{self, RegionCounts, RegionSearchStatistics};
use crate::search::{Budget, SharedBudget, WorkLease};
use crate::{Control, Incomplete, SearchStatistics};

/// Idle producers poll control while another producer owns the last region.
const CONTROL_WAIT: Duration = Duration::from_millis(1);

type PendingRegion = (Region, Vec<Knowledge>);

/// A reusable candidate frontier and an owned Rayon executor. Its immutable
/// preparation is shared across workers and every bounded production round.
pub(crate) struct RegionProposals {
    producers: Option<Producers>,
    narrower: Narrower,
    restrictions: Vec<(Theory, Narrower)>,
    pending: Vec<PendingRegion>,
    pool: rayon::ThreadPool,
    statistics: RegionSearchStatistics,
}

impl std::fmt::Debug for RegionProposals {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegionProposals")
            .field("workers", &self.pool.current_num_threads())
            .field("pending", &self.pending.len())
            .finish_non_exhaustive()
    }
}

/// Joined production retains its completed candidates even if another region
/// stopped. Only an empty frontier with no stop establishes exhaustion.
pub(crate) struct Produced {
    pub(crate) exhausted: bool,
    pub(crate) stopped: Option<Incomplete>,
}

impl RegionProposals {
    pub(crate) fn new(
        theory: &Theory,
        workers: NonZeroUsize,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Incomplete> {
        let regions::Opened {
            producers,
            narrower,
            statistics,
        } = regions::open(theory, budget)?;
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers.get())
            .thread_name(|index| format!("zetesis-proposal-{index}"))
            .build()
            .map_err(|_| Incomplete::Allocation)?;
        let mut pending = crate::search::storage(1)?;
        pending.push((
            Region::all_open(theory.atom_count()),
            vec![narrower.knowledge()],
        ));
        Ok(Self {
            producers,
            narrower,
            restrictions: Vec::new(),
            pending,
            pool,
            statistics,
        })
    }

    pub(crate) const fn statistics(&self) -> RegionSearchStatistics {
        self.statistics
    }

    pub(crate) fn restrict(
        &mut self,
        restriction: &Theory,
        budget: &mut Budget<'_>,
    ) -> Result<(), Incomplete> {
        self.restrictions
            .try_reserve(1)
            .map_err(|_| Incomplete::Allocation)?;
        let narrower = Narrower::new(restriction);
        budget.charge(narrower.work())?;
        self.statistics.counts.work = self
            .statistics
            .counts
            .work
            .checked_add(narrower.work())
            .ok_or(Incomplete::CounterOverflow)?;
        self.restrictions.push((restriction.clone(), narrower));
        Ok(())
    }

    /// Fill pre-admitted output slots without evaluating a reduct. The output
    /// is empty on entry; its allocation remains the caller's batch ownership.
    /// A zero remaining candidate allowance still permits coverage work until
    /// either the frontier is refuted or another classical leaf is encountered.
    pub(crate) fn fill(
        &mut self,
        theory: &Theory,
        maximum: usize,
        remaining: u64,
        budget: &mut Budget<'_>,
        output: &mut Vec<Interpretation>,
    ) -> Produced {
        debug_assert!(output.is_empty());
        debug_assert!(maximum > 0 && output.capacity() >= maximum);
        let allowance = SharedBudget::new(budget.limits, budget.statistics);
        let round = Round {
            theory,
            producers: self.producers.as_ref(),
            narrower: &self.narrower,
            restrictions: &self.restrictions,
            allowance: &allowance,
            limits: budget.limits,
            control: budget.control,
            maximum: maximum.min(usize::try_from(remaining).unwrap_or(usize::MAX).max(1)),
            remaining,
            changed: Condvar::new(),
            state: Mutex::new(State {
                pending: std::mem::take(&mut self.pending),
                output: std::mem::take(output),
                active: 0,
                counts: RegionCounts::default(),
                stopped: None,
            }),
        };
        self.pool.install(|| {
            (0..self.pool.current_num_threads())
                .into_par_iter()
                .for_each(|_| {
                    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| round.work()))
                        .is_err()
                    {
                        round.stop(Incomplete::WorkerPanicked);
                    }
                });
        });
        allowance.record(&mut budget.statistics);
        let mut state = round
            .state
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner);
        if let Err(error) = self.statistics.counts.add(state.counts) {
            state.stopped.get_or_insert(error);
        }
        self.pending = state.pending;
        *output = state.output;
        Produced {
            exhausted: state.stopped.is_none() && self.pending.is_empty(),
            stopped: state.stopped,
        }
    }
}

struct State {
    pending: Vec<PendingRegion>,
    output: Vec<Interpretation>,
    /// Each active region reserves a possible output slot.
    active: usize,
    counts: RegionCounts,
    stopped: Option<Incomplete>,
}

struct Round<'a> {
    theory: &'a Theory,
    producers: Option<&'a Producers>,
    narrower: &'a Narrower,
    restrictions: &'a [(Theory, Narrower)],
    allowance: &'a SharedBudget,
    limits: crate::SearchLimits,
    control: &'a Control,
    maximum: usize,
    remaining: u64,
    changed: Condvar,
    state: Mutex<State>,
}

enum Step {
    Refuted,
    Split(PendingRegion, PendingRegion),
    Candidate(Interpretation),
}

impl Round<'_> {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn stop(&self, error: Incomplete) {
        self.lock().stopped.get_or_insert(error);
        self.changed.notify_all();
    }

    fn take(&self) -> Option<PendingRegion> {
        let mut state = self.lock();
        loop {
            if state.stopped.is_some() || state.output.len() == self.maximum {
                return None;
            }
            if let Err(error) = self.control.poll() {
                state.stopped.get_or_insert(error.into());
                self.changed.notify_all();
                return None;
            }
            if state.active + state.output.len() < self.maximum {
                if let Some(entry) = state.pending.pop() {
                    state.active += 1;
                    return Some(entry);
                }
                if state.active == 0 {
                    return None;
                }
            }
            state = self
                .changed
                .wait_timeout(state, CONTROL_WAIT)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    fn work(&self) {
        while let Some(mut entry) = self.take() {
            let mut counts = RegionCounts {
                regions: 1,
                ..Default::default()
            };
            // Settle permits before touching the frontier mutex or waiting;
            // an idle producer must not strand another worker's allowance.
            let result = {
                let mut budget = Budget {
                    quota: self.allowance.lease(self.control),
                    limits: self.limits,
                    control: self.control,
                    statistics: SearchStatistics::default(),
                };
                self.step(&mut entry, &mut budget, &mut counts)
            };
            let mut state = self.lock();
            state.active -= 1;
            if let Err(error) = state.counts.add(counts) {
                state.stopped.get_or_insert(error);
            }
            match result {
                Ok(Step::Refuted) => {}
                Ok(Step::Candidate(candidate)) => state.output.push(candidate),
                Ok(Step::Split(cut, held)) => {
                    if state.pending.try_reserve(2).is_ok() {
                        state.pending.push(held);
                        state.pending.push(cut);
                    } else {
                        state.stopped.get_or_insert(Incomplete::Allocation);
                    }
                }
                Err(error) => {
                    // This region remains unresolved. Retain it when storage
                    // permits; the explicit stop is authoritative either way.
                    if state.pending.try_reserve(1).is_ok() {
                        state.pending.push(entry);
                    }
                    state.stopped.get_or_insert(error);
                }
            }
            self.changed.notify_all();
        }
    }

    fn step<'a>(
        &'a self,
        (region, knowledge): &mut PendingRegion,
        budget: &mut Budget<'a, WorkLease<'a>>,
        counts: &mut RegionCounts,
    ) -> Result<Step, Incomplete> {
        if regions::narrow(
            (self.theory, self.narrower),
            self.producers,
            self.restrictions,
            region,
            knowledge,
            budget,
            counts,
        )? == Narrowing::Refuted
        {
            counts.refuted = 1;
            return Ok(Step::Refuted);
        }
        if let Some(atom) = region.split_atom() {
            budget.decide()?;
            let (cut, held) = region.split(atom);
            let cut = (cut, knowledge.clone());
            let held = (held, std::mem::take(knowledge));
            return Ok(Step::Split(cut, held));
        }
        if self.remaining == 0 {
            return Err(Incomplete::CandidateLimit);
        }
        counts.leaves = 1;
        regions::leaf_interpretation(self.theory, region).map(Step::Candidate)
    }
}
