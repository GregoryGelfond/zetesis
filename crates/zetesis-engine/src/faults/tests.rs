use super::*;
use std::error::Error;
use themelios_solve::contract::Locus;

#[test]
fn request_controls_are_conclusions() {
    assert_eq!(control(Stop::Cancelled), Some(Conclusion::Interrupted));
    assert_eq!(control(Stop::Deadline), Some(Conclusion::Budget));
}

#[test]
fn native_ceilings_are_not_request_controls() {
    for stop in [
        Stop::WorkLimit,
        Stop::RoundLimit,
        Stop::StorageLimit,
        Stop::DerivedAtomLimit,
        Stop::CandidateLimit,
        Stop::CarrierLimit,
        Stop::Allocation,
    ] {
        assert_eq!(control(stop), None);
        let fault = interruption(Interruption::Oracle(stop)).expect_err("resource refusal");
        assert_eq!(fault.locus(), Locus::Resource);
        assert!(!fault.is_backend_bug());
    }
}

#[test]
fn resource_interruptions_retain_the_native_stage() {
    let native = Interruption::Countermodel(zetesis_sat::Incomplete::CounterOverflow);
    let fault = interruption(native).expect_err("accounting width is a resource");
    let cause = fault
        .source()
        .expect("typed cause")
        .downcast_ref::<Interruption>()
        .expect("the whole native interruption");
    assert_eq!(*cause, native);
}

#[test]
fn nested_certificate_controls_are_conclusions() {
    let reason = Interruption::Countermodel(zetesis_sat::Incomplete::Certificate(
        zetesis_sat::CertificateError::Tight(zetesis_ferraris::TightError::Stopped(Stop::Deadline)),
    ));
    assert_eq!(interruption(reason), Ok(Conclusion::Budget));
}

#[test]
fn nested_certificate_limits_are_resource_faults() {
    let reason = Interruption::Countermodel(zetesis_sat::Incomplete::Certificate(
        zetesis_sat::CertificateError::Evaluation(zetesis_ferraris::EvaluationError::Storage {
            required: 8,
            limit: 7,
        }),
    ));
    assert_eq!(
        interruption(reason).expect_err("resource").locus(),
        Locus::Resource
    );
}

#[test]
fn invalid_native_witnesses_are_engine_faults() {
    let reason = Interruption::Countermodel(zetesis_sat::Incomplete::InvalidWitness);
    let fault = interruption(reason).expect_err("native invariant refusal");
    assert_eq!(fault.locus(), Locus::Engine);
    assert!(!fault.is_backend_bug());
}

#[test]
fn absent_search_evidence_is_an_adapter_fault() {
    assert!(completed(None).expect_err("no coverage").is_backend_bug());
}

#[test]
fn model_storage_refusals_retain_the_solve_failure() {
    let original = Program::default();
    let native = SolveFailure::from(SolveError::Model(zetesis_core::ModelError::Catalog(
        zetesis_core::catalog::Error::Storage {
            required: 8,
            limit: 7,
        },
    )));
    let fault = session(native, &original).expect_err("resource");
    assert_eq!(fault.locus(), Locus::Resource);
    assert!(
        fault
            .source()
            .expect("typed cause")
            .downcast_ref::<SolveFailure>()
            .is_some()
    );
}

#[test]
fn batch_preparation_cancellation_is_a_conclusion() {
    let native = SolveFailure::from(SolveError::Batch(zetesis_cpu::BatchError::Preparation(
        Stop::Cancelled,
    )));
    assert_eq!(
        session(native, &Program::default()),
        Ok(Conclusion::Interrupted)
    );
}
