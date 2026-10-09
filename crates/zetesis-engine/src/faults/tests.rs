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

fn stratified_interruption(error: zetesis_ferraris::StratifiedError) -> Interruption {
    Interruption::Countermodel(zetesis_sat::Incomplete::Certificate(
        zetesis_sat::CertificateError::Stratified(error),
    ))
}

#[test]
fn stratified_controls_are_conclusions() {
    for (stop, conclusion) in [
        (Stop::Cancelled, Conclusion::Interrupted),
        (Stop::Deadline, Conclusion::Budget),
    ] {
        let reason = stratified_interruption(zetesis_ferraris::StratifiedError::Stopped(stop));
        assert_eq!(interruption(reason), Ok(conclusion));
    }
}

#[test]
fn stratified_resources_are_faults() {
    use zetesis_ferraris::{StratifiedError, StratifiedResource};

    for error in [
        StratifiedError::Limit {
            resource: StratifiedResource::Dependencies,
            observed: 2,
            limit: 1,
        },
        StratifiedError::Limit {
            resource: StratifiedResource::Bytes,
            observed: 8,
            limit: 7,
        },
        StratifiedError::Limit {
            resource: StratifiedResource::Work,
            observed: 2,
            limit: 1,
        },
        StratifiedError::Overflow,
        StratifiedError::Stopped(Stop::Allocation),
    ] {
        let fault = interruption(stratified_interruption(error)).expect_err("resource refusal");
        assert_eq!(fault.locus(), Locus::Resource);
    }
}

#[test]
fn stratified_invariant_failures_are_engine_faults() {
    use zetesis_ferraris::StratifiedError;

    // Ineligibility normally selects another checker. If it escapes that
    // selection boundary, it is an engine failure, not an exhausted search.
    for error in [
        StratifiedError::UnsupportedRoot { root: 0 },
        StratifiedError::UnsupportedBody { root: 2, body: 1 },
        StratifiedError::NegativeCycle { atom: 0, body: 1 },
        StratifiedError::InvalidClosure { root: 0 },
        StratifiedError::Stopped(Stop::InvalidProgram),
        StratifiedError::Stopped(Stop::WrongProgram),
    ] {
        let fault = interruption(stratified_interruption(error)).expect_err("engine refusal");
        assert_eq!(fault.locus(), Locus::Engine);
    }
}

#[test]
fn stratified_faults_retain_the_native_cause() {
    let native =
        stratified_interruption(zetesis_ferraris::StratifiedError::InvalidClosure { root: 7 });
    let fault = interruption(native).expect_err("native invariant failure");
    assert_eq!(
        fault
            .source()
            .expect("typed cause")
            .downcast_ref::<Interruption>(),
        Some(&native)
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
