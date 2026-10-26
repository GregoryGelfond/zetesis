// Shared preparation and enumeration for the two construction examples.
// ANCHOR: example
use std::{collections::BTreeMap, sync::Arc};
use zetesis::program::{AnswerSet, Program, Symbol};
use zetesis_cpu::Cancellation;
use zetesis_solve::{Backend, Completion, Session, SolveConfig, prepare_formula_execution};
use zetesis_themelios::{
    FormulaPurpose, ProgramFormulaOptions, observation, prepare_program_formula_with, symbols,
};

const ATOM_EXPORT_BYTES: u128 = 4096;

pub(super) fn enumerate(
    program: Arc<Program>,
) -> Result<BTreeMap<AnswerSet, Vec<Symbol>>, Box<dyn std::error::Error>> {
    let cancellation = Cancellation::default();
    let prepared = prepare_program_formula_with(
        program,
        ProgramFormulaOptions {
            purpose: FormulaPurpose::AnswerSets,
            cancellation: Some(cancellation.clone()),
            ..ProgramFormulaOptions::default()
        },
    )?;
    let config = SolveConfig {
        backend: Backend::Cpu,
        models: 0,
        ..SolveConfig::default()
    };
    let execution = prepare_formula_execution(prepared, &config, None)?;
    let mut session = Session::enumerate(execution.input(), config, cancellation.clone())?;
    let observations = execution.metadata().observations();
    let limits = observation::Limits::default();
    let mut answers = BTreeMap::new();
    for answer in session.by_ref() {
        let answer = answer?;
        let model = answer.interpretation();
        let atoms = model
            .atoms()
            .iter()
            .map(|atom| symbols::atom_with(atom, ATOM_EXPORT_BYTES, || cancellation.poll()))
            .collect::<Result<AnswerSet, _>>()?;
        let terms = observations
            .evaluate(model, limits, &cancellation)?
            .into_symbols();
        answers.insert(atoms, terms);
    }
    let outcome = session
        .outcome()
        .ok_or("enumeration ended without an outcome")?;
    if outcome.completion() != Some(Completion::Exhausted) {
        return Err(format!("enumeration unfinished: {:?}", outcome.search_state()).into());
    }
    Ok(answers)
}
// ANCHOR_END: example

// ANCHOR: hidden
// ANCHOR_END: hidden
