//! Prepare a canonical program for lazy enumeration with separate shown terms.

#[path = "shared/relational-program.rs"]
mod execution;

// ANCHOR: example
use std::sync::Arc;
use zetesis::{Program, program};

fn program() -> Arc<Program> {
    Arc::new(program! {
        { chosen(1) }.
        #show.
        #show 1 : chosen(1).
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for (atoms, terms) in execution::enumerate(program())? {
        println!("full atoms: {atoms:?}; #show terms: {terms:?}");
    }
    Ok(())
}
// ANCHOR_END: example

#[test]
fn macro_lazy_answers_preserve_shown_terms() -> Result<(), Box<dyn std::error::Error>> {
    use std::collections::BTreeMap;
    use zetesis::program::{AnswerSet, Name, Sign, Symbol};

    let chosen = Symbol::Function {
        name: Name::new("chosen")?,
        arguments: vec![1.into()],
        sign: Sign::Positive,
    };
    assert_eq!(
        execution::enumerate(program())?,
        BTreeMap::from([
            (AnswerSet::new(), vec![]),
            (AnswerSet::from([chosen]), vec![1.into()]),
        ])
    );
    Ok(())
}
