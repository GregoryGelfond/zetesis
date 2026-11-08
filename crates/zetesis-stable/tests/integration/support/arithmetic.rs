//! Distinct controls for optional preparation refusal and checked root absorption.

use std::{convert::Infallible, num::NonZeroUsize};

use zetesis_ferraris::{PreparationStatus, Theory};
use zetesis_stable::{
    BatchLimits, BatchVerdict, Cancellation, CertificateLimits, CertificateOrder,
    GeneratorOrderStatus, Limits, StableModels,
};

use super::routes::{ROUTES, search};

/// Certificate-route controls must actually decline the optional arithmetic stage.
pub(crate) fn assert_arithmetic_declined(stream: &StableModels) {
    assert!(!stream.root_refuted());
    assert_eq!(
        stream.statistics().regions.unwrap().parity.status,
        PreparationStatus::StorageDeclined
    );
}

/// The original, default-limit contradiction absorbs all routes and pull forms.
pub(crate) fn assert_root_absorbed(original: &Theory, order: CertificateOrder) {
    for route in ROUTES {
        for batched in [false, true] {
            for cancelled in [false, true] {
                let cancellation = Cancellation::default();
                let mut stream = search(route, original, Limits::default(), cancellation.clone());
                assert!(stream.theory().same_instance(original));
                assert!(stream.root_refuted());
                assert!(!stream.exhausted());
                let before = stream.statistics();
                let regions = before.regions.unwrap();
                assert!(regions.parity.root_refuted);
                assert_eq!(
                    regions.generator_order.status,
                    GeneratorOrderStatus::NotPrepared
                );
                assert_eq!(regions.counts.regions, 0);
                assert_eq!(regions.counts.leaves, 0);
                assert_eq!(regions.counts.propagations, 0);
                assert!(regions.frontier.is_none());
                assert!(regions.workers.is_none());
                assert!(before.certified.is_none());
                assert_eq!(before.candidates, 0);
                assert_eq!(before.stable_models, 0);
                assert_eq!(before.countermodel_queries, 0);
                assert_eq!(before.countermodels, 0);
                let before_batch = stream.batch_statistics();
                assert_eq!(before_batch.original_validation_work, 0);
                assert_eq!(before_batch.checker_calls, 0);
                assert_eq!(before_batch.completion_calls, 0);
                assert_eq!(before_batch.committed, 0);
                assert_eq!(before_batch.propagated, 0);
                assert_eq!(before_batch.residuals, 0);
                assert_eq!(before_batch.pending, 0);
                assert_eq!(before_batch.lookahead_slates, 0);
                if cancelled {
                    cancellation.cancel();
                }
                assert!(
                    !stream
                        .enable_class_checking(CertificateLimits::default(), order)
                        .unwrap()
                );
                if batched {
                    let answers = stream
                        .next_batch(
                            BatchLimits {
                                max_candidates: NonZeroUsize::new(1).unwrap(),
                                max_pending_bytes: 0,
                            },
                            |_, _| -> Result<Vec<BatchVerdict>, Infallible> {
                                panic!("refuted root called checker")
                            },
                        )
                        .unwrap();
                    assert!(answers.is_empty());
                } else {
                    assert!(stream.next().is_none());
                }
                assert!(stream.exhausted());
                assert_eq!(stream.statistics(), before);
                assert_eq!(stream.batch_statistics(), before_batch);
            }
        }
    }
}
