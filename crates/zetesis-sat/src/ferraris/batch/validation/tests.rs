use super::*;
use zetesis_ferraris::{
    AdmissionLimits, ExtremumCutRule, ExtremumRow, FormulaNodes, FormulaParts, Node, NodeView,
};

fn numeric_theory() -> Theory {
    let mut nodes = FormulaNodes::default();
    let mut transaction = nodes.transaction();
    for atom in 0..2 {
        transaction.push(NodeView::Atom(atom), 20, 100).unwrap();
    }
    let bounds = transaction
        .push_threshold_family_range(
            &[0, 1, 0],
            &[0, u64::MAX, 1],
            &[1, i128::from(u64::MAX)],
            20,
            100,
        )
        .unwrap();
    let cuts = transaction
        .push_extremum_cut_family_range(
            ExtremumRow {
                conditions: &[0, 1, 0],
                ranks: &[0, 4, 3],
                rank_count: 5,
            },
            &[ExtremumCutRule::Prefix(1), ExtremumCutRule::Suffix(4)],
            20,
            100,
        )
        .unwrap();
    transaction.commit();
    Theory::new(
        2,
        nodes.into_parts(),
        vec![bounds.start, cuts.end - 1],
        AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn retained_numeric_storage_never_retains_candidate_truth() {
    let owner = numeric_theory();
    let mut validation = Validation::default();
    let mut timings = Some(SearchPhaseTimings::default());
    let mut prior_bytes = None;
    for (index, bits) in [2, 1, 3, 0, 2].into_iter().enumerate() {
        let candidate =
            Interpretation::new(&owner, (0..2).filter(|atom| bits & (1 << atom) != 0)).unwrap();
        let before = validation.work();
        let checked = validation.check(
            &owner,
            &candidate,
            EvaluationLimits::default(),
            &Cancellation::default(),
            &mut timings,
        );
        assert_eq!(
            checked,
            if bits & 2 != 0 {
                Ok(())
            } else {
                Err(Incomplete::InvalidWitness)
            }
        );
        let mut fresh = EvaluationWorkspace::default();
        let attempt = fresh.evaluate(
            &candidate,
            EvaluationLimits::default(),
            &Cancellation::default(),
        );
        let initialized_rows = if index == 0 { 0 } else { 2 };
        assert_eq!(validation.work() - before, attempt.work - initialized_rows);
        assert_eq!(attempt.result.unwrap().is_model(), checked.is_ok());
        if let Some(bytes) = prior_bytes {
            assert_eq!(validation.retained_bytes(), bytes);
        }
        prior_bytes = Some(validation.retained_bytes());
    }
    assert_eq!(timings.unwrap().original_validation.calls, 5);
}

#[test]
fn foreign_candidates_are_refused_before_evaluation() {
    let owner = numeric_theory();
    let foreign = numeric_theory();
    let candidate = Interpretation::new(&foreign, [1]).unwrap();
    let cancelled = Cancellation::default();
    cancelled.cancel();
    let mut validation = Validation::default();
    let bytes = validation.retained_bytes();
    let mut timings = Some(SearchPhaseTimings::default());
    assert_eq!(
        validation.check(
            &owner,
            &candidate,
            EvaluationLimits {
                max_work: 0,
                max_bytes: 0
            },
            &cancelled,
            &mut timings
        ),
        Err(Incomplete::WrongTheory)
    );
    assert_eq!(validation.work(), 0);
    assert_eq!(validation.retained_bytes(), bytes);
    assert_eq!(timings.unwrap().original_validation.calls, 1);
}

#[test]
fn interrupted_validation_can_reuse_storage_without_a_receipt() {
    let owner = numeric_theory();
    let accepted = Interpretation::new(&owner, [1]).unwrap();
    let rejected = Interpretation::new(&owner, [0]).unwrap();
    let mut validation = Validation::default();
    let mut timings = None;
    assert_eq!(
        validation.check(
            &owner,
            &accepted,
            EvaluationLimits {
                max_work: 1,
                ..EvaluationLimits::default()
            },
            &Cancellation::default(),
            &mut timings
        ),
        Err(Incomplete::Verification(zetesis_cpu::Stop::WorkLimit))
    );
    assert_eq!(validation.work(), 1);
    let bytes = validation.retained_bytes();
    assert_eq!(
        validation.check(
            &owner,
            &rejected,
            EvaluationLimits::default(),
            &Cancellation::default(),
            &mut timings
        ),
        Err(Incomplete::InvalidWitness)
    );
    assert_eq!(
        validation.check(
            &owner,
            &accepted,
            EvaluationLimits::default(),
            &Cancellation::default(),
            &mut timings
        ),
        Ok(())
    );
    assert_eq!(validation.retained_bytes(), bytes);
    assert!(timings.is_none());
}

#[test]
fn optional_capacity_does_not_block_a_smaller_owner() {
    let large = Theory::new(
        1,
        FormulaParts::new(vec![Node::atom(0); 1024], vec![]).unwrap(),
        vec![0],
        AdmissionLimits::default(),
    )
    .unwrap();
    let small = Theory::new(
        1,
        FormulaParts::new(vec![Node::atom(0)], vec![]).unwrap(),
        vec![0],
        AdmissionLimits::default(),
    )
    .unwrap();
    let mut validation = Validation::default();
    let mut timings = None;
    validation
        .check(
            &large,
            &Interpretation::new(&large, [0]).unwrap(),
            EvaluationLimits::default(),
            &Cancellation::default(),
            &mut timings,
        )
        .unwrap();
    let mut fresh = EvaluationWorkspace::default();
    fresh
        .reserve(&small, usize::MAX, &Cancellation::default())
        .unwrap();
    let exact = usize::try_from(fresh.retained_bytes()).unwrap();
    assert!(validation.retained_bytes() > exact as u128);
    validation.release_excess(exact);
    assert!(validation.retained_bytes() <= exact as u128);
    let candidate = Interpretation::new(&small, [0]).unwrap();
    assert_eq!(
        validation.check(
            &small,
            &candidate,
            EvaluationLimits {
                max_bytes: exact,
                ..EvaluationLimits::default()
            },
            &Cancellation::default(),
            &mut timings
        ),
        Ok(())
    );
    assert_eq!(validation.retained_bytes(), exact as u128);
    assert_eq!(
        validation.check(
            &small,
            &candidate,
            EvaluationLimits {
                max_bytes: exact - 1,
                ..EvaluationLimits::default()
            },
            &Cancellation::default(),
            &mut timings
        ),
        Err(Incomplete::PendingBytes)
    );
}

#[test]
fn pending_retry_charges_the_workspace_header_but_can_release_optional_storage() {
    use crate::{BatchError, BatchLimits, BatchVerdict, Limits, StableModels};
    use std::{convert::Infallible, num::NonZeroUsize};

    let owner = numeric_theory();
    let mut search = StableModels::new(&owner, Limits::default(), Cancellation::default()).unwrap();
    search.enable_phase_timing();
    let mut limits = BatchLimits {
        max_candidates: NonZeroUsize::new(1).unwrap(),
        max_pending_bytes: 4096,
    };
    let failed = search.next_batch(limits, |_, _| Err::<Vec<BatchVerdict>, _>("retry"));
    assert!(matches!(failed, Err(BatchError::Checker("retry"))));
    let before = search.batch_statistics();
    let calls = search
        .statistics()
        .phase_timings
        .unwrap()
        .original_validation
        .calls;
    let rows = search.pending_row_bytes() * search.batch.pending.capacity() as u128;
    limits.max_pending_bytes = u64::try_from(rows).unwrap();
    assert!(matches!(
        search.next_batch(limits, |_, _| -> Result<Vec<BatchVerdict>, Infallible> {
            panic!("workspace header cannot fit")
        }),
        Err(BatchError::Limits(Incomplete::PendingBytes))
    ));
    assert_eq!(search.batch_statistics().pending, 1);
    let header = std::mem::size_of::<EvaluationWorkspace>() as u128;
    assert_eq!(search.batch_statistics().original_validation_bytes, header);
    assert!(before.original_validation_bytes > header);
    limits.max_pending_bytes = u64::try_from(rows + header).unwrap();
    let retried = search
        .next_batch(limits, |original, candidates| {
            assert!(original.same_instance(&owner));
            assert_eq!(candidates.len(), 1);
            let accepted = zetesis_ferraris::check(
                original,
                &candidates[0],
                zetesis_ferraris::Limits::default(),
                &Cancellation::default(),
            )
            .unwrap()
            .accepted();
            Ok::<_, Infallible>(vec![if accepted {
                BatchVerdict::NoProperSubset
            } else {
                BatchVerdict::Refuted
            }])
        })
        .unwrap();
    assert!(retried.len() <= 1);
    assert_eq!(
        search.batch_statistics().original_validation_work,
        before.original_validation_work
    );
    assert_eq!(
        search
            .statistics()
            .phase_timings
            .unwrap()
            .original_validation
            .calls,
        calls
    );
}

#[test]
fn joined_validation_retains_only_the_exact_validated_prefix_on_failure() {
    use crate::{Limits, StableModels};

    let owner = numeric_theory();
    let mut search = StableModels::new(&owner, Limits::default(), Cancellation::default()).unwrap();
    search.batch.pending = vec![
        Interpretation::new(&owner, [1]).unwrap(),
        Interpretation::new(&owner, [0]).unwrap(),
        Interpretation::new(&owner, [0, 1]).unwrap(),
    ];
    search.admit_produced(
        &crate::ferraris::region_proposals::Produced {
            exhausted: true,
            stopped: None,
            original_validation: crate::PhaseMeasurement::default(),
        },
        usize::MAX,
    );
    assert_eq!(search.batch.pending.len(), 1);
    assert_eq!(search.batch.pending[0].atoms().collect::<Vec<_>>(), vec![1]);
    assert!(!search.batch.exhausted);
    assert_eq!(search.pending_error, Some(Incomplete::InvalidWitness));
    assert_eq!(search.statistics().candidates, 1);
}

#[test]
fn pending_reservation_discards_old_scratch_before_a_header_refusal() {
    use crate::{BatchLimits, Limits, StableModels};

    let owner = numeric_theory();
    let mut search = StableModels::new(&owner, Limits::default(), Cancellation::default()).unwrap();
    search
        .batch
        .validation
        .check(
            &owner,
            &Interpretation::new(&owner, [1]).unwrap(),
            EvaluationLimits::default(),
            &Cancellation::default(),
            &mut None,
        )
        .unwrap();
    let header = std::mem::size_of::<EvaluationWorkspace>() as u128;
    assert!(search.batch.validation.retained_bytes() > header);
    assert_eq!(search.batch.pending.capacity(), 0);
    let limits = BatchLimits {
        max_candidates: std::num::NonZeroUsize::new(3).unwrap(),
        max_pending_bytes: u64::try_from(3 * search.pending_row_bytes() + header - 1).unwrap(),
    };
    assert_eq!(
        search.prepare_pending(limits),
        Err(Incomplete::PendingBytes)
    );
    assert_eq!(search.batch.validation.retained_bytes(), header);
    assert_eq!(
        search.batch.pending.capacity(),
        0,
        "a known header refusal does not allocate pending rows"
    );
}
