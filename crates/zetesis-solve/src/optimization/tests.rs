//! Retention failure preserves the last completely admitted incumbent.

use zetesis_core::Model;
use zetesis_cpu::Control;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, admit_formula,
};

use super::{Incumbents, OptimizationStop};
use crate::{Backend, Interruption, PreparedInput, Session, SolveConfig};

fn fixture() -> (AdmittedFormula, Model, Model) {
    let owner = admit_formula(
        "a | b. c :- b. #minimize {2,k:a;1,k:b}.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap();
    // Use genuinely checked answers; order the two verified models explicitly
    // so this retention test does not assume a SAT enumeration order.
    let mut answers =
        Session::enumerate(PreparedInput::formula(&owner), config(), Control::default())
            .unwrap()
            .map(|answer| answer.unwrap().into_interpretation())
            .collect::<Vec<_>>();
    answers.sort_by_key(|model| model.atoms().len());
    assert_eq!(
        answers.iter().map(|m| m.atoms().len()).collect::<Vec<_>>(),
        [1, 2]
    );
    let second = answers.pop().unwrap();
    let first = answers.pop().unwrap();
    (owner, first, second)
}

fn config() -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        models: 0,
        ..SolveConfig::default()
    }
}

#[test]
fn refused_improvement_preserves_the_old_incumbent() {
    let (owner, first, second) = fixture();
    // Three nullary catalog records, the larger two-position selection, and
    // one score record. This refuses the improvement by exactly one byte.
    let required = 8 + 3 * 18 + 8 + 2 * 8 + 21;
    let options = SolveConfig {
        max_optimal_bytes: required - 1,
        ..config()
    };
    let mut retained = Incumbents::default();
    assert!(
        retained
            .consider(
                owner.objectives(),
                first.clone(),
                &options,
                &Control::default()
            )
            .unwrap()
    );
    let before = retained.retention.payload();
    assert!(matches!(
        retained.consider(owner.objectives(), second, &options, &Control::default()),
        Err(Interruption::Incumbent(OptimizationStop::Bytes)),
    ));
    assert_eq!(retained.models, [first]);
    assert_eq!(retained.retention.payload(), before);
    let best = retained.metadata().unwrap();
    assert_eq!(best.score.costs(), [(0, 2)]);
    assert_eq!(best.tied_models, 1);
    assert_eq!(best.scored_models, 2);
}

#[test]
fn improvement_replaces_the_complete_charge() {
    let (owner, first, second) = fixture();
    let required = 8 + 3 * 18 + 8 + 2 * 8 + 21;
    let options = SolveConfig {
        max_optimal_bytes: required,
        ..config()
    };
    let mut retained = Incumbents::default();
    retained
        .consider(owner.objectives(), first, &options, &Control::default())
        .unwrap();
    assert!(
        retained
            .consider(
                owner.objectives(),
                second.clone(),
                &options,
                &Control::default()
            )
            .unwrap()
    );
    assert_eq!(retained.models, [second]);
    assert_eq!(retained.retention.payload().bytes, required);
    assert_eq!(retained.retention.payload().catalogs, 1);
    assert_eq!(retained.retention.payload().associated_bytes, 21);
    let best = retained.metadata().unwrap();
    assert_eq!(best.score.costs(), [(0, 1)]);
    assert_eq!(best.tied_models, 1);
    assert_eq!(best.scored_models, 2);
}
