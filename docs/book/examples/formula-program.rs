//! Construct an exact-one choice and enumerate its complete typed answer family.

#[path = "shared/formula-program.rs"]
mod execution;

// ANCHOR: example
use std::sync::Arc;
use zetesis::program::{
    Atom, Name, Program, Relation, Rule, Term,
    program::{Choice, ChoiceElement, Condition, Guard},
};

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for answer in execution::enumerate(choice_program()?)? {
        println!("{answer:?}");
    }
    Ok(())
}
// ANCHOR_END: example

#[test]
fn constructed_choice_has_exactly_two_complete_answers() -> Result<(), Box<dyn std::error::Error>> {
    use std::collections::BTreeSet;
    use zetesis::program::{AnswerSet, Name, Sign, Symbol};

    let chosen = Name::new("chosen")?;
    let expected = [1, 2].map(|value| {
        AnswerSet::from([Symbol::Function {
            name: chosen.clone(),
            arguments: vec![Symbol::Number(value)],
            sign: Sign::Positive,
        }])
    });
    assert_eq!(
        execution::enumerate(choice_program()?)?,
        BTreeSet::from(expected)
    );
    Ok(())
}
