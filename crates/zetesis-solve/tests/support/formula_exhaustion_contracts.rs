//! A missing execution result cannot certify candidate exhaustion.

use super::{FormulaSession, Input};
use crate::execution_observation::Ignore;
use crate::formula_execution::{Execution, Failure, MembershipExecution};
use crate::phase_timing::Recorder;
use crate::{
    AnswerSelection, Backend, FormulaExecutionStatistics, Oracle, SolveConfig, SolveError,
    SolveMeasurements,
};
use zetesis_cpu::Control;
use zetesis_ferraris::Interpretation;
use zetesis_sat::StableModels;
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

struct TruncatedExecution {
    remaining: usize,
}

impl MembershipExecution for TruncatedExecution {
    fn next(
        &mut self,
        models: &mut StableModels,
        config: &SolveConfig,
        control: &Control,
        phases: &Recorder,
    ) -> Option<Result<Interpretation, Failure>> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        Execution::Cpu.next(models, config, control, phases)
    }
    fn statistics(&self, _: &StableModels) -> Option<FormulaExecutionStatistics> {
        None
    }
}

#[test]
fn unexhausted_execution_preserves_only_established_evidence() {
    for (source, selection, delivered) in [
        ("{a;b}.", AnswerSelection::All, 0),
        ("{a;b}.", AnswerSelection::All, 1),
        (
            "{a;b}. #minimize{1,a:a;1,b:b}.",
            AnswerSelection::Optimal,
            1,
        ),
    ] {
        let owner = admit_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap();
        let measurements = SolveMeasurements::new(false);
        let phases = measurements.recorder();
        let config = SolveConfig {
            backend: Backend::Cpu,
            oracle: Oracle::Countermodel,
            models: 0,
            max_objective_bound_work: 0,
            ..SolveConfig::default()
        };
        let control = Control::default();
        let mut observations = Ignore;
        let mut session = FormulaSession::with_selection(
            Input {
                theory: owner.theory(),
                atoms: owner.atom_catalog(),
                objectives: owner.objectives(),
                gate_atoms: 0,
            },
            TruncatedExecution {
                remaining: delivered,
            },
            &config,
            &mut observations,
            &control,
            phases,
            selection,
        );
        let mut yielded = 0;
        loop {
            match session.next(&config, &mut observations, &control, phases) {
                Some(Ok(_)) => yielded += 1,
                Some(Err(SolveError::CandidateStreamNotExhausted)) => break,
                other => panic!("expected the typed execution failure, got {other:?}"),
            }
        }
        assert_eq!(
            yielded,
            if selection == AnswerSelection::All {
                delivered
            } else {
                0
            }
        );
        let outcome = session.outcome(phases);
        assert_eq!(outcome.verified_models(), delivered as u64);
        assert_eq!(outcome.search_state(), None);
        assert_eq!(outcome.completion(), None);
        assert_eq!(outcome.interruption(), None);
        assert!(!outcome.unsatisfiable());
        assert!(!outcome.optimum_proved());
        if selection == AnswerSelection::Optimal {
            assert_eq!(outcome.scored_models(), 1);
            assert_eq!(outcome.retained_models(), 1);
            assert!(outcome.incumbent().is_some());
        }
        assert!(
            session
                .next(&config, &mut observations, &control, phases)
                .is_none()
        );
    }
}
