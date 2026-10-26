use std::convert::Infallible;
use std::num::NonZeroUsize;

use zetesis_ferraris::{Interpretation, Theory};

use super::{
    BatchError, BatchLimits, BatchVerdict, Lookahead, LookaheadDeclined, OriginalCheck,
    SlateProposal, SlateTicket, hooks,
};
use crate::ferraris::fixtures::choice_beside_a_loop;
use crate::ferraris::region_proposals::Produced;
use crate::{Cancellation, CompletionExecutor, Incomplete, Limits, PhaseMeasurement, StableModels};

fn one_producer(theory: &Theory) -> StableModels {
    StableModels::with_region_producers(
        theory,
        NonZeroUsize::new(1).unwrap(),
        Limits::default(),
        Cancellation::default(),
    )
    .unwrap()
}

fn one_candidate() -> BatchLimits {
    BatchLimits {
        max_candidates: NonZeroUsize::new(1).unwrap(),
        max_pending_bytes: 4096,
    }
}

fn new_slate(search: &mut StableModels, limits: BatchLimits) -> SlateTicket {
    match search.propose_slate(limits).unwrap() {
        SlateProposal::New(ticket) => ticket,
        other => panic!("expected a new slate, found {other:?}"),
    }
}

fn residuals(search: &StableModels, ticket: &SlateTicket) -> Vec<BatchVerdict> {
    vec![BatchVerdict::Residual; search.slate(ticket).unwrap().candidates().len()]
}

/// A ticket can be obtained only for the oldest pending slate, so only a
/// forged one names the slate proposed ahead.
#[test]
fn commit_slate_refuses_a_ticket_for_the_slate_proposed_ahead() {
    let theory = choice_beside_a_loop();
    let mut search = one_producer(&theory);
    let first = new_slate(&mut search, one_candidate());
    let Lookahead::Proposed(ahead) = search.propose_lookahead(one_candidate()).unwrap() else {
        panic!("expected a slate proposed ahead");
    };
    let forged = SlateTicket { slate: ahead };
    assert!(matches!(
        search.commit_slate(
            forged,
            vec![BatchVerdict::Residual],
            &mut CompletionExecutor::default()
        ),
        Err(BatchError::SlateOrder)
    ));
    assert_eq!(search.batch_statistics().committed, 0);
    let verdicts = residuals(&search, &first);
    search
        .commit_slate(first, verdicts, &mut CompletionExecutor::default())
        .unwrap();
    assert_eq!(search.batch_statistics().committed, 1);
}

fn commit(search: &mut StableModels, ticket: SlateTicket) -> Vec<Interpretation> {
    let verdicts = residuals(search, &ticket);
    search
        .commit_slate(ticket, verdicts, &mut CompletionExecutor::default())
        .unwrap()
}

#[test]
fn an_allocator_refusal_preparing_a_lookahead_declines_it() {
    let theory = choice_beside_a_loop();
    let mut search = one_producer(&theory);
    let _first = new_slate(&mut search, one_candidate());
    hooks::refuse_next_lookahead_reservation();
    assert_eq!(
        search.propose_lookahead(one_candidate()).unwrap(),
        Lookahead::Declined(LookaheadDeclined::Allocation)
    );
    assert_eq!(search.batch_statistics().lookahead_slates, 0);
}

/// Nothing is held after the decline, so the proposal after the commit
/// reserves and produces its slate as the serial schedule's does.
#[test]
fn the_proposal_after_an_allocation_decline_reserves_its_own_slate() {
    let theory = choice_beside_a_loop();
    let mut search = one_producer(&theory);
    let first = new_slate(&mut search, one_candidate());
    hooks::refuse_next_lookahead_reservation();
    assert_eq!(
        search.propose_lookahead(one_candidate()).unwrap(),
        Lookahead::Declined(LookaheadDeclined::Allocation)
    );
    commit(&mut search, first);
    assert!(matches!(
        search.propose_slate(one_candidate()).unwrap(),
        SlateProposal::New(_)
    ));
}

/// The walk's index failure, which a lookahead's walk cannot meet since it
/// reads the index the pending slate's proposal built, is held as the
/// lookahead's stop: the stream stays open, the pending slate commits, and
/// the stop is reported by the proposal after that commit.
#[test]
fn a_walk_failure_in_a_lookahead_is_held_until_the_pending_slate_commits() {
    let theory = choice_beside_a_loop();
    let mut search = one_producer(&theory);
    let first = new_slate(&mut search, one_candidate());
    hooks::fail_next_walk();
    assert_eq!(
        search.propose_lookahead(one_candidate()).unwrap(),
        Lookahead::Stopped
    );
    assert_eq!(commit(&mut search, first).len(), 1);
    assert!(matches!(
        search.propose_slate(one_candidate()),
        Err(BatchError::Search(Incomplete::Allocation))
    ));
    assert!(!search.exhausted());
}

/// An overflow counting the slate proposed ahead is held beside its
/// candidates: the slate is handed out and committed, and the overflow is
/// reported after it.
#[test]
fn an_overflow_counting_a_lookahead_is_held_beside_its_candidates() {
    let theory = choice_beside_a_loop();
    let mut search = one_producer(&theory);
    let first = new_slate(&mut search, one_candidate());
    search.batch.statistics.lookahead_slates = u64::MAX;
    assert!(matches!(
        search.propose_lookahead(one_candidate()).unwrap(),
        Lookahead::Proposed(_)
    ));
    commit(&mut search, first);
    let SlateProposal::Ahead(ahead) = search.propose_slate(one_candidate()).unwrap() else {
        panic!("expected the slate proposed ahead");
    };
    assert_eq!(search.slate(&ahead).unwrap().candidates().len(), 1);
    commit(&mut search, ahead);
    assert!(matches!(
        search.propose_slate(one_candidate()),
        Err(BatchError::Search(Incomplete::CounterOverflow))
    ));
    assert!(!search.exhausted());
}

/// A pending slate whose own production exhausted the frontier is followed
/// by an empty lookahead, and exhaustion waits for that slate's commit. The
/// exhausted production is handed over directly, as the region producers
/// report it at a frontier's end.
#[test]
fn a_lookahead_beside_an_exhausting_slate_establishes_exhaustion_only_after_its_commit() {
    let theory = choice_beside_a_loop();
    let mut search = one_producer(&theory);
    let first = new_slate(&mut search, one_candidate());
    search.batch.exhausted = true;
    assert_eq!(
        search.propose_lookahead(one_candidate()).unwrap(),
        Lookahead::Exhausted
    );
    assert!(!search.exhausted());
    let verdicts = residuals(&search, &first);
    search
        .commit_slate(first, verdicts, &mut CompletionExecutor::default())
        .unwrap();
    assert!(matches!(
        search.propose_slate(one_candidate()).unwrap(),
        SlateProposal::Exhausted
    ));
    assert!(search.exhausted());
}

#[test]
fn a_lookahead_that_does_not_fit_beside_the_pending_slate_is_declined_for_pending_bytes() {
    let theory = choice_beside_a_loop();
    let mut search = one_producer(&theory);
    // One slate's row beside the validation workspace: the pending slate
    // fits, a second does not.
    let limits = BatchLimits {
        max_candidates: NonZeroUsize::new(1).unwrap(),
        max_pending_bytes: u64::try_from(
            search.pending_row_bytes() + search.batch.validation.retained_bytes(),
        )
        .unwrap(),
    };
    let _first = new_slate(&mut search, limits);
    assert_eq!(search.lookahead_capacity(limits.max_pending_bytes), 0);
    assert_eq!(
        search.propose_lookahead(limits).unwrap(),
        Lookahead::Declined(LookaheadDeclined::PendingBytes)
    );
    assert_eq!(search.batch_statistics().lookahead_slates, 0);
}

/// A slate that filled and then stopped, as one does when a producer's step
/// fails after the filling emission: its candidates are checked and
/// committed, and the stop is reported by the next call, never exhaustion.
/// The join is handed over directly, since no schedule of real producers
/// reaches that interleaving on demand.
#[test]
fn a_full_slate_beside_a_stop_commits_its_candidates_before_the_stop() {
    let theory = choice_beside_a_loop();
    let mut search = StableModels::with_region_producers(
        &theory,
        NonZeroUsize::new(2).unwrap(),
        Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    let limits = BatchLimits {
        max_candidates: NonZeroUsize::new(2).unwrap(),
        max_pending_bytes: 4096,
    };
    // Two of the theory's answer sets fill the slate, admitted as the proposal
    // step admits a joined slate's output, after building the original index
    // its producers read and its completion borrows.
    let available = search.prepare_pending(limits).unwrap();
    search.batch.original_checked = true;
    search.walk_index().unwrap();
    let mut pending = std::mem::take(&mut search.batch.pending);
    for atoms in [[1], [2]] {
        pending.push(Interpretation::new(&theory, atoms).unwrap());
    }
    let produced = Produced {
        exhausted: false,
        stopped: Some(Incomplete::WorkLimit),
        original_validation: PhaseMeasurement::default(),
    };
    let production = search.admit_produced(
        &produced,
        available,
        OriginalCheck::Independent,
        &mut pending,
    );
    search.batch.pending = pending;
    search.batch.exhausted = production.exhausted;
    search.pending_error = production.stop;
    let residual = |_: &Theory, candidates: &[Interpretation]| {
        Ok::<_, Infallible>(vec![BatchVerdict::Residual; candidates.len()])
    };
    assert_eq!(search.next_batch(limits, residual).unwrap().len(), 2);
    assert!(matches!(
        search.next_batch(limits, residual),
        Err(BatchError::Search(Incomplete::WorkLimit))
    ));
    assert!(!search.exhausted());
}
