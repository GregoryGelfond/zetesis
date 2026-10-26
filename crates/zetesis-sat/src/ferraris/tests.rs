//! Actual ordinary ownership and pending-batch contracts.
use super::*;
use crate::{BatchLimits, BatchVerdict, Cancellation, StableModels};
use std::{convert::Infallible, num::NonZeroUsize};
use zetesis_ferraris::Node;

fn choice() -> Theory {
    Theory::new(
        1,
        zetesis_ferraris::FormulaParts::new(
            vec![
                Node::atom(0),
                Node::falsum(),
                Node::implies(0, 1),
                Node::or_pair([0, 2]),
            ],
            vec![],
        )
        .unwrap(),
        vec![3],
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn a_reduct_storage_refusal_preserves_pending_candidate_coverage() {
    let limits = Limits {
        max_reduct_bytes: 0,
        ..Default::default()
    };
    let mut search = StableModels::new(&choice(), limits, Cancellation::default()).unwrap();
    let result = search.next_batch(
        BatchLimits {
            max_candidates: NonZeroUsize::new(2).unwrap(),
            max_pending_bytes: 4096,
        },
        |_, candidates| Ok::<_, Infallible>(vec![BatchVerdict::Residual; candidates.len()]),
    );
    assert!(matches!(
        result,
        Err(crate::BatchError::Search(Incomplete::ReductStorage { .. }))
    ));
    assert!(!search.exhausted());
    assert_eq!(search.batch_statistics().pending, 2);
    assert_eq!(search.batch_statistics().committed, 0);
    assert_eq!(search.statistics().countermodel_queries, 0);
}
