//! Decoded reasons survive the separate exact-completion boundary.

use super::{Failure, FormulaResidualStatistics, decoded_verdict};
use zetesis_algebra::region::Region;
use zetesis_stable::{BatchVerdict, Incomplete};
use zetesis_wgpu::{FormulaVerdict, ResidualReason};

#[test]
fn decoded_verdicts_count_only_their_residual_reason() {
    let mut counts = FormulaResidualStatistics::default();
    for verdict in [FormulaVerdict::NotModel, FormulaVerdict::NoProperSubset] {
        assert!(!matches!(
            decoded_verdict(&mut counts, verdict)
                .unwrap_or_else(|_| panic!("counter unexpectedly refused")),
            (BatchVerdict::Residual, _)
        ));
    }
    assert_eq!(counts, FormulaResidualStatistics::default());
    for reason in [
        ResidualReason::FixedPoint,
        ResidualReason::RoundLimit,
        ResidualReason::WorkLimit,
        ResidualReason::FixedPoint,
    ] {
        assert!(matches!(
            decoded_verdict(
                &mut counts,
                FormulaVerdict::Residual(reason, Region::all_open(1))
            )
            .unwrap_or_else(|_| panic!("counter unexpectedly refused")),
            (BatchVerdict::Residual, Some(_))
        ));
    }
    assert_eq!(
        counts,
        FormulaResidualStatistics {
            fixed_point: 2,
            round_limit: 1,
            work_limit: 1
        }
    );
}

#[test]
fn residual_counter_overflow_preserves_its_receipt() {
    for reason in [
        ResidualReason::FixedPoint,
        ResidualReason::RoundLimit,
        ResidualReason::WorkLimit,
    ] {
        let mut counts = FormulaResidualStatistics {
            fixed_point: u64::MAX,
            round_limit: u64::MAX,
            work_limit: u64::MAX,
        };
        let before = counts;
        assert!(matches!(
            decoded_verdict(
                &mut counts,
                FormulaVerdict::Residual(reason, Region::all_open(1))
            ),
            Err(Failure::Search(Incomplete::CounterOverflow))
        ));
        assert_eq!(counts, before);
    }
}

#[test]
fn decoded_reasons_survive_exact_completion_refusal() {
    use zetesis_ferraris::{AdmissionLimits, Node, Theory};
    let theory = Theory::new(
        1,
        zetesis_ferraris::FormulaParts::new(vec![Node::atom(0)], vec![]).unwrap(),
        vec![0],
        AdmissionLimits::default(),
    )
    .unwrap();
    let cancellation = zetesis_algebra::Cancellation::default();
    let mut models = zetesis_stable::StableModels::new(
        &theory,
        zetesis_stable::Limits::default(),
        cancellation.clone(),
    )
    .unwrap();
    let mut completion =
        zetesis_stable::CompletionExecutor::with_scratch_limit(std::num::NonZeroUsize::MIN, 0)
            .unwrap();
    let zetesis_stable::SlateProposal::New(ticket) = models
        .propose_slate(zetesis_stable::BatchLimits {
            max_candidates: std::num::NonZeroUsize::MIN,
            max_pending_bytes: u64::MAX,
        })
        .unwrap()
    else {
        panic!("one candidate")
    };
    let mut counts = FormulaResidualStatistics::default();
    let (provisional, root) = decoded_verdict(
        &mut counts,
        FormulaVerdict::Residual(ResidualReason::RoundLimit, Region::all_open(1)),
    )
    .unwrap_or_else(|_| panic!("decoded reason fits"));
    let begin = models.begin_regional_pass(
        ticket,
        vec![provisional],
        vec![(0, root.unwrap())],
        &mut completion,
    );
    assert!(matches!(
        begin,
        Err(zetesis_stable::BatchError::Limits(
            Incomplete::CompletionScratch
        ))
    ));
    drop(begin);
    assert_eq!(counts.round_limit, 1);
    assert_eq!(models.batch_statistics().residuals, 0);
    assert_eq!(models.batch_statistics().committed, 0);
    assert_eq!(models.batch_statistics().pending, 1);
    assert!(!models.exhausted());
}

/// Compare both real CPU walks on every physically decoded residual in this
/// slate. The conjunctive head prevents producer extraction from eliminating
/// the nonminimal classical model before either membership query can see it.
pub(super) fn compare_physical_roots(profile: &zetesis_wgpu::GpuFormulaProfile) {
    use zetesis_ferraris::{AdmissionLimits, FormulaParts, Node, Theory};
    use zetesis_wgpu::{FormulaLimits, GpuFormulaOracle};
    // p → (p ∧ p) has both classical models. Under {p}, the empty set is
    // its only proper frozen witness; under {}, no proper subset exists.
    let theory = Theory::new(
        1,
        FormulaParts::new(
            vec![Node::atom(0), Node::and_pair([0, 0]), Node::implies(0, 1)],
            Vec::new(),
        )
        .unwrap(),
        vec![2],
        AdmissionLimits::default(),
    )
    .unwrap();
    let mut device = GpuFormulaOracle::from_profile(profile).unwrap();
    // Three nodes (six visits), four child occurrences, one atom and one
    // asserted root. The chain uses serial truth. Projection reserves three.
    let setup = 6 + 4 + 1 + 1 + 3;
    for (rounds, work, reason, narrowed) in [
        (0, u32::MAX, ResidualReason::RoundLimit, false),
        (1, u32::MAX, ResidualReason::RoundLimit, true),
        (4, u32::MAX, ResidualReason::FixedPoint, true),
        (1, setup, ResidualReason::WorkLimit, false),
    ] {
        compare_regional_slate(
            &mut device,
            &theory,
            FormulaLimits {
                max_rounds: rounds,
                max_work_per_candidate: work,
                ..FormulaLimits::default()
            },
            reason,
            narrowed,
        );
    }
}

fn compare_regional_slate(
    device: &mut zetesis_wgpu::GpuFormulaOracle,
    theory: &zetesis_ferraris::Theory,
    device_limits: zetesis_wgpu::FormulaLimits,
    expected_reason: ResidualReason,
    narrowed: bool,
) {
    use std::num::NonZeroUsize;
    use zetesis_algebra::Cancellation;
    use zetesis_stable::{
        BatchLimits, CompletionExecutor, Limits, RegionalOutcome, SlateProposal, StableModels,
    };
    let limits = BatchLimits {
        max_candidates: NonZeroUsize::new(3).unwrap(),
        max_pending_bytes: u64::MAX,
    };
    let mut whole = StableModels::new(theory, Limits::default(), Cancellation::default()).unwrap();
    let mut region = StableModels::new(theory, Limits::default(), Cancellation::default()).unwrap();
    let (SlateProposal::New(full_ticket), SlateProposal::New(device_ticket)) = (
        whole.propose_slate(limits).unwrap(),
        region.propose_slate(limits).unwrap(),
    ) else {
        panic!("both fresh streams must expose the complete two-candidate slate")
    };
    let left = whole.slate(&full_ticket).unwrap().candidates();
    let right = region.slate(&device_ticket).unwrap().candidates();
    assert_eq!(left.len(), 2);
    assert_eq!(right.len(), 2);
    assert_ne!(right[0].contains(0), right[1].contains(0));
    for (a, b) in left.iter().zip(right) {
        assert!(a.theory().same_instance(b.theory()) && a.atoms().eq(b.atoms()));
    }
    let results = device
        .propagate_batch(theory, right, device_limits)
        .unwrap();
    assert_eq!(results.len(), right.len());
    let mut full_executor = CompletionExecutor::default();
    let full_answers = whole
        .commit_slate(
            full_ticket,
            vec![BatchVerdict::Residual; 2],
            &mut full_executor,
        )
        .unwrap();
    assert_eq!(whole.statistics().countermodel_queries, 2);
    assert_eq!(whole.statistics().countermodels, 1);
    assert_eq!(full_answers.len(), 1);
    assert!(full_answers[0].atoms().next().is_none());
    let mut tags = Vec::new();
    let mut roots = Vec::new();
    let mut expected = Vec::new();
    for (slot, (candidate, result)) in right.iter().zip(results).enumerate() {
        let stable = full_answers
            .iter()
            .any(|accepted| accepted.atoms().eq(candidate.atoms()));
        match result.into_verdict() {
            FormulaVerdict::Residual(reason, root) => {
                assert_eq!(reason, expected_reason);
                if candidate.contains(0) {
                    assert_eq!(root.decision(0), narrowed.then_some(false));
                }
                tags.push(BatchVerdict::Residual);
                roots.push((slot, root));
                expected.push((
                    slot,
                    if stable {
                        RegionalOutcome::NoWitnessInSuppliedRegion
                    } else {
                        RegionalOutcome::ValidatedCountermodel
                    },
                ));
            }
            FormulaVerdict::NoProperSubset => {
                assert!(stable && narrowed);
                tags.push(BatchVerdict::NoProperSubset);
            }
            FormulaVerdict::NotModel => panic!("both proposals satisfy the original theory"),
        }
    }
    let count = if narrowed { 1 } else { 2 };
    assert_eq!(expected.len(), count);
    let mut scoped = CompletionExecutor::default();
    let pass = region
        .begin_regional_pass(device_ticket, tags, roots, &mut scoped)
        .unwrap();
    assert_eq!(pass.outcomes().collect::<Vec<_>>(), expected);
    let conclusions = expected
        .into_iter()
        .map(|(_, outcome)| match outcome {
            RegionalOutcome::ValidatedCountermodel => BatchVerdict::Refuted,
            RegionalOutcome::NoWitnessInSuppliedRegion => BatchVerdict::NoProperSubset,
        })
        .collect();
    let answers = pass.finish(conclusions).unwrap();
    assert_eq!(
        region.statistics().countermodel_queries,
        u64::try_from(count).unwrap()
    );
    assert_eq!(region.statistics().countermodels, 1);
    assert_eq!(answers.len(), 1);
    assert!(answers[0].atoms().eq(full_answers[0].atoms()));
}
