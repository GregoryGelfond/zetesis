//! Check the complete family using retained producers and streamed constraints.

// ANCHOR: example
use std::{collections::BTreeSet, num::NonZeroUsize};
use zetesis_core::{Atom, Model, Predicate, Value};
use zetesis_cpu::Cancellation;
use zetesis_solve::{Backend, Completion, Grounder, Oracle, PreparedInput, Session, SolveConfig};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, prepare_formula};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let owner = prepare_formula(
        "d(1..3). p(X)|q(X):-d(X). :-p(X),q(Y),X<Y.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )?
    .ground_hybrid()?;
    assert_eq!(owner.streamed_templates(), 1);

    let config = SolveConfig {
        backend: Backend::Cpu,
        grounder: Grounder::Lazy,
        oracle: Oracle::Countermodel,
        workers: NonZeroUsize::MIN,
        completion_workers: NonZeroUsize::MIN,
        models: 0,
        ..SolveConfig::default()
    };
    let mut session = Session::enumerate(
        PreparedInput::hybrid(&owner),
        config,
        Cancellation::default(),
    )?;
    let mut family = BTreeSet::new();
    for answer in session.by_ref() {
        assert!(family.insert(answer?.interpretation().clone()));
    }
    let outcome = session.outcome().ok_or("missing session outcome")?;
    assert_eq!(outcome.completion(), Some(Completion::Exhausted));

    // A p before a later q violates the constraint. The four answers choose
    // a q-prefix followed by a p-suffix, and retain every d atom as well.
    let mut expected = BTreeSet::new();
    for cut in 0..=3 {
        let mut atoms = Vec::new();
        for x in 1..=3 {
            atoms.push(Atom::new(Predicate::new("d", 1)?, vec![Value::Number(x)])?);
            let name = if x <= cut { "q" } else { "p" };
            atoms.push(Atom::new(Predicate::new(name, 1)?, vec![Value::Number(x)])?);
        }
        expected.insert(Model::new(atoms));
    }
    assert_eq!(family, expected);
    let checked = outcome.hybrid_execution().ok_or("missing hybrid receipt")?;
    assert_eq!(checked.core_answers, 8);
    assert_eq!(checked.accepted, 4);
    assert_eq!(checked.rejected, 4);
    assert_eq!(checked.pending, 0);
    Ok(())
}
// ANCHOR_END: example

#[test]
fn streamed_constraints_preserve_the_example_family() {
    main().unwrap();
}
