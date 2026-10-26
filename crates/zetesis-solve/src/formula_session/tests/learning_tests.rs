//! A session shares one learned store among its streams and its execution,
//! unless the measurement switch turns sharing off.

use std::sync::Arc;

use super::{FormulaSession, Input};
use crate::execution_observation::Ignore;
use crate::formula_execution::{Execution, Failure, MembershipExecution};
use crate::phase_timing::Recorder;
use crate::{AnswerSelection, Backend, FormulaExecutionStatistics, SolveConfig};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::Interpretation;
use zetesis_reference_support::formula;
use zetesis_stable::{LearnedStore, StableModels};

/// The CPU execution, recording every store the session hands it.
#[derive(Default)]
struct Recording {
    stores: Vec<Arc<LearnedStore>>,
}

impl MembershipExecution for Recording {
    fn next(
        &mut self,
        models: &mut StableModels,
        config: &SolveConfig,
        cancellation: &Cancellation,
        phases: &Recorder,
    ) -> Option<Result<Interpretation, Failure>> {
        Execution::Cpu.next(models, config, cancellation, phases)
    }

    fn statistics(&self, _: &StableModels) -> Option<FormulaExecutionStatistics> {
        None
    }

    fn learned_store(&mut self, store: Arc<LearnedStore>) {
        self.stores.push(store);
    }
}

fn config(share_learned_rows: bool) -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        workers: std::num::NonZeroUsize::MIN,
        models: 0,
        share_learned_rows,
        ..SolveConfig::default()
    }
}

/// The answer sets the session enumerates, and the session after them.
fn enumerate<'a>(
    program: &'a zetesis_themelios::AdmittedFormula,
    config: &SolveConfig,
) -> (Vec<Vec<String>>, FormulaSession<'a, Recording>) {
    let cancellation = Cancellation::default();
    let phases = Recorder::new(false);
    let mut session = FormulaSession::with_selection(
        Input {
            theory: program.theory(),
            atoms: program.atom_catalog(),
            objectives: program.objectives(),
            required_choices: program.required_choices(),
            keyed_constraints: 0,
            key_analysis: zetesis_themelios::KeyAnalysis::Complete,
            certificate_order: zetesis_stable::CertificateOrder::TightFirst,
        },
        Recording::default(),
        config,
        &mut Ignore,
        &cancellation,
        &phases,
        AnswerSelection::All,
    );
    let mut answers = Vec::new();
    while let Some(next) = session.next(config, &mut Ignore, &cancellation, &phases) {
        let (model, _) = next.unwrap();
        let mut atoms: Vec<_> = model
            .atoms()
            .into_iter()
            .map(|atom| atom.predicate().name().to_owned())
            .collect();
        atoms.sort();
        answers.push(atoms);
    }
    answers.sort();
    (answers, session)
}

const LOOP: &str = "{x}. a :- b. b :- a. a :- x.";

#[test]
fn sharing_is_on_by_default() {
    assert!(SolveConfig::default().share_learned_rows);
    let resources = crate::Resources::new(8 << 30, std::num::NonZeroUsize::MIN);
    assert!(resources.solve_config().share_learned_rows);
}

#[test]
fn the_execution_receives_the_streams_store() {
    let program = formula(LOOP);
    let (_, session) = enumerate(&program, &config(true));
    let store = session.learned.as_ref().unwrap();
    let execution = session.execution.as_ref().unwrap();
    assert_eq!(execution.stores.len(), 1);
    assert!(Arc::ptr_eq(&execution.stores[0], store));
}

#[test]
fn turning_sharing_off_installs_no_store() {
    let program = formula(LOOP);
    let (_, session) = enumerate(&program, &config(false));
    assert!(session.learned.is_none());
    assert!(session.execution.as_ref().unwrap().stores.is_empty());
}

#[test]
fn the_switch_changes_no_answer_set() {
    let program = formula(LOOP);
    let (shared, _) = enumerate(&program, &config(true));
    let (alone, _) = enumerate(&program, &config(false));
    assert_eq!(shared, alone);
    assert_eq!(shared.len(), 2);
}
