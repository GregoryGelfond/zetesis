extern crate zetesis_cli;
extern crate zetesis_core;
extern crate zetesis_cpu;
extern crate zetesis_themelios;

// ANCHOR: example
use std::collections::BTreeSet;
use zetesis_cli::{Backend, Completion, PreparedInput, Session, SolveConfig};
use zetesis_core::{Atom, Predicate};
use zetesis_cpu::Control;
use zetesis_themelios::{AdmissionOptions, admit};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let admitted = admit(
        "a :- not b. b :- not a.".into(),
        AdmissionOptions::default(),
    )?;
    let config = SolveConfig {
        backend: Backend::Cpu,
        models: 0,
        ..SolveConfig::default()
    };
    let mut session = Session::new(
        PreparedInput::admitted(&admitted),
        config,
        Control::default(),
    )?;
    let answers = session.by_ref().collect::<Result<Vec<_>, _>>()?;
    let outcome = session.outcome().expect("the iterator reached its end");

    assert_eq!(answers.len(), 2);
    let family = answers
        .iter()
        .map(|answer| answer.interpretation().atoms().clone())
        .collect::<BTreeSet<_>>();
    let a = Atom::new(Predicate::new("a", 0)?, vec![])?;
    let b = Atom::new(Predicate::new("b", 0)?, vec![])?;
    assert_eq!(
        family,
        BTreeSet::from([BTreeSet::from([a]), BTreeSet::from([b])])
    );
    assert_eq!(outcome.verified_models(), 2);
    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
    assert!(!outcome.unsatisfiable());
    Ok(())
}
// ANCHOR_END: example
