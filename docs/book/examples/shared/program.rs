// Shared preparation and enumeration for the two construction examples.
// ANCHOR: example
use std::{collections::BTreeSet, sync::Arc};
use zetesis::program::{AnswerSet, Program};
use zetesis_cpu::Cancellation;
use zetesis_solve::{Backend, Completion, PreparedInput, Session, SolveConfig};
use zetesis_themelios::{
    ExpansionLimits, FormulaLimits, ProgramAdmissionOptions, admit_program_formula, symbols,
};

const ATOM_EXPORT_BYTES: u128 = 4096;

pub(super) fn enumerate(
    program: &Program,
) -> Result<BTreeSet<AnswerSet>, Box<dyn std::error::Error>> {
    let admitted = admit_program_formula(
        Arc::new(program.clone()),
        ProgramAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )?;
    let cancellation = Cancellation::default();
    let mut session = Session::enumerate(
        PreparedInput::formula(&admitted),
        SolveConfig {
            backend: Backend::Cpu,
            models: 0,
            ..SolveConfig::default()
        },
        cancellation.clone(),
    )?;
    let mut family = BTreeSet::new();
    for answer in session.by_ref() {
        let answer = answer?;
        let atoms = answer
            .interpretation()
            .atoms()
            .iter()
            .map(|atom| symbols::atom_with(atom, ATOM_EXPORT_BYTES, || cancellation.poll()))
            .collect::<Result<AnswerSet, _>>()?;
        family.insert(atoms);
    }
    let outcome = session
        .outcome()
        .ok_or("enumeration ended without a search outcome")?;
    if outcome.completion() != Some(Completion::Exhausted) {
        return Err(format!("enumeration unfinished: {:?}", outcome.search_state()).into());
    }
    Ok(family)
}
// ANCHOR_END: example

// ANCHOR: hidden
// ANCHOR_END: hidden
