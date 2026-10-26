//! Candidate slates follow completed work, bound feedback and the two existing
//! capacity owners.
//!
//! A slate is the candidates the stream proposes together for one membership
//! check and commit; `batch_size` is its initial and reset size. Under the
//! adaptive policy, a commit only records its evidence: the slate's selected
//! count, whether its proposals filled it, and the candidate bound's
//! generation. The next selection decides, after the committed slate's models
//! have been scored, because the queue hands every model of a slate out before
//! it selects again. A bound that moved since the commit was installed by that
//! slate's answers. The session's last selection follows its last non-empty
//! slate, consumes that slate's commit like any other, and precedes the empty
//! proposal that establishes exhaustion.
//!
//! A slate proposed ahead, while its predecessor is checked, takes the size
//! the selection after that predecessor's commit would take: the doubling, the
//! cap and the floor read the predecessor's proposal record (its selected
//! count, whether the stream filled it, and the device prefixes its
//! submissions admitted), and the pending-memory capacity left beside it bounds
//! the count. An enumeration declares no candidate bounds, so no reset can
//! intervene. A slate handed out ahead or as a retry keeps the count recorded
//! at its proposal.

use std::num::NonZeroUsize;

use zetesis_stable::StableModels;

use crate::{GpuBatching, SolveConfig};

/// The slate-size policy of one GPU formula queue, with the statistics owed
/// under either policy.
#[derive(Default)]
pub(super) struct Batching {
    policy: Policy,
    /// The oldest pending slate's proposal record, while a slate is pending.
    in_flight: Option<Proposal>,
    /// The proposal record of the slate proposed ahead of it.
    ahead: Option<Proposal>,
    /// Slates committed with at least one candidate. The stream's checked
    /// committed-candidate count bounds it, so it cannot wrap.
    slates: u64,
    /// The largest committed slate's candidate count.
    largest: Option<NonZeroUsize>,
    /// Selections that returned to the batch size because the bound moved
    /// since the previous commit. Each consumes one recorded commit, and the
    /// stream's checked checker-call count bounds the commits, so it cannot
    /// wrap.
    resets: u64,
}

#[derive(Default)]
enum Policy {
    /// Every slate is the batch size.
    #[default]
    Fixed,
    /// The current target, never below the batch size; the smallest shortened
    /// device prefix observed; and the last commit no selection has consumed.
    Adaptive {
        target: NonZeroUsize,
        prefix_limit: NonZeroUsize,
        last: Option<Commit>,
    },
}

/// Evidence one complete, successful commit leaves for the next selection.
/// What a pending slate's proposal established: the count selected for it, the
/// candidates the stream supplied, and the shortest device prefix its
/// submissions admitted below what they were offered.
#[derive(Clone, Copy)]
struct Proposal {
    selected: NonZeroUsize,
    proposed: usize,
    prefix: Option<NonZeroUsize>,
}

#[derive(Clone, Copy)]
struct Commit {
    /// The count selected for the committed slate.
    selected: NonZeroUsize,
    /// Whether the slate's proposals filled that count.
    filled: bool,
    /// The candidate bound's generation at the commit.
    generation: u64,
}

impl Batching {
    pub(super) fn new(options: &SolveConfig) -> Self {
        Self {
            policy: match options.gpu_batching {
                GpuBatching::Fixed => Policy::Fixed,
                GpuBatching::Adaptive => Policy::Adaptive {
                    target: options.batch_size,
                    prefix_limit: NonZeroUsize::MAX,
                    last: None,
                },
            },
            ..Self::default()
        }
    }

    /// The count the next slate requests. A retry of pending proposals keeps
    /// the target and changes nothing. Otherwise the last commit, if any, is
    /// consumed: a bound generation that moved since it resets the target to
    /// the batch size; an unchanged one doubles a filled slate's count, up to
    /// the device prefix but never below the batch size; a short slate keeps
    /// the target. The pending-memory capacity bounds the count selected.
    pub(super) fn select(&mut self, models: &StableModels, options: &SolveConfig) -> NonZeroUsize {
        let Policy::Adaptive {
            target,
            prefix_limit,
            last,
        } = &mut self.policy
        else {
            return options.batch_size;
        };
        // Retried proposals keep their original count. The normal batch
        // admission still checks changed limits and retained capacity.
        if models.batch_statistics().pending != 0 {
            return *target;
        }
        let base = options.batch_size;
        let cap = (*prefix_limit).max(base);
        if let Some(commit) = last.take() {
            *target = if models.bound_generation() != commit.generation {
                self.resets += 1;
                base
            } else if commit.filled {
                commit
                    .selected
                    .checked_mul(NonZeroUsize::new(2).unwrap())
                    .unwrap_or(NonZeroUsize::MAX)
                    .max(base)
                    .min(cap)
            } else {
                (*target).min(cap)
            };
        }
        let memory = NonZeroUsize::new(models.joint_batch_capacity(options.max_batch_bytes))
            .unwrap_or(NonZeroUsize::MIN);
        // If even one cannot fit, ordinary admission reports its typed
        // refusal; a sizing observation cannot claim exhaustion.
        (*target).min(memory)
    }

    /// Record the slate the stream just proposed under `selected` with
    /// `proposed` candidates as the oldest pending slate.
    pub(super) fn proposed(&mut self, selected: NonZeroUsize, proposed: usize) {
        self.in_flight = Some(Proposal {
            selected,
            proposed,
            prefix: None,
        });
    }

    /// Record the slate proposed ahead of the oldest pending one.
    pub(super) fn proposed_ahead(&mut self, selected: NonZeroUsize, proposed: usize) {
        self.ahead = Some(Proposal {
            selected,
            proposed,
            prefix: None,
        });
    }

    /// The slate proposed ahead became the oldest pending slate.
    pub(super) fn promoted(&mut self) {
        self.in_flight = self.ahead.take();
    }

    /// A submission of the oldest pending slate admitted `admitted` of the
    /// `offered` candidates left in it.
    pub(super) fn admitted(&mut self, offered: usize, admitted: usize) {
        if admitted < offered
            && let (Some(proposal), Some(admitted)) =
                (self.in_flight.as_mut(), NonZeroUsize::new(admitted))
        {
            proposal.prefix = Some(
                proposal
                    .prefix
                    .map_or(admitted, |prior| prior.min(admitted)),
            );
        }
    }

    /// The count a slate proposed ahead of the oldest pending one requests: the
    /// selection after that slate's commit would take it from its proposal
    /// record, and the pending-memory capacity beside it bounds it. The target
    /// becomes that selection, as after the commit.
    pub(super) fn select_lookahead(
        &mut self,
        models: &StableModels,
        options: &SolveConfig,
    ) -> NonZeroUsize {
        let memory = NonZeroUsize::new(models.lookahead_capacity(options.max_batch_bytes))
            .unwrap_or(NonZeroUsize::MIN);
        let Policy::Adaptive {
            target,
            prefix_limit,
            ..
        } = &mut self.policy
        else {
            return options.batch_size.min(memory);
        };
        let Some(current) = self.in_flight else {
            return (*target).min(memory);
        };
        let base = options.batch_size;
        let observed = current
            .prefix
            .map_or(*prefix_limit, |prefix| (*prefix_limit).min(prefix));
        let cap = observed.max(base);
        *target = if current.proposed == current.selected.get() {
            current
                .selected
                .checked_mul(NonZeroUsize::new(2).unwrap())
                .unwrap_or(NonZeroUsize::MAX)
                .max(base)
                .min(cap)
        } else {
            current.selected.min(cap)
        };
        (*target).min(memory)
    }

    /// Commit the oldest pending slate's proposal record at bound
    /// `generation`, as [`Self::committed`] records a closure-form slate.
    pub(super) fn committed_in_flight(&mut self, generation: u64) {
        if let Some(proposal) = self.in_flight.take() {
            self.committed(
                proposal.selected,
                proposal.proposed,
                proposal.prefix,
                generation,
            );
        }
    }

    /// Record a slate of `selected` candidates, `proposed` of which the stream
    /// supplied, committed at bound `generation`, with any shortened device
    /// prefix the checker observed. Called only after complete membership and
    /// commit, never after a partial device prefix or a failed completion. The
    /// empty proposal that establishes exhaustion commits no slate.
    pub(super) fn committed(
        &mut self,
        selected: NonZeroUsize,
        proposed: usize,
        observed_prefix: Option<NonZeroUsize>,
        generation: u64,
    ) {
        if let Some(proposed) = NonZeroUsize::new(proposed) {
            self.slates += 1;
            self.largest = Some(
                self.largest
                    .map_or(proposed, |largest| largest.max(proposed)),
            );
        }
        let Policy::Adaptive {
            prefix_limit, last, ..
        } = &mut self.policy
        else {
            return;
        };
        if let Some(observed) = observed_prefix {
            *prefix_limit = (*prefix_limit).min(observed);
        }
        *last = Some(Commit {
            selected,
            filled: proposed == selected.get(),
            generation,
        });
    }

    /// Slates committed with at least one candidate.
    pub(super) const fn slates(&self) -> u64 {
        self.slates
    }

    /// The largest committed slate's candidate count; absent before the first.
    pub(super) const fn largest(&self) -> Option<NonZeroUsize> {
        self.largest
    }

    /// Selections that returned to the batch size on bound feedback.
    pub(super) const fn resets(&self) -> u64 {
        self.resets
    }
}

#[cfg(test)]
mod tests;
