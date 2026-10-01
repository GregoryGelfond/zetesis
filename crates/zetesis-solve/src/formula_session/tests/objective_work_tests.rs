//! Objective receipts survive without an incumbent or a successful diagnostic.

use super::{FormulaSession, Input};
use crate::execution_observation::{Ignore, Observer};
use crate::formula_execution::Execution;
use crate::phase_timing::Recorder;
use crate::{
    AnswerSelection, Backend, ExecutionObservation, ExecutionObserver, Interruption, Oracle,
    SearchMethod, SolveConfig, SolveError,
};
use std::io;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::Interpretation;
use zetesis_themelios::objective_bound::{ObjectivePlan, ObjectivePlanLimits};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, admit_formula,
};

fn owner() -> AdmittedFormula {
    admit_formula(
        "{a}. #minimize {1,a:a}.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
}
fn input(owner: &AdmittedFormula) -> Input<'_> {
    Input {
        theory: owner.theory(),
        atoms: owner.atom_catalog(),
        objectives: owner.objectives(),
        gate_atoms: 0,
        keyed_constraints: 0,
        key_analysis: zetesis_themelios::KeyAnalysis::Complete,
        certificate_order: zetesis_sat::CertificateOrder::TightFirst,
    }
}
fn config() -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        oracle: Oracle::Countermodel,
        search: SearchMethod::Clauses,
        workers: std::num::NonZeroUsize::MIN,
        models: 0,
        max_objective_bound_work: 0,
        ..SolveConfig::default()
    }
}
fn plan(owner: &AdmittedFormula, cancellation: &Cancellation) -> ObjectivePlan {
    ObjectivePlan::new(
        owner.theory(),
        owner.atoms(),
        owner.objectives(),
        ObjectivePlanLimits::default(),
        cancellation,
    )
    .unwrap()
}

#[test]
fn cumulative_objective_work_counts_preparation_once() {
    let owner = owner();
    let cancellation = Cancellation::default();
    let plan = plan(&owner, &cancellation);
    let preparation = plan.statistics().work;
    // This fixed DAG visits all keys for either truth assignment.
    let read_work = plan
        .score(
            &Interpretation::new(owner.theory(), []).unwrap(),
            zetesis_objective::Limits::default(),
            &cancellation,
        )
        .unwrap()
        .unwrap()
        .work();
    for selection in [AnswerSelection::All, AnswerSelection::Optimal] {
        let config = config();
        let phases = Recorder::new(false);
        let mut session = FormulaSession::with_selection(
            input(&owner),
            Execution::Cpu,
            &config,
            &mut Ignore,
            &cancellation,
            &phases,
            selection,
        );
        let prepared = session.outcome(&phases);
        assert_eq!(prepared.objective_work(), preparation);
        assert_eq!(prepared.scored_models(), 0);
        assert!(prepared.incumbent().is_none());
        while let Some(answer) = session.next(&config, &mut Ignore, &cancellation, &phases) {
            answer.unwrap();
        }
        let outcome = session.outcome(&phases);
        assert_eq!(outcome.scored_models(), 2);
        assert_eq!(outcome.objective_work(), preparation + 2 * read_work);
        if selection == AnswerSelection::All {
            assert!(outcome.incumbent().is_none());
        } else {
            assert_eq!(outcome.incumbent().unwrap().work, outcome.objective_work());
        }
        session.stop(&phases).unwrap();
        assert_eq!(
            session.outcome(&phases).objective_work(),
            outcome.objective_work()
        );
    }
}

#[derive(Default)]
struct RefusePreparationDiagnostic {
    work: Option<u64>,
}
impl ExecutionObserver for RefusePreparationDiagnostic {
    type Error = io::Error;
    fn observe(&mut self, observation: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        if let ExecutionObservation::ObjectiveUnavailable(error) = observation {
            self.work = Some(error.statistics().work);
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "objective diagnostic closed",
            ));
        }
        Ok(())
    }
}

#[test]
fn refused_preparation_receipt_survives_its_diagnostic_failure() {
    let owner = owner();
    let config = SolveConfig {
        max_objective_work: 1,
        ..config()
    };
    let cancellation = Cancellation::default();
    let phases = Recorder::new(false);
    let mut diagnostic = RefusePreparationDiagnostic::default();
    let mut session = FormulaSession::with_selection(
        input(&owner),
        Execution::Cpu,
        &config,
        &mut Observer(&mut diagnostic),
        &cancellation,
        &phases,
        AnswerSelection::All,
    );
    assert_eq!(diagnostic.work, Some(1));
    let outcome = session.outcome(&phases);
    assert_eq!(outcome.objective_work(), 1);
    assert_eq!(outcome.scored_models(), 0);
    assert!(outcome.incumbent().is_none());
    assert!(matches!(
        session.next(&config, &mut Ignore, &cancellation, &phases),
        Some(Err(SolveError::ExecutionObservation(_)))
    ));
    assert_eq!(session.outcome(&phases).objective_work(), 1);
}

#[test]
fn refused_first_score_adds_its_prefix_without_creating_an_incumbent() {
    let owner = owner();
    let cancellation = Cancellation::default();
    let preparation = plan(&owner, &cancellation).statistics().work;
    let config = SolveConfig {
        max_objective_work: preparation + 3,
        ..config()
    };
    let phases = Recorder::new(false);
    let mut session = FormulaSession::with_selection(
        input(&owner),
        Execution::Cpu,
        &config,
        &mut Ignore,
        &cancellation,
        &phases,
        AnswerSelection::Optimal,
    );
    assert!(
        session
            .next(&config, &mut Ignore, &cancellation, &phases)
            .is_none()
    );
    let outcome = session.outcome(&phases);
    let Some(Interruption::PreparedObjective(error)) = outcome.interruption() else {
        panic!("expected the first prepared score to run out of work")
    };
    assert_eq!(error.work(), 3);
    assert_eq!(outcome.objective_work(), preparation + error.work());
    assert_eq!(outcome.scored_models(), 0);
    assert!(outcome.incumbent().is_none());
}
