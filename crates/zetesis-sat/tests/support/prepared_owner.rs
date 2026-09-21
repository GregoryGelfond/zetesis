//! Actual ordinary ownership, restriction and pending-batch contracts.
use super::*;
use crate::{BatchLimits, BatchVerdict, Cancellation, StableModels};
use std::{convert::Infallible, num::NonZeroUsize};
use zetesis_ferraris::Node;

fn choice() -> Theory {
    Theory::new(
        1,
        vec![
            Node::Atom(0),
            Node::False,
            Node::Implies(0, 1),
            Node::Or(0, 2),
        ],
        vec![3],
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn ordinary_restrictions_keep_the_original_prepared_owner() {
    let theory = choice();
    let mut search = StableModels::with_method(
        &theory,
        crate::SearchMethod::Clauses,
        Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    search.enable_phase_timing();
    let first = search.next().unwrap().unwrap();
    assert_eq!(first.atoms().count(), 0);
    let owner = search.reduct.prepared().unwrap().clone();
    let receipt = search.statistics().reduct.preparation;
    let restriction = Theory::new(
        1,
        vec![Node::Atom(0)],
        vec![0],
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap();
    search.restrict_candidates(&restriction).unwrap();
    let second = search.next().unwrap().unwrap();
    assert_eq!(second.atoms().collect::<Vec<_>>(), vec![0]);
    assert!(search.reduct.prepared().unwrap().same_owner(&owner));
    assert!(owner.theory().same_instance(&theory));
    assert_eq!(search.statistics().reduct.preparation, receipt);
    assert_eq!(
        search
            .statistics()
            .phase_timings
            .unwrap()
            .reduct_preparation
            .calls,
        1
    );
    assert!(search.next().is_none());
    assert!(search.exhausted());
}

#[test]
fn all_certified_batches_do_not_prepare_a_reduct() {
    let mut search = StableModels::with_method(
        &choice(),
        crate::SearchMethod::Clauses,
        Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    search.enable_phase_timing();
    let output = search
        .next_batch(
            BatchLimits {
                max_candidates: NonZeroUsize::new(2).unwrap(),
                max_pending_bytes: 4096,
            },
            |_, candidates| {
                Ok::<_, Infallible>(vec![BatchVerdict::NoProperSubset; candidates.len()])
            },
        )
        .unwrap();
    assert_eq!(output.len(), 2);
    assert!(search.reduct.prepared().is_none());
    assert!(search.statistics().reduct.preparation.is_none());
    assert_eq!(
        search
            .statistics()
            .phase_timings
            .unwrap()
            .reduct_preparation
            .calls,
        0
    );
}

#[test]
fn preparation_refusal_preserves_pending_candidate_coverage() {
    let limits = Limits {
        max_reduct_bytes: 0,
        ..Default::default()
    };
    let mut search = StableModels::with_method(
        &choice(),
        crate::SearchMethod::Clauses,
        limits,
        Cancellation::default(),
    )
    .unwrap();
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
    assert!(search.reduct.prepared().is_none());
    let preparation = search.statistics().reduct.preparation.unwrap();
    assert_eq!(preparation.retained_bytes, 0);
    assert!(preparation.work > 0);
    assert!(preparation.work <= search.statistics().search.work);
    assert_eq!(search.statistics().countermodel_queries, 0);
}
