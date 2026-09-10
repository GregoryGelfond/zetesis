use std::num::NonZeroUsize;

use super::*;
use crate::tight_measurement::{Case, Family};

fn configuration(family: Family) -> Configuration {
    Configuration {
        cases: vec![Case {
            family,
            atoms: NonZeroUsize::new(4).unwrap(),
            candidates: NonZeroUsize::new(32).unwrap(),
            reference: Reference::ExhaustiveReduct,
        }],
        backend: crate::Backend::Cpu,
        support: crate::tight_measurement::Support::Atomic,
        warmups: 0,
        repetitions: NonZeroUsize::new(1).unwrap(),
        workers: NonZeroUsize::new(2).unwrap(),
        plan_limits: zetesis_ferraris::TightPlanLimits::default(),
        certificate_limits: zetesis_ferraris::TightCheckLimits::default(),
        reference_limits: zetesis_ferraris::Limits::default(),
        residual_limits: zetesis_sat::Limits::default(),
        gpu_limits: zetesis_wgpu::TightGpuLimits::default(),
    }
}

#[test]
fn tiny_certificates_agree_with_every_reduct_subset() {
    let control = Control::default();
    for family in [
        Family::Normal,
        Family::Choices,
        Family::SupportUniform,
        Family::SupportSkewed,
    ] {
        let configuration = configuration(family);
        let prepared = prepare(configuration.cases[0], &configuration, &control).unwrap();
        let theory = &prepared.fixture.theory;
        for mask in 0..16 {
            let candidate =
                Interpretation::new(theory, (0..4).filter(|atom| mask & (1 << atom) != 0)).unwrap();
            let original = zetesis_ferraris::models(
                theory,
                &candidate,
                configuration.reference_limits,
                &control,
            )
            .unwrap();
            let mut smaller_model = false;
            for subset in 0..16 {
                if subset == mask || subset & !mask != 0 {
                    continue;
                }
                let tested =
                    Interpretation::new(theory, (0..4).filter(|atom| subset & (1 << atom) != 0))
                        .unwrap();
                smaller_model |= zetesis_ferraris::models_reduct(
                    theory,
                    &candidate,
                    &tested,
                    configuration.reference_limits,
                    &control,
                )
                .unwrap();
            }
            let verdict = prepared
                .plan
                .check_accounted(&candidate, configuration.certificate_limits, &control)
                .result
                .unwrap()
                .verdict;
            assert_eq!(
                matches!(verdict, TightVerdict::Stable),
                original && !smaller_model
            );
        }
    }
}

#[test]
fn certificate_witness_substitution_is_detected() {
    let configuration = configuration(Family::Normal);
    let control = Control::default();
    let prepared = prepare(configuration.cases[0], &configuration, &control).unwrap();
    let mut changed = prepared.certificates.clone();
    changed[0] = TightVerdict::NotModel { root: usize::MAX };
    assert!(matches!(
        validate(
            &prepared,
            &changed,
            &prepared.reference,
            &configuration,
            &control
        ),
        Err(Error::Parity)
    ));
}

#[test]
fn missing_occurrences_cannot_pass_parity() {
    let configuration = configuration(Family::Normal);
    let control = Control::default();
    let prepared = prepare(configuration.cases[0], &configuration, &control).unwrap();
    assert!(matches!(
        validate(
            &prepared,
            &prepared.certificates,
            &prepared.reference[..31],
            &configuration,
            &control
        ),
        Err(Error::Parity)
    ));
}

#[test]
fn a_candidate_is_not_its_own_countermodel() {
    let configuration = configuration(Family::Normal);
    let control = Control::default();
    let prepared = prepare(configuration.cases[0], &configuration, &control).unwrap();
    let candidate = &prepared.fixture.candidates[1];
    let false_witness = zetesis_sat::Check::NonMinimal(candidate.clone());
    assert!(matches!(
        validate_witness(candidate, &false_witness, &configuration, &control),
        Err(Error::Parity)
    ));
}

#[test]
fn a_foreign_countermodel_is_refused() {
    let configuration = configuration(Family::Normal);
    let control = Control::default();
    let prepared = prepare(configuration.cases[0], &configuration, &control).unwrap();
    let foreign = prepare(configuration.cases[0], &configuration, &control).unwrap();
    assert!(matches!(
        validate_witness(
            &prepared.fixture.candidates[1],
            &foreign.reference[1],
            &configuration,
            &control
        ),
        Err(Error::Parity)
    ));
}

#[test]
fn a_subset_failing_the_frozen_reduct_is_refused() {
    let configuration = configuration(Family::Normal);
    let control = Control::default();
    let prepared = prepare(configuration.cases[0], &configuration, &control).unwrap();
    let false_witness = zetesis_sat::Check::NonMinimal(prepared.fixture.candidates[0].clone());
    assert!(matches!(
        validate_witness(
            &prepared.fixture.candidates[1],
            &false_witness,
            &configuration,
            &control
        ),
        Err(Error::Parity)
    ));
}

#[test]
fn certificate_failure_keeps_every_joined_charge() {
    let mut configuration = configuration(Family::Normal);
    let control = Control::default();
    let prepared = prepare(configuration.cases[0], &configuration, &control).unwrap();
    configuration.certificate_limits.max_work = 1;
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .unwrap();
    for pool in [None, Some(&pool)] {
        let mut activity = Activity::default();
        assert!(matches!(
            classify(&prepared, &configuration, pool, &control, &mut activity),
            Err(Error::Certificate(_))
        ));
        assert_eq!(activity.cpu_certificate_work, 32);
        assert_eq!(activity.cpu_certificate_attempts, 32);
        assert_eq!(activity.classified, 0);
    }
}

#[test]
fn stopped_control_prevents_direct_completion() {
    let mut configuration = configuration(Family::Choices);
    configuration.cases[0].candidates = NonZeroUsize::new(1).unwrap();
    let control = Control::default();
    let prepared = prepare(configuration.cases[0], &configuration, &control).unwrap();
    assert_eq!(prepared.certificates, [TightVerdict::Stable]);
    control.cancel();
    let mut activity = Activity::default();
    assert!(matches!(
        complete(
            &prepared,
            &prepared.certificates,
            &configuration,
            &control,
            &mut activity
        ),
        Err(Error::Cpu(zetesis_cpu::Stop::Cancelled))
    ));
    assert_eq!(activity.residual_attempts, 0);
}

#[test]
fn stopped_control_prevents_direct_validation() {
    let mut configuration = configuration(Family::Choices);
    configuration.cases[0].candidates = NonZeroUsize::new(1).unwrap();
    let control = Control::default();
    let prepared = prepare(configuration.cases[0], &configuration, &control).unwrap();
    control.cancel();
    assert!(matches!(
        validate(
            &prepared,
            &prepared.certificates,
            &prepared.reference,
            &configuration,
            &control
        ),
        Err(Error::Cpu(zetesis_cpu::Stop::Cancelled))
    ));
}
