// Shared preparation and enumeration for the two construction examples.
// ANCHOR: example
use std::{collections::BTreeMap, sync::Arc};
use zetesis::program::{AnswerSet, Program, Symbol};
use zetesis_cpu::Cancellation;
use zetesis_solve::{
    Backend, ClosureRoute, Completion, Grounder, PreparedInput, Session, SolveConfig,
};
use zetesis_themelios::{
    FormulaPurpose, ProgramRelationalOptions, observation, prepare_program_relational, symbols,
};

const ATOM_EXPORT_BYTES: u128 = 4096;

pub(super) fn enumerate(
    program: Arc<Program>,
) -> Result<BTreeMap<AnswerSet, Vec<Symbol>>, Box<dyn std::error::Error>> {
    let cancellation = Cancellation::default();
    let prepared = prepare_program_relational(
        program,
        ProgramRelationalOptions {
            purpose: FormulaPurpose::AnswerSets,
            cancellation: Some(cancellation.clone()),
            ..ProgramRelationalOptions::default()
        },
    )?;
    let mut session = Session::enumerate(
        PreparedInput::relational(&prepared),
        SolveConfig {
            backend: Backend::Cpu,
            grounder: Grounder::Lazy,
            models: 0,
            ..SolveConfig::default()
        },
        cancellation.clone(),
    )?;
    let observations = prepared.metadata().observations();
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
    if !matches!(
        outcome.closure_execution().map(|execution| execution.route),
        Some(ClosureRoute::Lazy(_))
    ) {
        return Err("the session did not use lazy closure".into());
    }
    Ok(answers)
}
// ANCHOR_END: example

// ANCHOR: hidden
// ANCHOR_END: hidden
