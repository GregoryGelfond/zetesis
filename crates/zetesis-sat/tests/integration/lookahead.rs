//! A slate proposed ahead of its predecessor's commit, by the two-phase batch protocol.

use std::convert::Infallible;

use crate::support::batching::{batch as limits, residual};
use crate::support::choice_theories::three_choices;
use zetesis_ferraris::{Interpretation, Theory};
use zetesis_sat::{
    BatchError, BatchVerdict, Cancellation, CompletionExecutor, Incomplete, Limits, Lookahead,
    LookaheadDeclined, SlateProposal, SlateTicket, StableModels,
};

/// A one-producer stream over `owner`. Every interpretation of the fixture
/// satisfies its original theory, so a residual verdict meets the joint
/// protocol's obligation, and exact completion decides each candidate.
fn stream(owner: &Theory, limits: Limits) -> StableModels {
    StableModels::with_region_producers(
        owner,
        zetesis_test_support::counts::nonzero(1),
        limits,
        Cancellation::default(),
    )
    .unwrap()
}

fn ticket(proposal: SlateProposal) -> SlateTicket {
    match proposal {
        SlateProposal::New(ticket)
        | SlateProposal::Ahead(ticket)
        | SlateProposal::Retry(ticket) => ticket,
        other => panic!("expected a slate, found {other:?}"),
    }
}

fn residuals(search: &StableModels, ticket: &SlateTicket) -> Vec<BatchVerdict> {
    vec![BatchVerdict::Residual; search.slate(ticket).unwrap().candidates().len()]
}

fn commit(search: &mut StableModels, ticket: SlateTicket) -> Vec<Interpretation> {
    let verdicts = residuals(search, &ticket);
    search
        .commit_slate(ticket, verdicts, &mut CompletionExecutor::default())
        .unwrap()
}

fn atoms(candidates: &[Interpretation]) -> Vec<Vec<usize>> {
    candidates
        .iter()
        .map(|candidate| candidate.atoms().collect())
        .collect()
}

#[test]
fn a_stream_expecting_candidate_bounds_declines_the_lookahead() {
    let owner = three_choices();
    let mut search = stream(&owner, Limits::default());
    search.expect_candidate_bounds().unwrap();
    let _first = ticket(search.propose_slate(limits(3)).unwrap());
    assert_eq!(
        search.propose_lookahead(limits(3)).unwrap(),
        Lookahead::Declined(LookaheadDeclined::CandidateBounds)
    );
}

#[test]
fn a_slate_handed_out_for_the_independent_check_declines_the_lookahead() {
    let owner = three_choices();
    let mut search = stream(&owner, Limits::default());
    let failed = search.next_batch(limits(3), |_: &Theory, _: &[Interpretation]| {
        Err::<Vec<BatchVerdict>, _>(())
    });
    assert!(matches!(failed, Err(BatchError::Checker(()))));
    assert_eq!(
        search.propose_lookahead(limits(3)).unwrap(),
        Lookahead::Declined(LookaheadDeclined::IndependentCheck)
    );
}

#[test]
fn a_stream_with_no_pending_slate_declines_the_lookahead() {
    let owner = three_choices();
    let mut search = stream(&owner, Limits::default());
    assert_eq!(
        search.propose_lookahead(limits(3)).unwrap(),
        Lookahead::Declined(LookaheadDeclined::NoPendingSlate)
    );
}

#[test]
fn a_second_lookahead_beside_one_proposed_ahead_is_declined() {
    let owner = three_choices();
    let mut search = stream(&owner, Limits::default());
    let _first = ticket(search.propose_slate(limits(3)).unwrap());
    assert!(matches!(
        search.propose_lookahead(limits(3)).unwrap(),
        Lookahead::Proposed(_)
    ));
    assert_eq!(
        search.propose_lookahead(limits(3)).unwrap(),
        Lookahead::Declined(LookaheadDeclined::AlreadyProposed)
    );
}

#[test]
fn a_pending_slate_holding_a_stop_declines_the_lookahead() {
    let owner = three_choices();
    // Two candidates for the stream, three per slate: the first slate's
    // production stops at the stream's candidate limit and holds the stop.
    let mut search = stream(
        &owner,
        Limits {
            max_candidates: 2,
            ..Limits::default()
        },
    );
    let _first = ticket(search.propose_slate(limits(3)).unwrap());
    assert_eq!(
        search.propose_lookahead(limits(3)).unwrap(),
        Lookahead::Declined(LookaheadDeclined::PendingStop)
    );
}

#[test]
fn a_slate_proposed_ahead_becomes_pending_only_after_its_predecessor_commits() {
    let owner = three_choices();
    let mut search = stream(&owner, Limits::default());
    let first = ticket(search.propose_slate(limits(3)).unwrap());
    let first_atoms = atoms(search.slate(&first).unwrap().candidates());
    let Lookahead::Proposed(ahead) = search.propose_lookahead(limits(3)).unwrap() else {
        panic!("expected a slate proposed ahead");
    };
    // Both slates are pending, and the oldest is still the first.
    assert_eq!(search.batch_statistics().pending, 6);
    assert_eq!(
        atoms(search.slate(&first).unwrap().candidates()),
        first_atoms
    );
    assert_eq!(atoms(&commit(&mut search, first)), first_atoms);
    assert_eq!(search.batch_statistics().pending, 3);
    let second = ticket(search.propose_slate(limits(3)).unwrap());
    assert_eq!(second.slate(), ahead);
}

#[test]
fn propose_lookahead_issues_no_ticket_and_ahead_hands_out_the_first() {
    let owner = three_choices();
    let mut search = stream(&owner, Limits::default());
    let first = ticket(search.propose_slate(limits(3)).unwrap());
    let Lookahead::Proposed(ahead) = search.propose_lookahead(limits(3)).unwrap() else {
        panic!("expected a slate proposed ahead");
    };
    assert_eq!(ahead.sequence(), first.slate().sequence() + 1);
    commit(&mut search, first);
    match search.propose_slate(limits(3)).unwrap() {
        SlateProposal::Ahead(ticket) => assert_eq!(ticket.slate(), ahead),
        other => panic!("expected the slate proposed ahead, found {other:?}"),
    }
}

#[test]
fn an_uncommitted_check_keeps_both_slates_retryable_in_order() {
    let owner = three_choices();
    let mut search = stream(&owner, Limits::default());
    let first = ticket(search.propose_slate(limits(3)).unwrap());
    let Lookahead::Proposed(ahead) = search.propose_lookahead(limits(3)).unwrap() else {
        panic!("expected a slate proposed ahead");
    };
    // The caller's check of the first slate did not commit: it is handed out
    // again, and the slate proposed ahead waits behind it.
    let again = match search.propose_slate(limits(3)).unwrap() {
        SlateProposal::Retry(ticket) => ticket,
        other => panic!("expected a retry, found {other:?}"),
    };
    assert_eq!(again.slate(), first.slate());
    commit(&mut search, again);
    match search.propose_slate(limits(3)).unwrap() {
        SlateProposal::Ahead(ticket) => assert_eq!(ticket.slate(), ahead),
        other => panic!("expected the slate proposed ahead, found {other:?}"),
    }
}

#[test]
fn a_ticket_from_another_stream_is_refused() {
    let owner = three_choices();
    let mut one = stream(&owner, Limits::default());
    let mut other = stream(&owner, Limits::default());
    let foreign = ticket(one.propose_slate(limits(3)).unwrap());
    let _own = ticket(other.propose_slate(limits(3)).unwrap());
    assert!(matches!(other.slate(&foreign), Err(BatchError::SlateOrder)));
    assert!(matches!(
        other.commit_slate(
            foreign,
            vec![BatchVerdict::Residual; 3],
            &mut CompletionExecutor::default()
        ),
        Err(BatchError::SlateOrder)
    ));
    assert_eq!(other.batch_statistics().committed, 0);
}

#[test]
fn a_ticket_for_a_committed_slate_is_refused() {
    let owner = three_choices();
    let mut search = stream(&owner, Limits::default());
    let first = ticket(search.propose_slate(limits(3)).unwrap());
    let again = ticket(search.propose_slate(limits(3)).unwrap());
    commit(&mut search, again);
    assert!(matches!(
        search.commit_slate(
            first,
            vec![BatchVerdict::Residual; 3],
            &mut CompletionExecutor::default()
        ),
        Err(BatchError::SlateOrder)
    ));
    assert_eq!(search.batch_statistics().committed, 3);
}

#[test]
fn an_empty_lookahead_establishes_exhaustion_only_after_the_pending_slate_commits() {
    let owner = three_choices();
    let mut search = stream(&owner, Limits::default());
    let first = ticket(search.propose_slate(limits(4)).unwrap());
    assert!(matches!(
        search.propose_lookahead(limits(4)).unwrap(),
        Lookahead::Proposed(_)
    ));
    let mut answers = commit(&mut search, first).len();
    let second = ticket(search.propose_slate(limits(4)).unwrap());
    // The eight candidates are all pending or committed: the next proposal is empty.
    assert_eq!(
        search.propose_lookahead(limits(4)).unwrap(),
        Lookahead::Exhausted
    );
    assert!(!search.exhausted());
    answers += commit(&mut search, second).len();
    assert!(matches!(
        search.propose_slate(limits(4)).unwrap(),
        SlateProposal::Exhausted
    ));
    assert!(search.exhausted());
    assert_eq!(answers, 8);
}

#[test]
fn a_lookahead_production_stop_is_reported_after_the_pending_commit() {
    let owner = three_choices();
    // Five candidates for the stream: the slate proposed ahead takes two and stops.
    let mut search = stream(
        &owner,
        Limits {
            max_candidates: 5,
            ..Limits::default()
        },
    );
    let first = ticket(search.propose_slate(limits(3)).unwrap());
    assert!(matches!(
        search.propose_lookahead(limits(3)).unwrap(),
        Lookahead::Proposed(_)
    ));
    assert_eq!(commit(&mut search, first).len(), 3);
    let partial = ticket(search.propose_slate(limits(3)).unwrap());
    assert_eq!(commit(&mut search, partial).len(), 2);
    assert!(matches!(
        search.propose_slate(limits(3)),
        Err(BatchError::Search(Incomplete::CandidateLimit))
    ));
    assert!(!search.exhausted());
}

#[test]
fn a_lookahead_stopped_before_its_first_candidate_holds_its_stop_until_the_pending_commit() {
    let owner = three_choices();
    // Three candidates for the stream fill the first slate exactly.
    let mut search = stream(
        &owner,
        Limits {
            max_candidates: 3,
            ..Limits::default()
        },
    );
    let first = ticket(search.propose_slate(limits(3)).unwrap());
    assert_eq!(
        search.propose_lookahead(limits(3)).unwrap(),
        Lookahead::Stopped
    );
    assert_eq!(commit(&mut search, first).len(), 3);
    assert!(matches!(
        search.propose_slate(limits(3)),
        Err(BatchError::Search(Incomplete::CandidateLimit))
    ));
    assert!(!search.exhausted());
}

#[test]
fn a_search_error_at_commit_ends_the_stream_without_exhaustion() {
    let owner = three_choices();
    let mut search = stream(&owner, Limits::default());
    let first = ticket(search.propose_slate(limits(3)).unwrap());
    assert!(matches!(
        search.propose_lookahead(limits(3)).unwrap(),
        Lookahead::Proposed(_)
    ));
    assert!(matches!(
        search.commit_slate(
            first,
            vec![BatchVerdict::NotModel; 3],
            &mut CompletionExecutor::default()
        ),
        Err(BatchError::Search(Incomplete::InvalidWitness))
    ));
    assert!(matches!(
        search.propose_slate(limits(3)),
        Err(BatchError::Search(Incomplete::ClosedEnumerator))
    ));
    assert!(!search.exhausted());
}

#[test]
fn scalar_iteration_is_refused_beside_a_slate_proposed_ahead() {
    let owner = three_choices();
    let mut search = stream(&owner, Limits::default());
    let _first = ticket(search.propose_slate(limits(3)).unwrap());
    assert!(matches!(
        search.propose_lookahead(limits(3)).unwrap(),
        Lookahead::Proposed(_)
    ));
    assert!(matches!(search.next(), Some(Err(Incomplete::PendingBatch))));
}

#[test]
fn the_closure_form_checks_the_oldest_pending_slate_before_one_proposed_ahead() {
    let owner = three_choices();
    let mut search = stream(&owner, Limits::default());
    let first = ticket(search.propose_slate(limits(3)).unwrap());
    let first_atoms = atoms(search.slate(&first).unwrap().candidates());
    assert!(matches!(
        search.propose_lookahead(limits(3)).unwrap(),
        Lookahead::Proposed(_)
    ));
    let mut completion = CompletionExecutor::default();
    let mut checked = Vec::new();
    for _ in 0..2 {
        search
            .next_batch_with_joint_completion(limits(3), &mut completion, |_, candidates| {
                checked.push(atoms(candidates));
                Ok::<_, Infallible>(vec![BatchVerdict::Residual; candidates.len()])
            })
            .unwrap();
    }
    assert_eq!(checked[0], first_atoms);
    assert_eq!(checked[1].len(), 3);
    assert!(checked[1].iter().all(|atoms| !first_atoms.contains(atoms)));
}

/// The closure forms' receipts on this fixture, as the protocol recorded them
/// before the two-phase form existed.
#[test]
fn the_closure_forms_receipts_are_unchanged() {
    let owner = three_choices();
    for (joint, validation_work) in [(true, 0), (false, 188)] {
        let mut search = StableModels::with_region_producers(
            &owner,
            zetesis_test_support::counts::nonzero(1),
            Limits::default(),
            Cancellation::default(),
        )
        .unwrap();
        let mut completion = CompletionExecutor::default();
        let mut answers = 0;
        while !search.exhausted() {
            let batch = if joint {
                search.next_batch_with_joint_completion(limits(3), &mut completion, residual)
            } else {
                search.next_batch_with_completion(limits(3), &mut completion, residual)
            };
            answers += batch.unwrap().len();
        }
        let batches = search.batch_statistics();
        assert_eq!(answers, 8);
        assert_eq!(batches.checker_calls, 3);
        assert_eq!(batches.completion_calls, 3);
        assert_eq!(batches.committed, 8);
        assert_eq!(batches.propagated, 0);
        assert_eq!(batches.residuals, 8);
        assert_eq!(batches.pending, 0);
        assert_eq!(batches.lookahead_slates, 0);
        assert_eq!(batches.original_validation_work, validation_work);
        assert_eq!(search.statistics().candidates, 8);
        assert_eq!(search.statistics().stable_models, 8);
    }
}
