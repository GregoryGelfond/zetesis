use super::*;

#[test]
fn retained_roots_remain_live_across_submissions_and_lookahead() {
    let pending = 17;
    let ceiling = 1_000_000;
    let mut rows = CheckedRows::new(2, Some((ceiling, pending, 0)))
        .unwrap_or_else(|_| panic!("expected admitted regional operation"));
    let available = rows
        .remaining(ceiling, pending, 0)
        .unwrap_or_else(|_| panic!("expected admitted regional operation"));
    let per_root = 2048;
    let device = available - per_root + 1;
    assert!(rows.remaining(ceiling, pending, device).is_ok());
    rows.projected(Region::all_open(8192))
        .unwrap_or_else(|_| panic!("expected admitted regional operation"));
    assert!(matches!(
        rows.remaining(ceiling, pending, device),
        Err(Failure::Search(Incomplete::PendingBytes))
    ));
    assert_eq!(
        rows.remaining(ceiling + 1, pending, device)
            .unwrap_or_else(|_| panic!("expected admitted regional operation")),
        0
    );
    assert_eq!(
        u128::from(
            rows.remaining(ceiling + 1, 0, device)
                .unwrap_or_else(|_| panic!("expected admitted regional operation"))
        ),
        pending
    );
}

#[test]
fn terminal_slots_keep_the_next_submissions_full_allowance() {
    let mut rows = CheckedRows::new(2, Some((4096, 17, 0)))
        .unwrap_or_else(|_| panic!("expected admitted regional operation"));
    let initial = rows
        .remaining(4096, 17, 0)
        .unwrap_or_else(|_| panic!("expected admitted regional operation"));
    rows.classified(BatchVerdict::NoProperSubset)
        .unwrap_or_else(|_| panic!("expected admitted regional operation"));
    assert_eq!(
        rows.remaining(4096, 17, 0)
            .unwrap_or_else(|_| panic!("expected admitted regional operation")),
        initial
    );
    rows.projected(Region::all_open(1))
        .unwrap_or_else(|_| panic!("expected admitted regional operation"));
    assert_eq!(rows.regional.as_ref().unwrap().roots[0].0, 1);
    assert_eq!(&rows.regional.as_ref().unwrap().authorized[..], &[2]);
}

#[test]
fn full_subset_roots_compose_into_the_same_complete_disjunctive_family() {
    use std::collections::BTreeSet;
    use std::num::NonZeroUsize;
    use zetesis_algebra::Cancellation;
    use zetesis_ferraris::{AdmissionLimits, FormulaParts, Node, Theory};
    use zetesis_stable::{BatchLimits, Limits, SlateProposal};

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
    for workers in [1, 2] {
        let mut models =
            StableModels::new(&theory, Limits::default(), Cancellation::default()).unwrap();
        let mut completion = CompletionExecutor::new(NonZeroUsize::new(workers).unwrap()).unwrap();
        let limits = BatchLimits {
            max_candidates: NonZeroUsize::new(3).unwrap(),
            max_pending_bytes: u64::MAX,
        };
        let mut family = BTreeSet::new();
        loop {
            let ticket = match models.propose_slate(limits).unwrap() {
                SlateProposal::New(ticket) => ticket,
                SlateProposal::Exhausted => break,
                _ => panic!("serially proposed fresh slates"),
            };
            let view = models.slate(&ticket).unwrap();
            let mut rows = CheckedRows::new(
                view.candidates().len(),
                Some((u64::MAX, models.pending_batch_bytes().unwrap(), 0)),
            )
            .unwrap_or_else(|_| panic!("input slots fit"));
            // The full subset box is a sound over-approximation for each exact
            // candidate. This isolates the product's authority/slot composition.
            for candidate in view.candidates() {
                let mut root = Region::all_open(2);
                for atom in 0..2 {
                    if !candidate.contains(atom) {
                        assert!(root.cut(atom));
                    }
                }
                rows.projected(root)
                    .unwrap_or_else(|_| panic!("one checked root per slot"));
            }
            for answer in rows
                .finish(&mut models, ticket, &mut completion)
                .unwrap_or_else(|_| panic!("exact regional completion"))
            {
                assert!(family.insert(answer.atoms().collect::<Vec<_>>()));
            }
        }
        assert_eq!(family, BTreeSet::from([vec![0], vec![1]]));
        assert!(models.exhausted());
    }
}
