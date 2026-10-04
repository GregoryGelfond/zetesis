//! Construct a logical program and enumerate its full typed answers.

// ANCHOR: example
use std::collections::BTreeSet;
use zetesis_cpu::Cancellation;
use zetesis_solve::{Backend, Completion, PreparedInput, Session, SolveConfig};
use zetesis_themelios::logical::{
    AnswerSet, Name,
    construct::not,
    program::{Atom, Program, Rule},
};
use zetesis_themelios::{ProgramAdmissionOptions, admit_program, symbols};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let p = Atom::constant(Name::new("p")?);
    let q = Atom::constant(Name::new("q")?);
    let program = Program::of([Rule::new(p.clone(), not(q.clone())), Rule::new(q, not(p))]);
    for answer in enumerate(&program)? {
        println!("{answer:?}");
    }
    Ok(())
}

fn enumerate(program: &Program) -> Result<BTreeSet<AnswerSet>, Box<dyn std::error::Error>> {
    let admitted = admit_program(program, ProgramAdmissionOptions::default())
        .map_err(|failure| failure.kind)?;
    let cancellation = Cancellation::default();
    let mut session = Session::enumerate(
        PreparedInput::program(admitted.program()),
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
            .map(|atom| symbols::atom_with(atom, 4096, || cancellation.poll()))
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

#[test]
fn constructed_program_has_its_complete_answer_family() -> Result<(), Box<dyn std::error::Error>> {
    use zetesis_themelios::logical::Symbol;

    let p = Atom::constant(Name::new("p")?);
    let q = Atom::constant(Name::new("q")?);
    let program = Program::of([Rule::new(p.clone(), not(q.clone())), Rule::new(q, not(p))]);
    let expected = BTreeSet::from([
        AnswerSet::from([Symbol::constant(Name::new("p")?)]),
        AnswerSet::from([Symbol::constant(Name::new("q")?)]),
    ]);
    assert_eq!(enumerate(&program)?, expected);
    Ok(())
}
