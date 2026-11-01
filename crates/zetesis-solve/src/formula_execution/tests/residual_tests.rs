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
