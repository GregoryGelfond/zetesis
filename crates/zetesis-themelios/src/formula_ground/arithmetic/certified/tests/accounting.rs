//! A refused proof neither exempts a family nor retains its scratch.

use super::*;

fn attempt(
    limits: &FormulaLimits,
    counters: &mut Counters,
    program: &Prepared,
    support: &Support<'_>,
    computation: &mut Computation<'_, '_>,
) -> Result<bool, FormulaFailure> {
    family(
        program,
        partial_rule(program).location,
        support,
        computation,
        limits,
        &mut testing::budget(),
        counters,
    )
}

#[test]
fn every_proof_work_prefix_refuses_and_can_retry() {
    let mut required = 0;
    testing::with_completed_source(TOTAL, |program, support, computation, counters| {
        let before = counters.accounting.work;
        let live = counters.workspace_bytes();
        assert!(
            attempt(
                &FormulaLimits::default(),
                counters,
                program,
                support,
                computation
            )
            .unwrap()
        );
        required = counters.accounting.work - before;
        assert_eq!(counters.workspace_bytes(), live);
    });
    assert!(required > 0);
    for remaining in 0..=required {
        testing::with_completed_source(TOTAL, |program, support, computation, counters| {
            let limits = FormulaLimits {
                max_work: counters.accounting.work + remaining,
                ..Default::default()
            };
            let live = counters.workspace_bytes();
            let result = attempt(&limits, counters, program, support, computation);
            if remaining == required {
                assert!(result.unwrap());
            } else {
                assert!(matches!(
                    result,
                    Err(FormulaFailure::Limit {
                        resource: FormulaResource::Work,
                        ..
                    })
                ));
            }
            assert_eq!(
                counters.workspace_bytes(),
                live,
                "all private proof leases released"
            );
            assert!(
                attempt(
                    &FormulaLimits::default(),
                    counters,
                    program,
                    support,
                    computation
                )
                .unwrap()
            );
            assert_eq!(counters.workspace_bytes(), live);
        });
    }
}

#[test]
fn actual_proof_capacity_is_an_admission_boundary() {
    let mut peak = 0;
    testing::with_completed_source(TOTAL, |program, support, computation, counters| {
        let observer = Observer::default();
        let profile = Profile::new(Some(&observer));
        *counters = Counters::resume(std::mem::take(counters).into_accounting(), profile.work());
        profile
            .phase(GroundingPhase::SupportCompletion, None, || {
                attempt(
                    &FormulaLimits::default(),
                    counters,
                    program,
                    support,
                    computation,
                )
            })
            .unwrap();
        peak = usize::try_from(observer.0.get().support_peak_bytes.unwrap()).unwrap();
    });
    assert!(peak > 0);
    for cap in [peak - 1, peak] {
        testing::with_completed_source(TOTAL, |program, support, computation, counters| {
            let live = counters.workspace_bytes();
            let limits = FormulaLimits {
                max_support_bytes: cap,
                ..Default::default()
            };
            let result = attempt(&limits, counters, program, support, computation);
            if cap == peak {
                assert!(result.unwrap());
            } else {
                assert!(matches!(
                    result,
                    Err(FormulaFailure::Limit {
                        resource: FormulaResource::SupportBytes,
                        ..
                    })
                ));
            }
            assert_eq!(counters.workspace_bytes(), live);
        });
    }
}

#[test]
fn cancellation_before_publication_cannot_exempt() {
    testing::with_completed_source(TOTAL, |program, support, computation, counters| {
        let cancellation = Cancellation::default();
        *counters = std::mem::take(counters).with_cancellation(Some(&cancellation));
        let live = counters.workspace_bytes();
        let (result, proofs) = controlled(false, Some(cancellation), || {
            attempt(
                &FormulaLimits::default(),
                counters,
                program,
                support,
                computation,
            )
        });
        assert_eq!(
            proofs, 1,
            "cancel after the fragment proof, before family publication"
        );
        assert!(matches!(
            result,
            Err(FormulaFailure::Interrupted {
                reason: Stop::Cancelled,
                ..
            })
        ));
        assert_eq!(counters.workspace_bytes(), live);
        *counters = std::mem::take(counters).with_cancellation(None);
        assert!(
            attempt(
                &FormulaLimits::default(),
                counters,
                program,
                support,
                computation
            )
            .unwrap()
        );
    });
}

#[test]
fn reference_control_restores_after_unwind() {
    assert!(std::panic::catch_unwind(|| controlled(true, None, || panic!("test unwind"))).is_err());
    assert!(!disabled());
}
