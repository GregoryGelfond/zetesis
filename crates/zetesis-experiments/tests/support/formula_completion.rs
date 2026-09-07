//! Adversarial result claims exercise the qualification boundary without a device.

use super::{Membership, complete_residuals, native, verify, verify_residency};
use crate::{FormulaBenchmarkError, FormulaFamily, FormulaFixture};
use zetesis_ferraris::Interpretation;
use zetesis_wgpu::{FormulaBatchStats, FormulaVerdict, ResidualReason};

#[test]
fn every_claim_is_checked_against_complete_native_membership() {
    for family in [
        FormulaFamily::Choices,
        FormulaFamily::Cycle,
        FormulaFamily::Conjunction,
        FormulaFamily::Disjunction,
        FormulaFamily::MaskedImplication,
    ] {
        for atoms in 1..=4 {
            let fixture = FormulaFixture::new(family, atoms).unwrap();
            for bits in 0..1usize << atoms {
                let candidate = Interpretation::new(
                    fixture.theory(),
                    (0..atoms).filter(|a| bits & (1 << a) != 0),
                )
                .unwrap();
                let expected = native(fixture.theory(), &candidate, 100_000_000).unwrap();
                for claim in [
                    FormulaVerdict::NotModel,
                    FormulaVerdict::NoProperSubset,
                    FormulaVerdict::Residual(ResidualReason::FixedPoint),
                    FormulaVerdict::Residual(ResidualReason::RoundLimit),
                    FormulaVerdict::Residual(ResidualReason::WorkLimit),
                ] {
                    let result = complete_residuals(
                        fixture.theory(),
                        std::slice::from_ref(&candidate),
                        [claim].into_iter(),
                        &[expected],
                        100_000_000,
                    );
                    if matches!(claim, FormulaVerdict::Residual(_))
                        && expected == Membership::NotModel
                    {
                        assert!(matches!(result, Err(FormulaBenchmarkError::Parity)));
                        continue;
                    }
                    let (actual, residuals) = result.unwrap();
                    let justified = match claim {
                        FormulaVerdict::NotModel => expected == Membership::NotModel,
                        FormulaVerdict::NoProperSubset => expected == Membership::Stable,
                        FormulaVerdict::Residual(_) => true,
                    };
                    assert_eq!(
                        verify(&actual, &[expected]).is_ok(),
                        justified,
                        "{family:?}/{atoms}/{bits}/{claim:?}"
                    );
                    assert_eq!(
                        residuals,
                        usize::from(matches!(claim, FormulaVerdict::Residual(_)))
                    );
                }
            }
        }
    }
}

#[test]
fn missing_extra_or_reordered_results_cannot_qualify() {
    let fixture = FormulaFixture::new(FormulaFamily::Cycle, 1).unwrap();
    let candidates = fixture.candidates(2, 0).unwrap();
    let expected = [Membership::Stable, Membership::NonMinimal];
    for length in [0, 1, 3] {
        let claims = vec![FormulaVerdict::NoProperSubset; length];
        assert!(matches!(
            complete_residuals(
                fixture.theory(),
                &candidates,
                claims.into_iter(),
                &expected,
                100_000_000
            ),
            Err(FormulaBenchmarkError::Parity)
        ));
        assert!(matches!(
            complete_residuals(
                fixture.theory(),
                &candidates,
                [FormulaVerdict::NoProperSubset; 2].into_iter(),
                &vec![Membership::Stable; length],
                100_000_000
            ),
            Err(FormulaBenchmarkError::Parity)
        ));
    }
    assert!(matches!(
        verify(&[Membership::NonMinimal, Membership::Stable], &expected),
        Err(FormulaBenchmarkError::Parity)
    ));
    assert!(matches!(
        verify(&expected[..1], &expected),
        Err(FormulaBenchmarkError::Parity)
    ));
}

#[test]
fn an_incomplete_residual_cannot_publish_an_earlier_completed_prefix() {
    let fixture = FormulaFixture::new(FormulaFamily::Cycle, 1).unwrap();
    let candidates = fixture.candidates(2, 0).unwrap();
    let verdicts = [
        FormulaVerdict::NoProperSubset,
        FormulaVerdict::Residual(ResidualReason::WorkLimit),
    ];
    let expected = [Membership::Stable, Membership::NonMinimal];
    assert!(matches!(
        complete_residuals(
            fixture.theory(),
            &candidates,
            verdicts.into_iter(),
            &expected,
            0
        ),
        Err(FormulaBenchmarkError::Incomplete(_))
    ));
    let (completed, residuals) = complete_residuals(
        fixture.theory(),
        &candidates,
        verdicts.into_iter(),
        &expected,
        100_000_000,
    )
    .unwrap();
    assert_eq!(completed, expected);
    assert_eq!(residuals, 1);
    let foreign = FormulaFixture::new(FormulaFamily::Cycle, 1).unwrap();
    assert!(matches!(
        native(foreign.theory(), &candidates[1], 100_000_000),
        Err(FormulaBenchmarkError::Incomplete(_))
    ));
}

#[test]
fn warm_residency_must_be_proven_for_both_authored_allocations() {
    for iteration in [0, 1, 2, usize::MAX] {
        assert!(matches!(
            verify_residency(None, iteration),
            Err(FormulaBenchmarkError::Residency)
        ));
        for theory_uploaded in [false, true] {
            for transport_allocated in [false, true] {
                let stats = FormulaBatchStats {
                    theory_uploaded,
                    transport_allocated,
                    resident_theory_bytes: 32,
                    resident_transport_bytes: 64,
                    accounted_bytes: 128,
                };
                let expected = iteration == 0 || (!theory_uploaded && !transport_allocated);
                assert_eq!(verify_residency(Some(&stats), iteration).is_ok(), expected);
            }
        }
    }
}
