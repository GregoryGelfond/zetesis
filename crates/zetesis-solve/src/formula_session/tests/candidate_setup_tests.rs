//! A failed candidate setup remains in the session's terminal receipt.
use super::{FormulaSession, Input};
use crate::execution_observation::Ignore;
use crate::formula_execution::Execution;
use crate::phase_timing::Recorder;
use crate::{AnswerSelection, Backend, Completion, Interruption, Oracle, SolveConfig};
use zetesis_algebra::Cancellation;
use zetesis_reference_support::formula;
use zetesis_stable::{Incomplete, StableModels};
use zetesis_themelios::AdmittedFormula;

fn input(owner: &AdmittedFormula) -> Input<'_> {
    Input {
        theory: owner.theory(),
        atoms: owner.atom_catalog(),
        objectives: owner.objectives(),
        required_choices: owner.required_choices(),
        keyed_constraints: 0,
        key_analysis: zetesis_themelios::KeyAnalysis::Complete,
        certificate_order: zetesis_stable::CertificateOrder::TightFirst,
    }
}

fn limited(owner: &AdmittedFormula) -> SolveConfig {
    let mut config = SolveConfig {
        backend: Backend::Cpu,
        oracle: Oracle::Countermodel,
        workers: std::num::NonZeroUsize::MIN,
        models: 0,
        ..SolveConfig::default()
    };
    let full = StableModels::prepare(
        owner.theory(),
        crate::countermodel::search_limits(&config),
        Cancellation::default(),
    );
    assert!(full.result.is_ok());
    let regions = full.statistics.regions.unwrap();
    let arithmetic = regions.parity.preparation_work + regions.cardinality.preparation_work;
    assert!(arithmetic > 2);
    config.max_search_work =
        full.statistics.search.work - regions.generator_order.work - arithmetic / 2;
    config
}

#[test]
fn failed_initial_setup_is_reported_before_any_stream_exists() {
    let owner = formula("{a}.");
    let config = limited(&owner);
    let expected = StableModels::prepare(
        owner.theory(),
        crate::countermodel::search_limits(&config),
        Cancellation::default(),
    );
    assert!(matches!(expected.result, Err(Incomplete::WorkLimit)));
    let cancellation = Cancellation::default();
    let phases = Recorder::new(false);
    let mut session = FormulaSession::with_selection(
        input(&owner),
        Execution::Cpu,
        &config,
        &mut Ignore,
        &cancellation,
        &phases,
        AnswerSelection::All,
    );
    assert!(session.models.is_none());
    let outcome = session.outcome(&phases);
    assert_eq!(outcome.completion(), Some(Completion::Interrupted));
    assert_eq!(
        outcome.interruption(),
        Some(Interruption::Countermodel(Incomplete::WorkLimit))
    );
    assert_eq!(
        outcome.countermodel_statistics(),
        Some(&expected.statistics)
    );
    assert!(!outcome.unsatisfiable());
    assert!(
        session
            .next(&config, &mut Ignore, &cancellation, &phases)
            .is_none()
    );
    session.stop(&phases).unwrap();
    assert_eq!(
        session.outcome(&phases).countermodel_statistics(),
        Some(&expected.statistics)
    );
}

#[test]
fn failed_later_setup_adds_once_to_the_retained_pass() {
    let owner = formula("{a}.");
    let config = limited(&owner);
    let later = StableModels::prepare(
        owner.theory(),
        crate::countermodel::search_limits(&config),
        Cancellation::default(),
    );
    assert!(matches!(later.result, Err(Incomplete::WorkLimit)));
    let mut earlier = StableModels::new(
        owner.theory(),
        zetesis_stable::Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    for answer in earlier.by_ref() {
        answer.unwrap();
    }
    assert!(earlier.exhausted());
    let first = earlier.statistics();
    let mut session =
        FormulaSession::<Execution>::uninitialized(input(&owner), AnswerSelection::All);
    session.models = Some(earlier);
    session.candidate_setup = Some(later.statistics);
    let phases = Recorder::new(false);
    for _ in 0..2 {
        let outcome = session.outcome(&phases);
        let receipt = outcome.countermodel_statistics().unwrap();
        assert_eq!(
            receipt.search.work,
            first.search.work + later.statistics.search.work
        );
        assert_eq!(receipt.stable_models, first.stable_models);
        assert_eq!(receipt.candidates, first.candidates);
        assert_eq!(
            receipt.regions.unwrap().parity.preparation_work,
            first.regions.unwrap().parity.preparation_work
                + later.statistics.regions.unwrap().parity.preparation_work
        );
    }
}

#[test]
fn successful_setup_is_owned_only_by_the_stream() {
    let owner = formula("{a}.");
    let config = SolveConfig {
        backend: Backend::Cpu,
        oracle: Oracle::Countermodel,
        workers: std::num::NonZeroUsize::MIN,
        models: 0,
        ..SolveConfig::default()
    };
    let phases = Recorder::new(false);
    let session = FormulaSession::with_selection(
        input(&owner),
        Execution::Cpu,
        &config,
        &mut Ignore,
        &Cancellation::default(),
        &phases,
        AnswerSelection::All,
    );
    assert!(session.candidate_setup.is_none());
    assert_eq!(
        session.outcome(&phases).countermodel_statistics(),
        Some(&session.models.as_ref().unwrap().statistics())
    );
}
