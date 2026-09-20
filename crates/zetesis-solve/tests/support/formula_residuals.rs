//! Decoded reasons survive the separate exact-completion boundary.

use super::{Failure, FormulaResidualStatistics, decoded_verdict};
use zetesis_sat::{BatchVerdict, Incomplete};
use zetesis_wgpu::{FormulaVerdict, ResidualReason};

#[test]
fn decoded_verdicts_count_only_their_residual_reason() {
    let mut counts = FormulaResidualStatistics::default();
    for verdict in [FormulaVerdict::NotModel, FormulaVerdict::NoProperSubset] {
        assert!(!matches!(
            decoded_verdict(&mut counts, verdict)
                .unwrap_or_else(|_| panic!("counter unexpectedly refused")),
            BatchVerdict::Residual
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
            decoded_verdict(&mut counts, FormulaVerdict::Residual(reason))
                .unwrap_or_else(|_| panic!("counter unexpectedly refused")),
            BatchVerdict::Residual
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
            decoded_verdict(&mut counts, FormulaVerdict::Residual(reason)),
            Err(Failure::Search(Incomplete::CounterOverflow))
        ));
        assert_eq!(counts, before);
    }
}

#[test]
fn decoded_reasons_survive_exact_completion_refusal() {
    use zetesis_ferraris::{AdmissionLimits, Node, Theory};
    let theory = Theory::new(1, vec![Node::Atom(0)], vec![0], AdmissionLimits::default()).unwrap();
    let control = zetesis_cpu::Control::default();
    let mut models =
        zetesis_sat::StableModels::new(&theory, zetesis_sat::Limits::default(), control.clone())
            .unwrap();
    let options = crate::SolveConfig {
        max_completion_scratch_bytes: 0,
        completion_workers: std::num::NonZeroUsize::MIN,
        ..Default::default()
    };
    let mut queue = crate::formula_queue::BatchQueue::new(&options).unwrap();
    let mut counts = FormulaResidualStatistics::default();
    // An injected decoded result exercises host accounting, not GPU execution.
    let next = queue.next(&mut models, &options, &control, |_, candidates| {
        candidates
            .iter()
            .map(|_| {
                decoded_verdict(
                    &mut counts,
                    FormulaVerdict::Residual(ResidualReason::RoundLimit),
                )
            })
            .collect()
    });
    assert!(matches!(
        next,
        Some(Err(Failure::Search(Incomplete::CompletionScratch)))
    ));
    assert_eq!(counts.round_limit, 1);
    assert_eq!(models.batch_statistics().residuals, 0);
    assert_eq!(models.batch_statistics().committed, 0);
    assert_eq!(models.batch_statistics().pending, 1);
    assert!(!models.exhausted());
}
