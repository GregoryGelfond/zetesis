//! Enumerate full answer representatives without changing a world view.

// ANCHOR: example
use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use zetesis_cpu::Cancellation;
use zetesis_solve::{
    Backend, PreparedInput, ProjectionLimits, Session, SolveConfig, WorldViewLimits,
};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let owner = admit_formula(
        "{p;q}. #project p/0. #show.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )?;
    let input = PreparedInput::formula(&owner);
    let config = SolveConfig {
        backend: Backend::Cpu,
        models: 0,
        batch_size: NonZeroUsize::MIN,
        workers: NonZeroUsize::MIN,
        completion_workers: NonZeroUsize::MIN,
        ..SolveConfig::default()
    };
    let full = Session::builder(input, config, Cancellation::default())
        .collect(WorldViewLimits::default())?;
    assert_eq!(full.len(), 4);

    let mut projected = Session::builder(input, config, Cancellation::default())
        .projected(ProjectionLimits::default())
        .start()?;
    let mut keys = BTreeSet::new();
    for answer in projected.by_ref() {
        let answer = answer?;
        assert!(
            full.answer_sets()
                .iter()
                .any(|original| { original.interpretation() == answer.interpretation() })
        );
        let key: Vec<_> = owner
            .projection()
            .atoms()
            .iter()
            .filter(|atom| answer.interpretation().contains(*atom))
            .collect();
        assert!(keys.insert(key));
    }
    assert_eq!(keys.len(), 2);
    let outcome = projected.outcome().expect("finished enumeration");
    let receipt = outcome.projection().expect("explicit projection");
    assert_eq!(receipt.representatives, 2);
    assert_eq!(receipt.duplicates, 2);
    assert!(receipt.complete);
    Ok(())
}
// ANCHOR_END: example
