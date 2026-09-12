//! Native session capture for injected membership executors.

use zetesis_core::{Atom, Model};
use zetesis_cpu::Control;
use zetesis_objective::Score;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, admit_formula,
};

use crate::execution_observation::Ignore;
use crate::formula_execution::MembershipExecution;
use crate::formula_session::FormulaSession;
use crate::phase_timing::Recorder;
use crate::{AnswerSelection, PhaseTimings, SemanticOutcome, SolveConfig, SolveError};

pub(super) type Record = (Vec<Atom>, Option<Vec<(i32, i64)>>);

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
                    model.atoms().iter().cloned().collect(),
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
        objectives: owner.objectives(),
    }
}

pub(super) fn run(
    owner: &AdmittedFormula,
    config: &SolveConfig,
    control: &Control,
    execution: &mut impl MembershipExecution,
) -> Capture {
    run_consuming(owner, config, control, execution, |_| {})
}

pub(super) fn run_consuming(
    owner: &AdmittedFormula,
    config: &SolveConfig,
    control: &Control,
    execution: &mut impl MembershipExecution,
    mut consume: impl FnMut(&Model),
) -> Capture {
    let phases = Recorder::new(config.stats);
    let mut session = FormulaSession::with_selection(
        input(owner),
        execution,
        config,
        &mut Ignore,
        control,
        &phases,
        AnswerSelection::Optimal,
    );
    let mut answers = Vec::new();
    let mut error = None;
    while let Some(next) = session.next(config, &mut Ignore, control, &phases) {
        match next {
            Ok(answer) => {
                consume(&answer.0);
                answers.push(answer);
            }
            Err(cause) => {
                error = Some(cause);
                assert!(
                    session
                        .next(config, &mut Ignore, control, &phases)
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
