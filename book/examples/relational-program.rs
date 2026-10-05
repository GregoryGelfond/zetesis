//! Prepare a canonical program for lazy enumeration with separate shown terms.

#[path = "shared/relational-program.rs"]
mod execution;

// ANCHOR: example
use std::sync::Arc;
use zetesis::program::{
    Atom, Body, BodyElement, Name, Program, Rule, Statement,
    program::{Choice, ChoiceElement, Condition, Show},
};

fn program() -> Result<Arc<Program>, Box<dyn std::error::Error>> {
    let chosen = Atom::new(Name::new("chosen")?, [1.into()]);
    let selection = ChoiceElement::new(chosen.clone().into(), Condition::empty());
    let optional = Choice::new(None, [selection], None);
    let statements: [Statement; 3] = [
        Rule::fact(optional).into(),
        Show::All.into(), // #show. selects no atoms for display.
        Show::term_body(1, Body::new([BodyElement::Literal(chosen.into())])).into(),
    ];
    Ok(Arc::new(Program::of(statements)))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for (atoms, terms) in execution::enumerate(program()?)? {
        println!("full atoms: {atoms:?}; #show terms: {terms:?}");
    }
    Ok(())
}
// ANCHOR_END: example

#[test]
fn constructed_lazy_answers_preserve_shown_terms() -> Result<(), Box<dyn std::error::Error>> {
    use std::collections::BTreeMap;
    use zetesis::program::{AnswerSet, Name, Sign, Symbol};

    let chosen = Symbol::Function {
        name: Name::new("chosen")?,
        arguments: vec![1.into()],
        sign: Sign::Positive,
    };
    assert_eq!(
        execution::enumerate(program()?)?,
        BTreeMap::from([
            (AnswerSet::new(), vec![]),
            (AnswerSet::from([chosen]), vec![1.into()]),
        ])
    );
    Ok(())
}
