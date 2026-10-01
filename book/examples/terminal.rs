//! Collect original answers after adaptive terminal-definition materialization.

// ANCHOR: example
use std::{collections::BTreeSet, num::NonZeroUsize};
use zetesis_core::{Atom, Model, Predicate, Value};
use zetesis_cpu::Cancellation;
use zetesis_solve::{
    Backend, Completion, Grounder, PreparedInput, Session, SolveConfig, WorldViewLimits,
};
use zetesis_themelios::{
    AdmissionOptions, ExpansionLimits, FormulaLimits, FormulaMaterialization, prepare_formula,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let materialized = prepare_formula(
        "{seed(1);seed(2)}. receipt(X):-seed(X).".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )?
    .ground_adaptive()?;
    // Keep the materialized owner alive while the session borrows its input.
    let input = match &materialized {
        FormulaMaterialization::Complete(owner) => PreparedInput::formula(owner),
        FormulaMaterialization::Terminal(owner) => PreparedInput::terminal(owner),
    };
    let family = Session::builder(
        input,
        SolveConfig {
            backend: Backend::Cpu,
            grounder: Grounder::Auto,
            workers: NonZeroUsize::MIN,
            completion_workers: NonZeroUsize::MIN,
            models: 0,
            ..SolveConfig::default()
        },
        Cancellation::default(),
    )
    .collect(WorldViewLimits::default())?;
    assert_eq!(family.outcome().completion(), Some(Completion::Exhausted));
    assert_eq!(family.len(), 4);

    // Each chosen seed entails its receipt. Possible support alone does not:
    // the empty choice remains empty, and unchosen seeds gain no receipt.
    let mut expected = BTreeSet::new();
    for mask in 0..4 {
        let mut atoms = Vec::new();
        for number in 1..=2 {
            if mask & (1 << (number - 1)) != 0 {
                for name in ["seed", "receipt"] {
                    atoms.push(Atom::new(
                        Predicate::new(name, 1)?,
                        vec![Value::Number(number)],
                    )?);
                }
            }
        }
        expected.insert(Model::new(atoms)?);
    }
    let actual: BTreeSet<_> = family
        .answer_sets()
        .iter()
        .map(|answer| answer.interpretation().clone())
        .collect();
    assert_eq!(actual, expected);
    let receipt = family
        .outcome()
        .terminal_execution()
        .ok_or("fixture must use terminal reconstruction")?;
    assert_eq!(receipt.base_answers, 4);
    assert_eq!(receipt.reconstructed, 4);
    assert_eq!(receipt.pending, 0);
    Ok(())
}
// ANCHOR_END: example

#[test]
fn adaptive_materialization_preserves_original_answers() {
    main().unwrap();
}
