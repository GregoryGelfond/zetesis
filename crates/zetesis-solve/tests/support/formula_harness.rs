//! Native formula session capture for the membership routes' tests.

use zetesis_core::Model;
use zetesis_cpu::Cancellation;
use zetesis_objective::Score;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, admit_formula,
};

use crate::execution_observation::Ignore;
use crate::formula_execution::MembershipExecution;
use crate::formula_session::FormulaSession;
use crate::phase_timing::Recorder;
use crate::{AnswerSelection, PhaseTimings, SemanticOutcome, SolveConfig, SolveError};

pub(super) type Record = (Model, Option<Vec<(i32, i64)>>);

pub(super) struct Capture {
    pub(super) answers: Vec<(Model, Option<Score>)>,
    pub(super) outcome: SemanticOutcome,
    pub(super) error: Option<SolveError>,
    pub(super) timings: Option<PhaseTimings>,
}

impl Capture {
    pub(super) fn records(&self) -> Vec<Record> {
        let mut records: Vec<_> = self
            .answers
            .iter()
            .map(|(model, score)| {
                (
                    model.clone(),
                    score.as_ref().map(|score| score.costs().to_vec()),
                )
            })
            .collect();
        records.sort();
        records
    }
}

pub(super) fn admitted(source: &str) -> AdmittedFormula {
    admit_formula(
        source.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
}

pub(super) fn input(owner: &AdmittedFormula) -> super::Input<'_> {
    super::Input {
        theory: owner.theory(),
        atoms: owner.atom_catalog(),
        gate_atoms: 0,
        keyed_constraints: 0,
        key_analysis: zetesis_themelios::KeyAnalysis::Complete,
        objectives: owner.objectives(),
        certificate_order: crate::countermodel::certificate_order(
            owner.source_analysis(),
            owner.analysis_basis(),
        ),
    }
}

pub(super) fn run(
    owner: &AdmittedFormula,
    config: &SolveConfig,
    cancellation: &Cancellation,
    execution: &mut impl MembershipExecution,
) -> Capture {
    run_consuming(owner, config, cancellation, execution, |_| {})
}

pub(super) fn run_consuming(
    owner: &AdmittedFormula,
    config: &SolveConfig,
    cancellation: &Cancellation,
    execution: &mut impl MembershipExecution,
    mut consume: impl FnMut(&Model),
) -> Capture {
    let phases = Recorder::new(config.stats);
    let mut session = FormulaSession::with_selection(
        input(owner),
        execution,
        config,
        &mut Ignore,
        cancellation,
        &phases,
        AnswerSelection::Optimal,
    );
    let mut answers = Vec::new();
    let mut error = None;
    while let Some(next) = session.next(config, &mut Ignore, cancellation, &phases) {
        match next {
            Ok(answer) => {
                consume(&answer.0);
                answers.push(answer);
            }
            Err(cause) => {
                error = Some(cause);
                assert!(
                    session
                        .next(config, &mut Ignore, cancellation, &phases)
                        .is_none()
                );
                break;
            }
        }
    }
    let outcome = session.outcome(&phases);
    assert!(
        outcome
            .subject()
            .unwrap()
            .same_instance(&crate::Subject::Theory(owner.theory().clone()))
    );
    Capture {
        answers,
        outcome,
        error,
        timings: phases.snapshot(),
    }
}
