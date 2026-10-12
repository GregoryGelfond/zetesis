//! Exact source evaluation under a complete dependency-order certificate.
//!
//! The evaluator establishes membership before ordinary bounded model
//! construction. No candidate theory is substituted for the original source.

use zetesis_core::Model;
use zetesis_cpu::Cancellation;
use zetesis_objective::Score;
use zetesis_themelios::AcyclicSource;

use crate::execution_observation::ExecutionSink;
use crate::model_construction::{Account, Failure};
use crate::phase_timing::{Recorder, SolvePhase};
use crate::{AnswerSelection, Interruption, SearchState, SemanticOutcome, SolveConfig, SolveError};

#[cfg(test)]
mod tests;

pub(crate) struct SourceSession<'a> {
    owner: &'a AcyclicSource,
    finished: bool,
    checked: u64,
    verified: u64,
    state: Option<SearchState>,
    construction: Option<Account>,
}

impl<'a> SourceSession<'a> {
    pub(crate) fn new(
        owner: &'a AcyclicSource,
        observations: &mut impl ExecutionSink,
    ) -> Result<Self, SolveError> {
        observations.record(crate::ExecutionObservation::SourceEvaluation)?;
        Ok(Self {
            owner,
            finished: false,
            checked: 0,
            verified: 0,
            state: None,
            construction: None,
        })
    }

    pub(crate) fn next(
        &mut self,
        config: &SolveConfig,
        cancellation: &Cancellation,
        phases: &Recorder,
    ) -> Option<Result<(Model, Option<Score>), SolveError>> {
        if self.finished {
            return None;
        }
        // Source preparation already established the exact interpretation.
        // This session consumes it once, under its own publication allowances.
        self.finished = true;
        if let Err(stop) = cancellation.poll() {
            self.state = Some(SearchState::Interrupted(Interruption::Preparation(stop)));
            return None;
        }
        self.checked = 1;
        let Some(positions) = self.owner.selected_positions() else {
            self.state = Some(SearchState::Exhausted);
            return None;
        };
        self.verified = 1;
        let construction = self.construction.get_or_insert_with(Account::default);
        let model = phases.measure(SolvePhase::ModelConstruction, || {
            let order = construction.prepare(self.owner.atom_catalog(), config, cancellation)?;
            construction.select(&order, positions.iter().copied(), config, cancellation)
        });
        match model {
            Ok(model) => {
                // The class certificate establishes uniqueness. A requested
                // count of one does not erase already established exhaustion.
                self.state = Some(SearchState::Exhausted);
                Some(Ok((model, None)))
            }
            Err(Failure::Interrupted(stop)) => {
                self.state = Some(SearchState::Interrupted(Interruption::ModelConstruction(
                    stop,
                )));
                None
            }
            Err(Failure::Run(error)) => Some(Err(error)),
        }
    }

    pub(crate) const fn finished(&self) -> bool {
        self.finished
    }

    pub(crate) const fn stop(&mut self) {
        self.finished = true;
    }

    pub(crate) fn outcome(&self) -> SemanticOutcome {
        SemanticOutcome {
            projection: None,
            subject: Some(crate::Subject::Source(self.owner.clone())),
            selection: Some(AnswerSelection::All),
            verified: self.verified,
            scored: 0,
            retained: 0,
            search_state: self.state,
            optimization: None,
            objective_work: 0,
            checked: self.checked,
            gate_atoms: 0,
            candidate_statistics: None,
            countermodel_statistics: None,
            formula_execution: None,
            lazy_execution: None,
            shared_execution: None,
            closure_execution: None,
            query_execution: None,
            hybrid_execution: None,
            terminal_execution: None,
            source_evaluation: Some(self.owner.statistics()),
            model_construction: self.construction.as_ref().map(Account::statistics),
        }
    }
}
