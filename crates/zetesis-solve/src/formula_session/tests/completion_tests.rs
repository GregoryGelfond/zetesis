//! Final session evidence is captured only after candidate workers settle.

use super::{FormulaSession, Input};
use crate::execution_observation::Ignore;
use crate::formula_execution::{Execution, Failure, MembershipExecution};
use crate::phase_timing::Recorder;
use crate::{
    AnswerSelection, Backend, Completion, FormulaExecutionStatistics, Interruption, Oracle,
    SearchMethod, SearchState, SolveConfig, SolveError,
};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::Interpretation;
use zetesis_sat::{Incomplete, StableModels};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

/// One native answer followed by an execution stop outside the iterator.
/// This exercises session finalization independently of native error cleanup.
struct StopAfterAnswer {
    delivered: bool,
    failure: bool,
}

impl MembershipExecution for StopAfterAnswer {
    fn next(
        &mut self,
        models: &mut StableModels,
        config: &SolveConfig,
        cancellation: &Cancellation,
        phases: &Recorder,
    ) -> Option<Result<Interpretation, Failure>> {
        if !self.delivered {
            self.delivered = true;
            return Execution::Cpu.next(models, config, cancellation, phases);
        }
        Some(Err(if self.failure {
            Failure::Run(SolveError::CandidateStreamNotExhausted)
        } else {
            Failure::Search(Incomplete::Deadline)
        }))
    }

    fn statistics(&self, _: &StableModels) -> Option<FormulaExecutionStatistics> {
        None
    }
}

#[test]
fn terminal_outcomes_settle_workers_before_caching_receipts() {
    let owner = admit_formula(
        include_str!("../../../tests/fixtures/completion/choices.lp").into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    for (models, failure) in [(1, false), (0, false), (0, true)] {
        let config = SolveConfig {
            backend: Backend::Cpu,
            oracle: Oracle::Countermodel,
            search: SearchMethod::Regions,
            workers: std::num::NonZeroUsize::new(2).unwrap(),
            models,
            ..SolveConfig::default()
        };
        let cancellation = Cancellation::default();
        let phases = Recorder::new(false);
        let mut session = FormulaSession::with_selection(
            Input {
                theory: owner.theory(),
                atoms: owner.atom_catalog(),
                objectives: owner.objectives(),
                gate_atoms: 0,
                keyed_constraints: 0,
                key_analysis: zetesis_themelios::KeyAnalysis::Complete,
                certificate_order: zetesis_sat::CertificateOrder::TightFirst,
            },
            StopAfterAnswer {
                delivered: false,
                failure,
            },
            &config,
            &mut Ignore,
            &cancellation,
            &phases,
            AnswerSelection::All,
        );
        session
            .next(&config, &mut Ignore, &cancellation, &phases)
            .unwrap()
            .unwrap();
        let next = session.next(&config, &mut Ignore, &cancellation, &phases);
        if failure {
            assert!(matches!(
                next,
                Some(Err(SolveError::CandidateStreamNotExhausted))
            ));
        } else {
            assert!(next.is_none());
        }
        let outcome = session.outcome(&phases);
        let expected = if failure {
            None
        } else if models == 1 {
            Some(SearchState::RequestedModels)
        } else {
            Some(SearchState::Interrupted(Interruption::Countermodel(
                Incomplete::Deadline,
            )))
        };
        assert_eq!(outcome.search_state(), expected);
        assert_ne!(outcome.completion(), Some(Completion::Exhausted));
        assert_eq!(outcome.verified_models(), 1);
        let native = session.models.as_mut().unwrap();
        assert!(
            native.next().is_none(),
            "finalization must fuse the native stream"
        );
        let settled = native.statistics();
        let reported = outcome.countermodel_statistics().unwrap();
        assert_eq!(reported.search, settled.search);
        assert_eq!(reported.regions, settled.regions);
        assert!(settled.regions.unwrap().counts.work <= settled.search.work);
        assert_eq!(cancellation.poll(), Ok(()));
    }
}
