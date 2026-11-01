//! Complete public formula sessions consume whole-theory positive certificates.

use crate::support::models::{CostFamily as Family, cost_family as family};
use std::{collections::BTreeSet, convert::Infallible, num::NonZeroUsize};

use zetesis_algebra::Cancellation;
use zetesis_ferraris::{NodeView, PositiveError, PositiveResource, TightError, TightResource};
use zetesis_reference_support::formula;
use zetesis_solve::{
    AnswerSelection, Backend, Completion, ExecutionObservation, ExecutionObserver, Grounder,
    Interruption, Oracle, PreparedInput, Session, SolveConfig, WorldView, WorldViewLimits,
};
use zetesis_stable::CertificatePlanStatistics;
use zetesis_test_support::programs::model;
use zetesis_themelios::{AdmittedFormula, AnalysisBasis, analysis::classify::HornKind};

#[derive(Debug, PartialEq, Eq)]
enum Membership {
    Positive,
    Tight,
    General,
}

#[derive(Default)]
struct Observations {
    membership: Vec<Membership>,
    completion_workers: Option<NonZeroUsize>,
    cancel_after_positive: Option<Cancellation>,
}

impl ExecutionObserver for Observations {
    type Error = Infallible;

    fn observe(&mut self, observation: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        match observation {
            ExecutionObservation::PositiveMembership => {
                self.membership.push(Membership::Positive);
                if let Some(cancellation) = &self.cancel_after_positive {
                    cancellation.cancel();
                }
            }
            ExecutionObservation::TightMembership => self.membership.push(Membership::Tight),
            ExecutionObservation::GeneralMembership(_) => self.membership.push(Membership::General),
            ExecutionObservation::ExactCompletion { workers, .. } => {
                self.completion_workers = Some(workers);
            }
            _ => {}
        }
        Ok(())
    }
}

/// One region walker, whose leaves a completion pool of `workers` checks:
/// the pool these sessions observe.
fn config(workers: usize, oracle: Oracle) -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        oracle,
        grounder: Grounder::Eager,
        models: 0,
        workers: NonZeroUsize::MIN,
        completion_workers: NonZeroUsize::new(workers).unwrap(),
        batch_size: NonZeroUsize::new(4).unwrap(),
        ..SolveConfig::default()
    }
}

/// `workers` region walkers, which decide their leaves themselves, by the
/// same certificates, as zetesis-stable qualifies.
fn walkers(workers: usize, oracle: Oracle) -> SolveConfig {
    SolveConfig {
        workers: NonZeroUsize::new(workers).unwrap(),
        ..config(workers, oracle)
    }
}

fn collect(owner: &AdmittedFormula, config: SolveConfig) -> (WorldView, Observations) {
    let mut observations = Observations::default();
    let view = Session::builder(
        PreparedInput::formula(owner),
        config,
        Cancellation::default(),
    )
    .collect_observed(WorldViewLimits::default(), &mut observations)
    .unwrap();
    assert_eq!(view.outcome().selection(), Some(AnswerSelection::All));
    assert_eq!(view.outcome().completion(), Some(Completion::Exhausted));
    (view, observations)
}

fn positive(view: &WorldView, observations: &Observations, workers: usize) {
    assert_eq!(observations.membership, [Membership::Positive]);
    let statistics = view.outcome().countermodel_statistics().unwrap();
    let certified = statistics.certified.unwrap();
    let Some(CertificatePlanStatistics::Positive(plan)) = certified.plan else {
        panic!("ordinary session did not retain a positive certificate: {certified:?}");
    };
    assert!(plan.retained_bytes > 0);
    assert_eq!(certified.refuted, 0);
    assert_eq!(certified.failed, 0);
    assert_eq!(statistics.countermodel_queries, 0);
    // The least interpretation is the only candidate; a violated constraint
    // leaves none.
    assert!(statistics.candidates <= 1);
    if workers == 1 {
        assert!(observations.completion_workers.is_none());
        assert!(view.outcome().formula_execution().is_none());
    } else {
        assert_eq!(observations.completion_workers.unwrap().get(), workers);
        let execution = view.outcome().formula_execution().unwrap();
        assert_eq!(execution.completion.requested_workers, workers);
        assert_eq!(execution.cpu_residuals, 0);
        assert!(execution.gpu_limits.is_none());
    }
}

#[test]
fn positive_sessions_return_the_literal_least_families() {
    // Source cycles with a seed survive support grounding. The unseeded cycle
    // and empty source also check the singleton empty interpretation boundary.
    for (source, names) in [
        ("", &[][..]),
        ("a.", &["a"][..]),
        ("a:-b. b:-a.", &[][..]),
        ("a. b:-a. a:-b.", &["a", "b"][..]),
        ("a. b:-a. c:-c.", &["a", "b"][..]),
    ] {
        let owner = formula(source);
        assert_eq!(owner.analysis_basis(), AnalysisBasis::NormalizedProgram);
        assert!(matches!(
            owner.source_analysis().classes().horn(),
            HornKind::Horn
        ));
        let expected = BTreeSet::from([(model(names), None)]);
        for workers in [1, 4] {
            let (actual, observations) = collect(
                &owner,
                SolveConfig {
                    // This route must not depend on even an empty reduct allocation.
                    max_reduct_bytes: 0,
                    ..config(workers, Oracle::Auto)
                },
            );
            positive(&actual, &observations, workers);
            assert_eq!(
                family(&actual),
                expected,
                "source={source}, workers={workers}"
            );
            let (reference, events) = collect(&owner, config(workers, Oracle::Countermodel));
            assert!(events.membership.is_empty());
            assert!(
                reference
                    .outcome()
                    .countermodel_statistics()
                    .unwrap()
                    .certified
                    .is_none()
            );
            assert_eq!(
                family(&reference),
                expected,
                "source={source}, workers={workers}"
            );
        }
    }
}

#[test]
fn positive_regions_complete_without_candidate_search() {
    for (source, names) in [
        ("", &[][..]),
        ("a:-b. b:-a.", &[][..]),
        ("a. b:-a. a:-b.", &["a", "b"][..]),
    ] {
        let owner = formula(source);
        let expected = BTreeSet::from([(model(names), None)]);
        for workers in [1, 4] {
            let (view, observations) = collect(
                &owner,
                SolveConfig {
                    max_reduct_bytes: 0,
                    ..walkers(workers, Oracle::Auto)
                },
            );
            assert_eq!(family(&view), expected);
            assert_eq!(observations.membership, [Membership::Positive]);
            let statistics = view.outcome().countermodel_statistics().unwrap();
            assert_eq!(statistics.candidates, 1);
            assert_eq!(statistics.countermodel_queries, 0);
            assert_eq!(statistics.search.decisions, 0);
            assert_eq!(statistics.regions.unwrap().counts.regions, 0);
            assert_eq!(statistics.certified.unwrap().stable, 1);
        }
    }
}

#[test]
fn positive_regions_preserve_scored_observations() {
    let owner = formula("a. b:-a. a:-b. #minimize {2@3,k:b;1@1,k:a}. #show a/0.");
    let expected = BTreeSet::from([(model(&["a", "b"]), Some(vec![(3, 2), (1, 1)]))]);
    for workers in [1, 4] {
        let (view, observations) = collect(&owner, walkers(workers, Oracle::Auto));
        assert_eq!(observations.membership, [Membership::Positive]);
        assert_eq!(family(&view), expected);
        assert_eq!(view.outcome().scored_models(), 1);
        assert!(
            view.answer_sets()[0]
                .interpretation()
                .catalog()
                .same_owner(owner.atom_catalog())
        );
        assert_eq!(
            view.outcome()
                .countermodel_statistics()
                .unwrap()
                .search
                .decisions,
            0
        );
    }
}

#[test]
fn positive_sessions_preserve_original_constraints() {
    for (source, expected) in [
        ("a. b:-a. a:-b. :-b.", Family::new()),
        (":-.", Family::new()),
        ("a. :-b.", BTreeSet::from([(model(&["a"]), None)])),
    ] {
        let owner = formula(source);
        for workers in [1, 4] {
            // Decline optional root arithmetic through its existing allowance,
            // keeping this test on the positive-certificate constraint path.
            let (actual, observations) = collect(
                &owner,
                SolveConfig {
                    max_reduct_bytes: 0,
                    ..config(workers, Oracle::Auto)
                },
            );
            positive(&actual, &observations, workers);
            assert_eq!(
                actual
                    .outcome()
                    .countermodel_statistics()
                    .unwrap()
                    .regions
                    .unwrap()
                    .parity
                    .status,
                zetesis_ferraris::PreparationStatus::StorageDeclined,
            );
            let (automatic, _) = collect(&owner, config(workers, Oracle::Auto));
            let (reference, _) = collect(&owner, config(workers, Oracle::Countermodel));
            assert_eq!(
                family(&actual),
                expected,
                "source={source}, workers={workers}"
            );
            assert_eq!(family(&reference), expected);
            assert_eq!(family(&automatic), expected);
            assert_eq!(actual.outcome().unsatisfiable(), expected.is_empty());
            assert_eq!(automatic.outcome().unsatisfiable(), expected.is_empty());
        }
    }
}

#[test]
fn positive_answers_retain_original_atoms_and_full_scores() {
    let owner = formula("a. b:-a. a:-b. #minimize {2@3,k:b;1@1,k:a}. #show a/0.");
    let catalog = owner.atom_catalog().clone();
    let expected = BTreeSet::from([(model(&["a", "b"]), Some(vec![(3, 2), (1, 1)]))]);
    let mut retained = Vec::new();
    for workers in [1, 4] {
        let (actual, observations) = collect(&owner, config(workers, Oracle::Auto));
        positive(&actual, &observations, workers);
        let (reference, _) = collect(&owner, config(workers, Oracle::Countermodel));
        for view in [&actual, &reference] {
            assert_eq!(family(view), expected);
            assert_eq!(view.outcome().scored_models(), 1);
            assert!(
                view.answer_sets()[0]
                    .interpretation()
                    .catalog()
                    .same_owner(&catalog)
            );
        }
        retained.push(actual);
    }
    drop(owner);
    for view in retained {
        assert_eq!(family(&view), expected);
        assert!(
            view.answer_sets()[0]
                .interpretation()
                .catalog()
                .same_owner(&catalog)
        );
    }
}

#[test]
fn optional_certificate_byte_refusal_keeps_general_solving() {
    let owner = formula("a. b:-a. a:-b.");
    let (actual, observations) = collect(
        &owner,
        SolveConfig {
            // Scalar residual storage has its independent nonzero reduct allowance.
            max_completion_scratch_bytes: 0,
            ..config(1, Oracle::Auto)
        },
    );
    assert_eq!(observations.membership, [Membership::General]);
    let statistics = actual.outcome().countermodel_statistics().unwrap();
    let certified = statistics.certified.unwrap();
    assert!(certified.plan.is_none());
    assert!(matches!(certified.positive_refusal,
        Some(PositiveError::Limit { resource: PositiveResource::Bytes, observed, limit: 0 }) if observed > 0));
    assert_eq!(
        certified.tight_refusal,
        Some(TightError::Limit(TightResource::Bytes))
    );
    let attempt = certified.positive_attempt.unwrap();
    assert!(attempt.peak_bytes > 0);
    assert_eq!(attempt.retained_bytes, 0);
    assert!(statistics.search.work >= certified.construction_work);
    assert!(statistics.countermodel_queries > 0);
    let (reference, _) = collect(&owner, config(1, Oracle::Countermodel));
    let expected = BTreeSet::from([(model(&["a", "b"]), None)]);
    assert_eq!(family(&actual), expected);
    assert_eq!(family(&reference), expected);
}

#[test]
fn disjunctive_source_declines_both_atomic_head_plans() {
    let owner = formula("a|b.");
    assert_eq!(owner.analysis_basis(), AnalysisBasis::NormalizedProgram);
    assert!(!matches!(
        owner.source_analysis().classes().horn(),
        HornKind::Horn
    ));
    let root = *owner
        .theory()
        .roots()
        .iter()
        .find(|&&root| match owner.theory().view().node(root).unwrap() {
            NodeView::Or(_) => true,
            NodeView::Implies(_, head) => {
                matches!(owner.theory().view().node(head).unwrap(), NodeView::Or(_))
            }
            _ => false,
        })
        .unwrap();
    let expected = BTreeSet::from([(model(&["a"]), None), (model(&["b"]), None)]);
    for workers in [1, 4] {
        let (actual, observations) = collect(&owner, config(workers, Oracle::Auto));
        assert_eq!(observations.membership, [Membership::General]);
        let statistics = actual.outcome().countermodel_statistics().unwrap();
        let certified = statistics.certified.unwrap();
        assert!(certified.plan.is_none());
        assert_eq!(
            certified.tight_refusal,
            Some(TightError::UnsupportedRoot { root })
        );
        assert_eq!(
            certified.positive_refusal,
            Some(PositiveError::UnsupportedRoot { root })
        );
        assert!(statistics.countermodel_queries > 0);
        let (reference, _) = collect(&owner, config(workers, Oracle::Countermodel));
        assert_eq!(family(&actual), expected);
        assert_eq!(family(&reference), expected);
    }
}

#[test]
fn cancellation_after_positive_preparation_is_incomplete() {
    let owner = formula("a. b:-a. a:-b.");
    let cancellation = Cancellation::default();
    let mut observations = Observations {
        cancel_after_positive: Some(cancellation.clone()),
        ..Observations::default()
    };
    let failure = Session::builder(
        PreparedInput::formula(&owner),
        config(1, Oracle::Auto),
        cancellation,
    )
    .collect_observed(WorldViewLimits::default(), &mut observations)
    .unwrap_err();
    assert_eq!(observations.membership, [Membership::Positive]);
    assert!(failure.answer_sets().is_empty());
    let outcome = failure.outcome().unwrap();
    assert_eq!(outcome.completion(), Some(Completion::Interrupted));
    assert_eq!(
        outcome.interruption(),
        Some(Interruption::Countermodel(
            zetesis_stable::Incomplete::Cancelled
        ))
    );
    assert!(!outcome.unsatisfiable());
    let statistics = outcome.countermodel_statistics().unwrap();
    assert!(matches!(
        statistics.certified.unwrap().plan,
        Some(CertificatePlanStatistics::Positive(_))
    ));
    assert_eq!(statistics.candidates, 0);
    assert_eq!(statistics.countermodel_queries, 0);
}

#[test]
fn arithmetic_refutation_precedes_positive_certificate_selection() {
    let owner = formula("a. b:-a. a:-b. :-b.");
    for workers in [1, 4] {
        let (view, observations) = collect(&owner, config(workers, Oracle::Auto));
        assert!(family(&view).is_empty());
        assert!(view.outcome().unsatisfiable());
        assert!(observations.membership.is_empty());
        let statistics = view.outcome().countermodel_statistics().unwrap();
        let regions = statistics.regions.unwrap();
        assert!(statistics.certified.is_none());
        assert_eq!(
            regions.parity.status,
            zetesis_ferraris::PreparationStatus::Prepared
        );
        assert!(regions.parity.root_refuted);
        assert_eq!(regions.counts.regions, 0);
        assert_eq!(statistics.candidates, 0);
        assert_eq!(statistics.countermodel_queries, 0);
        assert_eq!(statistics.search.decisions, 0);
    }
}
