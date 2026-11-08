//! Observer failure must preserve the original candidate search after a bound refusal.

use std::collections::BTreeSet;
use std::io;

use zetesis_algebra::Cancellation;
use zetesis_core::Model;
use zetesis_ferraris::Interpretation;
use zetesis_reference_support::formula as admitted;
use zetesis_stable::{Limits, StableModels};

use super::{Bounds, Preparation};
use crate::countermodel::Input;
use crate::execution_observation::Observer;
use crate::{ExecutionObservation, ExecutionObserver, SolveConfig, SolveError};

fn attempt(
    foreign: bool,
    observer: &mut impl ExecutionObserver,
) -> (Result<(), SolveError>, StableModels) {
    let planned = admitted(super::HEAVY_OBJECTIVE);
    let different = foreign.then(|| admitted("{x;y}."));
    let original = different.as_ref().unwrap_or(&planned);
    let options = SolveConfig::default();
    let candidate = Interpretation::new(planned.theory(), [0]).unwrap();
    assert!(
        zetesis_ferraris::check(
            planned.theory(),
            &candidate,
            zetesis_ferraris::Limits::default(),
            &Cancellation::default()
        )
        .unwrap()
        .accepted()
    );
    let model = Model::from_positions(planned.atom_catalog(), candidate.atoms()).unwrap();
    let score = zetesis_objective::evaluate(
        planned.objectives(),
        &model,
        zetesis_objective::Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let preparation = Preparation::new(
        Input {
            theory: planned.theory(),
            atoms: planned.atom_catalog(),
            keyed_constraints: 0,
            key_analysis: zetesis_themelios::KeyAnalysis::Complete,
            objectives: planned.objectives(),
            required_choices: planned.required_choices(),
            certificate_order: zetesis_stable::CertificateOrder::TightFirst,
        },
        &options,
        &Cancellation::default(),
    );
    preparation
        .observe(&mut crate::execution_observation::Ignore)
        .unwrap();
    let mut bounds = Bounds::new(&options);
    assert!(preparation.plan().is_some());
    let limits = if foreign {
        Limits::default()
    } else {
        super::exact_search_limits(original.theory())
    };
    let mut models = StableModels::new(original.theory(), limits, Cancellation::default()).unwrap();
    let result = bounds.improve(
        preparation.plan(),
        score.score(),
        &mut models,
        &options,
        &mut Observer(observer),
        &Cancellation::default(),
    );
    assert_eq!(models.statistics().candidate_restrictions, 0);
    assert!(models.theory().same_instance(original.theory()));
    assert!(
        !models.exhausted(),
        "a refused bound did not exhaust original search"
    );
    (result, models)
}

fn all_original_models(mut models: StableModels) {
    let actual: BTreeSet<Vec<usize>> = models
        .by_ref()
        .map(|result| result.unwrap().atoms().collect())
        .collect();
    assert!(models.exhausted());
    assert_eq!(
        actual,
        BTreeSet::from([vec![], vec![0], vec![1], vec![0, 1]])
    );
}

struct RefuseObservation {
    foreign: bool,
    fail: bool,
    calls: usize,
}

impl ExecutionObserver for RefuseObservation {
    type Error = io::Error;

    fn observe(&mut self, observation: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        self.calls += 1;
        if self.foreign {
            assert!(matches!(
                observation,
                ExecutionObservation::ObjectiveTheoryMismatch
            ));
        } else {
            assert!(matches!(
                observation,
                ExecutionObservation::ObjectiveRestrictionStopped(
                    zetesis_stable::Incomplete::WorkLimit
                )
            ));
        }
        if self.fail {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "bound observer closed",
            ));
        }
        Ok(())
    }
}

fn check_refusal(foreign: bool) {
    for fail in [false, true] {
        let mut observer = RefuseObservation {
            foreign,
            fail,
            calls: 0,
        };
        let (result, models) = attempt(foreign, &mut observer);
        assert_eq!(observer.calls, 1);
        if fail {
            let SolveError::ExecutionObservation(cause) = result.unwrap_err() else {
                panic!("the original observer failure must remain external")
            };
            let error = cause.downcast_ref::<io::Error>().unwrap();
            assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
            assert_eq!(error.to_string(), "bound observer closed");
        } else {
            result.unwrap();
        }
        all_original_models(models);
    }
}

#[test]
fn foreign_bound_observer_failure_preserves_search() {
    check_refusal(true);
}

#[test]
fn restriction_observer_failure_preserves_search() {
    check_refusal(false);
}
