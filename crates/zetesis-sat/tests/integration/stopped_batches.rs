//! Explicit stopping closes batch pulls while preserving unresolved receipts.

use std::{convert::Infallible, num::NonZeroUsize};
use zetesis_ferraris::{AdmissionLimits, Interpretation, Node, Theory};
use zetesis_sat::{
    BatchError, BatchLimits, BatchVerdict, Cancellation, CompletionExecutor, Incomplete, Limits,
    StableModels,
};

fn searches() -> [StableModels; 2] {
    let theory = Theory::new(
        1,
        vec![
            Node::Atom(0),
            Node::False,
            Node::Implies(0, 1),
            Node::Or(0, 2),
        ],
        vec![3],
        AdmissionLimits::default(),
    )
    .unwrap();
    [
        StableModels::new(&theory, Limits::default(), Cancellation::default()).unwrap(),
        StableModels::with_region_producers(
            &theory,
            NonZeroUsize::new(4).unwrap(),
            Limits::default(),
            Cancellation::default(),
        )
        .unwrap(),
    ]
}

fn limits() -> BatchLimits {
    BatchLimits {
        max_candidates: NonZeroUsize::MIN,
        max_pending_bytes: 1024 * 1024,
    }
}

fn unexpected_check(_: &Theory, _: &[Interpretation]) -> Result<Vec<BatchVerdict>, Infallible> {
    panic!("a stopped enumerator must not invoke a batch checker")
}

#[test]
fn explicit_stop_prevents_the_first_batch_proposal() {
    for mut search in searches() {
        search.stop().unwrap();
        let before = search.statistics();
        assert_eq!(before.candidates, 0);
        assert!(matches!(
            search.next_batch(limits(), unexpected_check),
            Err(BatchError::Search(Incomplete::ClosedEnumerator))
        ));
        assert_eq!(search.statistics(), before);
        assert!(!search.exhausted());
        assert!(search.next().is_none());
    }
}

#[test]
fn explicit_stop_preserves_a_batch_without_retrying_it() {
    for mut search in searches() {
        assert!(matches!(
            search.next_batch(limits(), |_, _| Err::<Vec<BatchVerdict>, _>("retained")),
            Err(BatchError::Checker("retained"))
        ));
        let pending = search.batch_statistics();
        assert_eq!(
            pending.pending, 1,
            "the failed checker retained an actual proposal"
        );
        assert_eq!(pending.committed, 0);
        search.stop().unwrap();
        let before = search.statistics();
        for _ in 0..2 {
            assert!(matches!(
                search.next_batch_with_completion(
                    limits(),
                    &mut CompletionExecutor::default(),
                    unexpected_check,
                ),
                Err(BatchError::Search(Incomplete::ClosedEnumerator))
            ));
            assert_eq!(search.batch_statistics(), pending);
            assert_eq!(search.statistics(), before);
            assert!(!search.exhausted());
        }
        assert!(search.next_verified().is_none());
    }
}
