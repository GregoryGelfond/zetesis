//! Construct an exact-one choice and enumerate its complete typed answer family.

#[path = "shared/formula-program.rs"]
mod execution;

// ANCHOR: example
use std::sync::Arc;
use zetesis::{Program, program};

fn choice_program() -> Arc<Program> {
    Arc::new(program! {
        1 { chosen(1); chosen(2) } 1.
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for answer in execution::enumerate(choice_program())? {
        println!("{answer:?}");
    }
    Ok(())
}
// ANCHOR_END: example

#[test]
fn macro_choice_has_exactly_two_complete_answers() -> Result<(), Box<dyn std::error::Error>> {
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
        execution::enumerate(choice_program())?,
        BTreeSet::from(expected)
    );
    Ok(())
}
