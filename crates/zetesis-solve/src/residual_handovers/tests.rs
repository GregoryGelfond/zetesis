use super::*;
use zetesis_algebra::region::Region;
use zetesis_wgpu::ResidualReason;

#[test]
fn retry_preserves_an_incomplete_attempt_and_commits_the_slate_once() {
    let mut stats = ResidualHandoverStatistics::default();
    stats.begin(9, false);
    stats.decoded(ResidualReason::RoundLimit, 1);
    stats.finish(false);
    let first = stats.last_attempt.as_ref().unwrap();
    assert_eq!((first.ordinal, first.slate_sequence), (1, 9));
    assert!(!first.decode_complete && !first.committed);
    stats.begin(9, true);
    stats.decoded(ResidualReason::FixedPoint, 1);
    stats.last_attempt.as_mut().unwrap().decode_complete = true;
    stats.finish(true);
    let retry = stats.last_attempt.as_ref().unwrap();
    assert_eq!((retry.ordinal, retry.slate_sequence), (2, 9));
    assert!(retry.retry && retry.committed);
    assert_eq!(
        (
            stats.attempts,
            stats.incomplete_attempts,
            stats.committed_slates
        ),
        (2, 1, 1)
    );
    assert_eq!(stats.zero_residual_slates, 0);
}

#[test]
fn open_atom_sum_counts_only_semantic_bits_across_word_boundaries() {
    let mut stats = ResidualHandoverStatistics::default();
    stats.begin(0, false);
    let mut root = Region::all_open(65);
    assert!(root.cut(0) && root.hold(64));
    stats.decoded(ResidualReason::WorkLimit, open_atoms(&root));
    stats.decoded(ResidualReason::RoundLimit, 0);
    let attempt = stats.last_attempt.unwrap();
    assert_eq!(attempt.open_atoms, 63);
    assert_eq!(
        attempt.residuals,
        FormulaResidualStatistics {
            fixed_point: 0,
            work_limit: 1,
            round_limit: 1,
        }
    );
}

#[test]
fn unavailable_entered_work_keeps_the_known_prefix() {
    let mut stats = ResidualHandoverStatistics {
        query_work: 7,
        ..Default::default()
    };
    stats.begin(0, false);
    stats.decoded(ResidualReason::FixedPoint, 0);
    stats.join(None, &decoded::DecodeJournal::default(), true);
    stats.finish(false);
    assert_eq!(stats.query_work, 7);
    assert!(!stats.complete_work);
    assert!(stats.last_attempt.unwrap().search.is_none());
}

#[test]
fn a_committed_empty_handover_has_measured_zero_residuals() {
    let mut stats = ResidualHandoverStatistics::default();
    stats.begin(0, false);
    stats.last_attempt.as_mut().unwrap().decode_complete = true;
    stats.finish(true);
    assert_eq!(
        (
            stats.attempts,
            stats.committed_slates,
            stats.zero_residual_slates
        ),
        (1, 1, 1)
    );
    assert!(stats.complete_work);
}

#[test]
fn joined_work_belongs_to_each_original_decoded_slot() {
    use std::num::NonZeroUsize;
    use zetesis_algebra::Cancellation;
    use zetesis_ferraris::{AdmissionLimits, FormulaParts, Node, Theory};
    use zetesis_stable::{
        BatchLimits, BatchVerdict, CompletionExecutor, Limits, SlateProposal, StableModels,
    };

    let theory = Theory::new(
        2,
        FormulaParts::new(
            vec![Node::atom(0), Node::atom(1), Node::or_pair([0, 1])],
            vec![],
        )
        .unwrap(),
        vec![2],
        AdmissionLimits::default(),
    )
    .unwrap();
    for n in [1, 2] {
        let mut models =
            StableModels::new(&theory, Limits::default(), Cancellation::default()).unwrap();
        let mut executor = CompletionExecutor::new(NonZeroUsize::new(n).unwrap()).unwrap();
        let SlateProposal::New(ticket) = models
            .propose_slate(BatchLimits {
                max_candidates: NonZeroUsize::new(8).unwrap(),
                max_pending_bytes: u64::MAX,
            })
            .unwrap()
        else {
            panic!("expected a candidate slate")
        };
        let candidates = models.slate(&ticket).unwrap().candidates();
        assert!(candidates.len() >= 2);
        let mut stats = ResidualHandoverStatistics::default();
        let mut decoded = decoded::DecodeJournal::default();
        let mut roots = vec![];
        stats.begin(ticket.slate().sequence(), false);
        for (slot, candidate) in candidates.iter().enumerate() {
            let mut root = Region::all_open(2);
            for atom in 0..2 {
                if !candidate.contains(atom) {
                    assert!(root.cut(atom));
                }
            }
            let reason = if slot == 0 {
                ResidualReason::RoundLimit
            } else {
                ResidualReason::FixedPoint
            };
            let open = open_atoms(&root);
            decoded.record(slot, candidates.len(), reason, open, u64::MAX);
            stats.decoded(reason, open);
            roots.push((slot, root));
        }
        let tags = vec![BatchVerdict::Residual; roots.len()];
        executor.observe_next_regional_pass(1, u64::MAX);
        let pass = models
            .begin_regional_pass(ticket, tags, roots, &mut executor)
            .unwrap();
        let receipt = executor.take_regional_observations().unwrap();
        stats.join(Some(&receipt), &decoded, true);
        let attempt = stats.last_attempt.as_ref().unwrap();
        let groups = attempt.by_reason.as_ref().unwrap();
        let search = attempt.search.as_ref().unwrap();
        let first = receipt.slots().unwrap()[0].as_ref().unwrap();
        assert_eq!(
            groups.round_limit.work,
            first.query.map_or(0, |q| u128::from(q.work))
        );
        assert_eq!(
            groups.round_limit.open_atoms,
            u128::from(decoded.rows().unwrap()[0].open_atoms)
        );
        assert_eq!(
            groups.round_limit.work + groups.fixed_point.work,
            search.totals.work
        );
        assert_eq!(stats.query_work, search.totals.work);
        assert_eq!(
            groups.round_limit.visited + groups.fixed_point.visited,
            search.totals.visited
        );
        assert_eq!(
            groups.round_limit.open_atoms + groups.fixed_point.open_atoms,
            attempt.open_atoms
        );
        assert_eq!(groups.work_limit.visited, 0);
        drop(pass);
    }
}
