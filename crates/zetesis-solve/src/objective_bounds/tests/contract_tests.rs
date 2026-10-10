//! Optional pruning must preserve the original reduct and recover transactionally.

use std::collections::BTreeSet;

use super::by_clauses;
use zetesis_core::Model;
use zetesis_cpu::Cancellation;
use zetesis_objective::Score;
use zetesis_sat::{Limits, StableModels};
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, ExpansionLimits, FormulaLimits, admit_formula,
};

use super::{Bounds, Preparation};
use crate::countermodel::Input;
use crate::execution_observation::Observer;
use crate::{ExecutionObservation, ExecutionObserver, SolveConfig};

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
        required_choices: admitted.required_choices(),
        certificate_order: zetesis_sat::CertificateOrder::TightFirst,
    }
}

fn score(admitted: &AdmittedFormula, names: &[&str]) -> Score {
    let model = Model::from_positions(
        admitted.atom_catalog(),
        admitted
            .atoms()
            .iter()
            .enumerate()
            .filter_map(|(position, atom)| {
                names.contains(&atom.predicate().name()).then_some(position)
            }),
    )
    .unwrap();
    zetesis_objective::evaluate(
        admitted.objectives(),
        &model,
        zetesis_objective::Limits::default(),
        &Cancellation::default(),
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
    let preparation = Preparation::new(input(&planned), &options, &Cancellation::default());
    preparation
        .observe(&mut Observer(&mut observations))
        .unwrap();
    let mut bounds = Bounds::new(&options);
    assert!(preparation.plan().is_some());
    let nodes = original.theory().nodes().to_vec();
    let mut models = StableModels::new(
        original.theory(),
        Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
    bounds
        .improve(
            preparation.plan(),
            &score(&planned, &["a"]),
            &mut models,
            &options,
            &mut Observer(&mut observations),
            &Cancellation::default(),
        )
        .unwrap();
    assert!(!bounds.enabled);
    assert!(preparation.plan().is_some());
    assert_eq!(original.theory().nodes(), nodes);
    assert_eq!(observations.mismatches, 1);
    assert!(observations.restrictions.is_empty());
    let previous = observations.clone();
    bounds
        .improve(
            preparation.plan(),
            &score(&planned, &[]),
            &mut models,
            &options,
            &mut Observer(&mut observations),
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(
        observations, previous,
        "a rejected plan must remain disabled"
    );
    let actual = complete(&mut models);
    let mut baseline = StableModels::new(
        original.theory(),
        Limits::default(),
        Cancellation::default(),
    )
    .unwrap();
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
    let preparation = Preparation::new(input(&admitted), &options, &Cancellation::default());
    preparation
        .observe(&mut Observer(&mut observations))
        .unwrap();
    let mut bounds = Bounds::new(&options);
    assert!(preparation.plan().is_some());
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
    let mut models = by_clauses(admitted.theory(), limits, Cancellation::default()).unwrap();
    let original_nodes = models.theory().nodes().to_vec();
    bounds
        .improve(
            preparation.plan(),
            &score(&admitted, &["a"]),
            &mut models,
            &options,
            &mut Observer(&mut observations),
            &Cancellation::default(),
        )
        .unwrap();
    assert!(!bounds.enabled);
    assert!(preparation.plan().is_some());
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
    let mut baseline = by_clauses(admitted.theory(), limits, Cancellation::default()).unwrap();
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
