//! Construction failure after real membership cannot publish a partial model.

use super::{FormulaSession, Input};
use crate::execution_observation::Ignore;
use crate::formula_execution::{Execution, Failure, MembershipExecution};
use crate::phase_timing::Recorder;
use crate::{
    AnswerSelection, Backend, Completion, FormulaExecutionStatistics, Interruption,
    ModelConstructionStop, SolveConfig, SolvePhase,
};
use zetesis_core::Model;
use zetesis_cpu::{Cancellation, Stop};
use zetesis_ferraris::Interpretation;
use zetesis_objective::Score;
use zetesis_reference_support::formula;
use zetesis_sat::StableModels;
use zetesis_test_support::programs::model;
use zetesis_themelios::AdmittedFormula;

struct CancelAfterMembership;

impl MembershipExecution for CancelAfterMembership {
    fn next(
        &mut self,
        models: &mut StableModels,
        config: &SolveConfig,
        cancellation: &Cancellation,
        phases: &Recorder,
    ) -> Option<Result<Interpretation, Failure>> {
        let next = Execution::Cpu.next(models, config, cancellation, phases);
        if matches!(next, Some(Ok(_))) {
            cancellation.cancel();
        }
        next
    }

    fn statistics(&self, _: &StableModels) -> Option<FormulaExecutionStatistics> {
        None
    }
}

#[test]
fn cancellation_after_membership_prevents_construction() {
    let owner = formula("a.");
    let config = SolveConfig {
        backend: Backend::Cpu,
        workers: std::num::NonZeroUsize::MIN,
        models: 0,
        ..SolveConfig::default()
    };
    let cancellation = Cancellation::default();
    let phases = Recorder::new(true);
    let mut session = FormulaSession::with_selection(
        Input {
            theory: owner.theory(),
            atoms: owner.atom_catalog(),
            objectives: owner.objectives(),
            required_choices: owner.required_choices(),
            gate_atoms: 0,
            keyed_constraints: 0,
            key_analysis: zetesis_themelios::KeyAnalysis::Complete,
            certificate_order: zetesis_sat::CertificateOrder::TightFirst,
        },
        CancelAfterMembership,
        &config,
        &mut Ignore,
        &cancellation,
        &phases,
        AnswerSelection::All,
    );
    let prepared = *session.outcome(&phases).model_construction().unwrap();
    assert!(
        session
            .next(&config, &mut Ignore, &cancellation, &phases)
            .is_none()
    );
    let outcome = session.outcome(&phases);
    assert_eq!(outcome.verified_models(), 1);
    assert_eq!(outcome.completion(), Some(Completion::Interrupted));
    assert_eq!(
        outcome.interruption(),
        Some(Interruption::ModelConstruction(
            ModelConstructionStop::Control(Stop::Cancelled)
        ))
    );
    let receipt = outcome.model_construction().unwrap();
    assert_eq!(receipt.work, prepared.work);
    assert_eq!(receipt.prepared_bytes, prepared.prepared_bytes);
    assert_eq!(receipt.constructed, 0);
    assert!(!outcome.unsatisfiable());
    assert_eq!(
        phases
            .snapshot()
            .unwrap()
            .get(SolvePhase::ModelConstruction)
            .unwrap()
            .calls,
        2
    );
    assert!(
        session
            .next(&config, &mut Ignore, &cancellation, &phases)
            .is_none()
    );
}

/// Retain actual CPU membership results, then present the smaller model first.
/// These one-choice fixtures need a cheap construction before an expensive one;
/// their failure contract does not depend on native candidate traversal order.
#[derive(Default)]
struct OrderedMembership {
    ready: Option<std::vec::IntoIter<Interpretation>>,
}

impl MembershipExecution for OrderedMembership {
    fn next(
        &mut self,
        models: &mut StableModels,
        config: &SolveConfig,
        cancellation: &Cancellation,
        phases: &Recorder,
    ) -> Option<Result<Interpretation, Failure>> {
        if self.ready.is_none() {
            let mut answers = Vec::new();
            while let Some(answer) = Execution::Cpu.next(models, config, cancellation, phases) {
                match answer {
                    Ok(answer) => answers.push(answer),
                    Err(error) => return Some(Err(error)),
                }
            }
            answers.sort_by_key(|answer| answer.atoms().count());
            self.ready = Some(answers.into_iter());
        }
        self.ready.as_mut().unwrap().next().map(Ok)
    }

    fn statistics(&self, _: &StableModels) -> Option<FormulaExecutionStatistics> {
        None
    }
}

fn config() -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        workers: std::num::NonZeroUsize::MIN,
        completion_workers: std::num::NonZeroUsize::MIN,
        models: 0,
        max_objective_bound_work: 0,
        stats: true,
        ..SolveConfig::default()
    }
}

fn session<'a>(
    owner: &'a AdmittedFormula,
    config: &SolveConfig,
    cancellation: &Cancellation,
    phases: &Recorder,
    selection: AnswerSelection,
) -> FormulaSession<'a, OrderedMembership> {
    FormulaSession::with_selection(
        Input {
            theory: owner.theory(),
            atoms: owner.atom_catalog(),
            objectives: owner.objectives(),
            required_choices: owner.required_choices(),
            gate_atoms: 0,
            keyed_constraints: 0,
            key_analysis: zetesis_themelios::KeyAnalysis::Complete,
            certificate_order: zetesis_sat::CertificateOrder::TightFirst,
        },
        OrderedMembership::default(),
        config,
        &mut Ignore,
        cancellation,
        phases,
        selection,
    )
}

struct Prefix {
    first: Model,
    score: Option<Score>,
    prepared: u64,
    first_work: u64,
    limit: u64,
}

/// Measure the exact unbounded prefix for this explicitly ordered executor.
/// The derived fixed limit admits preparation and the first model, then refuses
/// the second. Production Session order remains covered at its public boundary.
fn prefix(owner: &AdmittedFormula) -> Prefix {
    let config = config();
    let cancellation = Cancellation::default();
    let phases = Recorder::new(true);
    let mut complete = session(owner, &config, &cancellation, &phases, AnswerSelection::All);
    let prepared = complete.outcome(&phases).model_construction().unwrap().work;
    let first = complete
        .next(&config, &mut Ignore, &cancellation, &phases)
        .unwrap()
        .unwrap();
    let first_work = complete.outcome(&phases).model_construction().unwrap().work - prepared;
    let second = complete
        .next(&config, &mut Ignore, &cancellation, &phases)
        .unwrap()
        .unwrap();
    assert!(
        complete
            .next(&config, &mut Ignore, &cancellation, &phases)
            .is_none()
    );
    assert_eq!(first.0, model(&[]));
    assert_eq!(second.0, model(&["a"]));
    let outcome = complete.outcome(&phases);
    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
    let second_work = outcome.model_construction().unwrap().work - prepared - first_work;
    let limit = second_work - 1;
    assert!(prepared <= limit && first_work <= limit);
    Prefix {
        first: first.0,
        score: first.1,
        prepared,
        first_work,
        limit,
    }
}

#[test]
fn construction_refusal_preserves_the_checked_prefix() {
    let owner = formula("{a}.");
    let expected = prefix(&owner);
    let config = SolveConfig {
        max_model_work: expected.limit,
        ..config()
    };
    let cancellation = Cancellation::default();
    let phases = Recorder::new(true);
    let mut limited = session(
        &owner,
        &config,
        &cancellation,
        &phases,
        AnswerSelection::All,
    );
    let first = limited
        .next(&config, &mut Ignore, &cancellation, &phases)
        .unwrap()
        .unwrap();
    assert_eq!(first.0, expected.first);
    assert!(
        limited
            .next(&config, &mut Ignore, &cancellation, &phases)
            .is_none()
    );
    let outcome = limited.outcome(&phases);
    assert_eq!(outcome.verified_models(), 2);
    assert!(matches!(
        outcome.interruption(),
        Some(Interruption::ModelConstruction(ModelConstructionStop::Work { observed, limit }))
            if observed == u128::from(expected.limit) + 1 && limit == expected.limit
    ));
    let receipt = outcome.model_construction().unwrap();
    assert_eq!(
        receipt.work,
        expected.prepared + expected.first_work + expected.limit
    );
    assert_eq!(receipt.constructed, 1);
    assert_eq!(outcome.completion(), Some(Completion::Interrupted));
    assert!(!outcome.unsatisfiable());
    assert!(
        limited
            .next(&config, &mut Ignore, &cancellation, &phases)
            .is_none()
    );
    assert_eq!(first.0, expected.first);
}

#[test]
fn construction_refusal_drains_prior_incumbents() {
    let owner = formula("{a}. #minimize{1,a:a}.");
    let expected = prefix(&owner);
    let config = SolveConfig {
        max_model_work: expected.limit,
        ..config()
    };
    let cancellation = Cancellation::default();
    let phases = Recorder::new(true);
    let mut limited = session(
        &owner,
        &config,
        &cancellation,
        &phases,
        AnswerSelection::Optimal,
    );
    let retained = limited
        .next(&config, &mut Ignore, &cancellation, &phases)
        .unwrap()
        .unwrap();
    assert_eq!(retained.0, expected.first);
    assert_eq!(retained.1, expected.score);
    assert!(
        limited
            .next(&config, &mut Ignore, &cancellation, &phases)
            .is_none()
    );
    let outcome = limited.outcome(&phases);
    assert_eq!(
        (
            outcome.verified_models(),
            outcome.scored_models(),
            outcome.retained_models()
        ),
        (2, 1, 1)
    );
    let receipt = outcome.model_construction().unwrap();
    assert_eq!(
        receipt.work,
        expected.prepared + expected.first_work + expected.limit
    );
    assert_eq!(receipt.constructed, 1);
    assert_eq!(outcome.completion(), Some(Completion::Interrupted));
    assert!(matches!(
        outcome.interruption(),
        Some(Interruption::ModelConstruction(ModelConstructionStop::Work { observed, limit }))
            if observed == u128::from(expected.limit) + 1 && limit == expected.limit
    ));
    assert!(!outcome.optimum_proved());
}
