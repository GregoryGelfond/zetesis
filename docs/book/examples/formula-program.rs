//! Construct an exact-one choice and enumerate its complete typed answer family.

// ANCHOR: example
use std::{collections::BTreeSet, sync::Arc};
use zetesis_cpu::Cancellation;
use zetesis_solve::{Backend, Completion, PreparedInput, Session, SolveConfig};
use zetesis_themelios::logical::{
    AnswerSet, Name, Term,
    program::{Atom, Choice, ChoiceElement, Condition, Guard, Program, Relation, Rule},
};
use zetesis_themelios::{
    ExpansionLimits, FormulaLimits, ProgramAdmissionOptions, prepare_program_formula, symbols,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for answer in enumerate(choice_program()?)? {
        println!("{answer:?}");
    }
    Ok(())
}

fn choice_program() -> Result<Arc<Program>, Box<dyn std::error::Error>> {
    let chosen = Name::new("chosen")?;
    let elements = [1, 2].map(|value| {
        let atom = Atom::new(chosen.clone(), [Term::from(value)]);
        ChoiceElement::new(atom.into(), Condition::default())
    });
    let bound = || Guard {
        relation: Some(Relation::Le),
        term: 1.into(),
    };
    let choice = Choice::new(Some(bound()), elements, Some(bound()));
    Ok(Arc::new(Program::of([Rule::fact(choice)])))
}

fn enumerate(program: Arc<Program>) -> Result<BTreeSet<AnswerSet>, Box<dyn std::error::Error>> {
    let prepared = prepare_program_formula(
        program,
        ProgramAdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )?;
    let admitted = prepared.ground()?;
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
fn constructed_choice_has_exactly_two_complete_answers() -> Result<(), Box<dyn std::error::Error>> {
    use zetesis_themelios::logical::{Sign, Symbol};

    let chosen = Name::new("chosen")?;
    let expected = [1, 2].map(|value| {
        AnswerSet::from([Symbol::Function {
            name: chosen.clone(),
            arguments: vec![Symbol::Number(value)],
            sign: Sign::Positive,
        }])
    });
    assert_eq!(enumerate(choice_program()?)?, BTreeSet::from(expected));
    Ok(())
}
