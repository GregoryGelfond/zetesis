//! Optional pruning must preserve the original reduct and recover transactionally.

use std::collections::BTreeSet;

use zetesis_core::Model;
use zetesis_cpu::Control;
use zetesis_objective::Score;
use zetesis_sat::{Limits, StableModels};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, admit_formula,
};

use super::Bounds;
use crate::countermodel::Input;
use crate::execution_observation::Observer;
use crate::{ExecutionObservation, ExecutionObserver, SolveConfig};

/// Enumerate by the clause forms, the subject of the tests below.
fn by_clauses(
    theory: &zetesis_ferraris::Theory,
    limits: zetesis_sat::Limits,
    control: zetesis_sat::Control,
) -> Result<zetesis_sat::StableModels, zetesis_sat::Incomplete> {
    zetesis_sat::StableModels::with_method(
        theory,
        zetesis_sat::SearchMethod::Clauses,
        limits,
        control,
    )
}

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
        atoms: admitted.atom_catalog(),
        gate_atoms: 0,
        keyed_constraints: 0,
        key_analysis: zetesis_themelios::KeyAnalysis::Complete,
        objectives: admitted.objectives(),
        certificate_order: zetesis_sat::CertificateOrder::TightFirst,
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
    let options = SolveConfig::default();
    let mut observations = Observations::default();
    let mut bounds = Bounds::new(
        input(&planned),
        &options,
        &mut Observer(&mut observations),
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
            &options,
            &mut Observer(&mut observations),
            &Control::default(),
        )
        .unwrap();
    assert!(bounds.plan.is_none());
    assert_eq!(original.theory().nodes(), nodes);
    assert_eq!(observations.mismatches, 1);
    assert!(observations.restrictions.is_empty());
    let previous = observations.clone();
    bounds
        .improve(
            &score(&planned, &[]),
            &mut models,
            &options,
            &mut Observer(&mut observations),
            &Control::default(),
        )
        .unwrap();
    assert_eq!(
        observations, previous,
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
    let options = SolveConfig::default();
    let mut observations = Observations::default();
    let mut bounds = Bounds::new(
        input(&admitted),
        &options,
        &mut Observer(&mut observations),
        &Control::default(),
    )
    .unwrap();
    assert!(bounds.plan.is_some());
    // The original independent choices need no auxiliary candidate variables.
    // The cost guard does; refusing it must roll back that candidate owner.
    // Exact reduct checking has a separate admission allowance.
    let limits = Limits {
        admission: zetesis_sat::AdmissionLimits {
            max_variables: admitted.atoms().len(),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut models = by_clauses(admitted.theory(), limits, Control::default()).unwrap();
    let original_nodes = models.theory().nodes().to_vec();
    bounds
        .improve(
            &score(&admitted, &["a"]),
            &mut models,
            &options,
            &mut Observer(&mut observations),
            &Control::default(),
        )
        .unwrap();
    assert!(bounds.plan.is_none());
    assert_eq!(models.statistics().candidate_restrictions, 0);
    assert!(models.theory().same_instance(admitted.theory()));
    assert_eq!(models.theory().nodes(), original_nodes);
    assert_eq!(observations.mismatches, 0);
    assert!(matches!(
        observations.restrictions.as_slice(),
        [zetesis_sat::Incomplete::Admission(
            zetesis_sat::AdmissionError::Limit {
                resource: zetesis_sat::Resource::Variables,
                ..
            }
        )]
    ));
    let actual = complete(&mut models);
    let mut baseline = by_clauses(admitted.theory(), limits, Control::default()).unwrap();
    assert_eq!(actual, complete(&mut baseline));
    assert_eq!(
        actual,
        BTreeSet::from([vec![], vec![0], vec![1], vec![0, 1]])
    );
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Observations {
    mismatches: usize,
    restrictions: Vec<zetesis_sat::Incomplete>,
}

impl ExecutionObserver for Observations {
    type Error = std::convert::Infallible;

    fn observe(&mut self, observation: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        match observation {
            ExecutionObservation::ObjectiveTheoryMismatch => self.mismatches += 1,
            ExecutionObservation::ObjectiveRestrictionStopped(cause) => {
                self.restrictions.push(cause);
            }
            _ => panic!("these plans must reach their specific bound refusal"),
        }
        Ok(())
    }
}
