//! Optional pruning must preserve the original reduct and recover transactionally.

use std::collections::BTreeSet;

use clap::Parser;
use zetesis_core::Model;
use zetesis_cpu::Control;
use zetesis_objective::Score;
use zetesis_sat::{Limits, StableModels};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, admit_formula,
};

use super::Bounds;
use crate::countermodel::Input;
use crate::presentation::Diagnostics;
use crate::{ColorMode, Options};

fn admitted(source: &str) -> AdmittedFormula {
    admit_formula(
        source.to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )
    .unwrap()
}

fn input(admitted: &AdmittedFormula) -> Input<'_> {
    Input {
        theory: admitted.theory(),
        atoms: admitted.atoms(),
        gate_atoms: 0,
        objectives: admitted.objectives(),
        observations: admitted.metadata().observations(),
    }
}

fn score(admitted: &AdmittedFormula, names: &[&str]) -> Score {
    let model = Model::new(
        admitted
            .atoms()
            .iter()
            .filter(|atom| names.contains(&atom.predicate().name()))
            .cloned(),
    );
    zetesis_objective::evaluate(
        admitted.objectives(),
        &model,
        zetesis_objective::Limits::default(),
        &Control::default(),
    )
    .unwrap()
    .score()
    .clone()
}

fn complete(models: &mut StableModels) -> BTreeSet<Vec<usize>> {
    let theory = models.theory().clone();
    let result = models
        .by_ref()
        .map(|model| {
            let model = model.expect("the original exact search remains complete");
            assert!(model.theory().same_instance(&theory));
            model.atoms().collect()
        })
        .collect();
    assert!(models.exhausted());
    assert_eq!(models.statistics().candidate_restrictions, 0);
    result
}

#[test]
fn equal_atom_counts_do_not_authorize_a_bound_from_a_different_theory() {
    let planned = admitted("{a;b}. #minimize{1:a;1:b}.");
    let original = admitted("{x;y}.");
    assert_eq!(
        planned.theory().atom_count(),
        original.theory().atom_count()
    );
    assert!(!planned.theory().same_instance(original.theory()));
    let options = Options::try_parse_from(["zetesis"]).unwrap();
    let mut diagnostics = Vec::new();
    let mut bounds = Bounds::new(
        input(&planned),
        &(&options).into(),
        &mut Diagnostics::new(&mut diagnostics, ColorMode::Never),
        &Control::default(),
    )
    .unwrap();
    assert!(bounds.plan.is_some());
    let nodes = original.theory().nodes().to_vec();
    let mut models =
        StableModels::new(original.theory(), Limits::default(), Control::default()).unwrap();
    bounds
        .improve(
            &score(&planned, &["a"]),
            &mut models,
            &(&options).into(),
            &mut Diagnostics::new(&mut diagnostics, ColorMode::Never),
            &Control::default(),
        )
        .unwrap();
    assert!(bounds.plan.is_none());
    assert_eq!(original.theory().nodes(), nodes);
    assert_eq!(
        String::from_utf8(diagnostics.clone()).unwrap(),
        "Objective pruning stopped: original theory mismatch; exact search continues\n"
    );
    let previous = diagnostics.clone();
    bounds
        .improve(
            &score(&planned, &[]),
            &mut models,
            &(&options).into(),
            &mut Diagnostics::new(&mut diagnostics, ColorMode::Never),
            &Control::default(),
        )
        .unwrap();
    assert_eq!(
        diagnostics, previous,
        "a rejected plan must remain disabled"
    );
    let actual = complete(&mut models);
    let mut baseline =
        StableModels::new(original.theory(), Limits::default(), Control::default()).unwrap();
    assert_eq!(actual, complete(&mut baseline));
    assert_eq!(
        actual,
        BTreeSet::from([vec![], vec![0], vec![1], vec![0, 1]])
    );
}

#[test]
fn bound_capacity_failure_restores_exact_search_and_disables_only_pruning() {
    let admitted = admitted("{a;b}. #minimize{1:a;1:b}.");
    let options = Options::try_parse_from(["zetesis"]).unwrap();
    let mut diagnostics = Vec::new();
    let mut bounds = Bounds::new(
        input(&admitted),
        &(&options).into(),
        &mut Diagnostics::new(&mut diagnostics, ColorMode::Never),
        &Control::default(),
    )
    .unwrap();
    assert!(bounds.plan.is_some());
    // The original independent choices and their reducts need no auxiliary
    // variables. The nontrivial cost guard does; refusing it must roll back.
    let limits = Limits {
        admission: zetesis_sat::AdmissionLimits {
            max_variables: admitted.atoms().len(),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut models = StableModels::new(admitted.theory(), limits, Control::default()).unwrap();
    let original_nodes = models.theory().nodes().to_vec();
    bounds
        .improve(
            &score(&admitted, &["a"]),
            &mut models,
            &(&options).into(),
            &mut Diagnostics::new(&mut diagnostics, ColorMode::Never),
            &Control::default(),
        )
        .unwrap();
    assert!(bounds.plan.is_none());
    assert_eq!(models.statistics().candidate_restrictions, 0);
    assert!(models.theory().same_instance(admitted.theory()));
    assert_eq!(models.theory().nodes(), original_nodes);
    let diagnostic = String::from_utf8(diagnostics).unwrap();
    assert!(
        diagnostic.starts_with("Objective pruning stopped:"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("Variables"), "{diagnostic}");
    assert!(
        diagnostic.ends_with("; exact search continues\n"),
        "{diagnostic}"
    );
    let actual = complete(&mut models);
    let mut baseline = StableModels::new(admitted.theory(), limits, Control::default()).unwrap();
    assert_eq!(actual, complete(&mut baseline));
    assert_eq!(
        actual,
        BTreeSet::from([vec![], vec![0], vec![1], vec![0, 1]])
    );
}
