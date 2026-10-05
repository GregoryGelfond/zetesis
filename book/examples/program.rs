//! Construct a logical program and enumerate its full typed answers.

#[path = "shared/program.rs"]
mod execution;

// ANCHOR: example
use zetesis::program::{Atom, Name, Program, Rule, construct::not};

fn program() -> Result<Program, Box<dyn std::error::Error>> {
    let p = Atom::constant(Name::new("p")?);
    let q = Atom::constant(Name::new("q")?);
    Ok(Program::of([
        Rule::new(p.clone(), not(q.clone())),
        Rule::new(q, not(p)),
    ]))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for answer in execution::enumerate(&program()?)? {
        println!("{answer:?}");
    }
    Ok(())
}
// ANCHOR_END: example

#[test]
fn constructed_program_has_its_complete_answer_family() -> Result<(), Box<dyn std::error::Error>> {
    use std::collections::BTreeSet;
    use zetesis::program::{AnswerSet, Name, Symbol};

    let expected = BTreeSet::from([
        AnswerSet::from([Symbol::constant(Name::new("p")?)]),
        AnswerSet::from([Symbol::constant(Name::new("q")?)]),
    ]);
    assert_eq!(execution::enumerate(&program()?)?, expected);
    Ok(())
}
