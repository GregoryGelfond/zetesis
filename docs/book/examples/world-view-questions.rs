//! Decide questions about a world view without collecting the family.

// ANCHOR: example
use std::num::NonZeroUsize;
use zetesis_core::{Atom, ConstructionError, Predicate};
use zetesis_cpu::Cancellation;
use zetesis_solve::{Backend, PreparedInput, QueryAnswer, Session, SolveConfig, WorldViewVerdict};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

fn literal(name: &str) -> Result<Atom, ConstructionError> {
    Atom::new(Predicate::new(name, 0)?, Vec::new())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Two answer sets: {r, p} and {r, q, -s}.
    let owner = admit_formula(
        "r. 1 {p; q} 1. -s :- q.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )?;
    let config = SolveConfig {
        backend: Backend::Cpu,
        models: 0,
        workers: NonZeroUsize::MIN,
        ..SolveConfig::default()
    };
    let ask = || {
        Session::builder(
            PreparedInput::formula(&owner),
            config,
            Cancellation::default(),
        )
    };

    // The first answer set containing p decides that p is possible; the
    // session stops there, and that answer set is the witness.
    let possible = ask().possible(&literal("p")?)?;
    let WorldViewVerdict::Decided { value, witnesses } = possible.verdict() else {
        return Err("p is possible as soon as an answer set contains it".into());
    };
    assert!(*value);
    assert!(witnesses[0].interpretation().contains(&literal("p")?));

    // r is in every answer set, which only exhausting the family establishes.
    let known = ask().known(&literal("r")?)?;
    assert!(matches!(known.verdict(), WorldViewVerdict::Complete(true)));

    // The cautious consequences are {r}. s is unknown in {r, p}, which
    // decides the three-valued answer by itself.
    let cautious = ask().cautious()?;
    assert_eq!(
        cautious.verdict().value().map(|atoms| atoms.atoms().len()),
        Some(1)
    );
    let answer = ask().answer(&literal("s")?)?;
    assert_eq!(answer.verdict().value(), Some(&QueryAnswer::Unknown));

    // A program without answer sets has no value for any question.
    let inconsistent = admit_formula(
        "p. :- p.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )?;
    let none = Session::builder(
        PreparedInput::formula(&inconsistent),
        config,
        Cancellation::default(),
    )
    .known(&literal("p")?)?;
    assert!(matches!(none.verdict(), WorldViewVerdict::Inconsistent));
    Ok(())
}
// ANCHOR_END: example
