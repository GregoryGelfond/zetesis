//! Direct stratified evaluation retains complete original answer identity.

use crate::support::models::cost_family as family;
use std::num::NonZeroUsize;

use zetesis_cpu::Cancellation;
use zetesis_reference_support::formula;
use zetesis_solve::{
    Backend, Completion, Grounder, Oracle, PreparedInput, Session, SolveConfig, WorldView,
    WorldViewLimits,
};
use zetesis_stable::CertificatePlanStatistics;
use zetesis_themelios::AdmittedFormula;

fn config(workers: usize, oracle: Oracle) -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        grounder: Grounder::Eager,
        models: 0,
        workers: NonZeroUsize::new(workers).unwrap(),
        oracle,
        ..SolveConfig::default()
    }
}

fn collect(owner: &AdmittedFormula, config: SolveConfig) -> WorldView {
    Session::builder(
        PreparedInput::formula(owner),
        config,
        Cancellation::default(),
    )
    .collect(WorldViewLimits::default())
    .unwrap()
}

#[test]
fn stratified_sessions_preserve_the_complete_family() {
    for source in [
        include_str!("../fixtures/stratified/cycle.lp"),
        include_str!("../fixtures/stratified/constraints.lp"),
        include_str!("../fixtures/stratified/inconsistent.lp"),
        include_str!("../fixtures/stratified/objective.lp"),
    ] {
        let owner = formula(source);
        let reference = collect(&owner, config(1, Oracle::Countermodel));
        assert_eq!(
            reference.outcome().completion(),
            Some(Completion::Exhausted)
        );
        for workers in [1, 3] {
            // Keep the certificate-specific assertions on that route by
            // declining optional arithmetic through its existing byte allowance.
            let actual = collect(
                &owner,
                SolveConfig {
                    max_reduct_bytes: 0,
                    ..config(workers, Oracle::Auto)
                },
            );
            assert_eq!(actual.outcome().completion(), Some(Completion::Exhausted));
            let statistics = actual.outcome().countermodel_statistics().unwrap();
            assert!(matches!(
                statistics.certified.unwrap().plan,
                Some(CertificatePlanStatistics::Stratified(_))
            ));
            assert_eq!(
                statistics.regions.unwrap().parity.status,
                zetesis_ferraris::PreparationStatus::StorageDeclined,
            );
            assert_eq!(statistics.countermodel_queries, 0);
            assert_eq!(statistics.search.decisions, 0);
            assert!(statistics.candidates <= 1);
            assert_eq!(
                family(&actual),
                family(&reference),
                "workers={workers}; {source}"
            );
            let automatic = collect(&owner, config(workers, Oracle::Auto));
            assert_eq!(
                automatic.outcome().completion(),
                Some(Completion::Exhausted)
            );
            assert_eq!(
                family(&automatic),
                family(&reference),
                "workers={workers}; {source}"
            );
        }
    }
}

#[test]
fn negative_recursion_keeps_the_existing_checker() {
    let owner = formula(include_str!("../fixtures/stratified/negative-cycle.lp"));
    let actual = collect(&owner, config(3, Oracle::Auto));
    assert_eq!(actual.outcome().completion(), Some(Completion::Exhausted));
    assert_eq!(actual.answer_sets().len(), 2);
    assert!(matches!(
        actual
            .outcome()
            .countermodel_statistics()
            .unwrap()
            .certified
            .unwrap()
            .plan,
        Some(CertificatePlanStatistics::Tight(_))
    ));
}

#[test]
fn arithmetic_refutation_precedes_stratified_certificate_selection() {
    let owner = formula(include_str!("../fixtures/stratified/inconsistent.lp"));
    for workers in [1, 3] {
        let view = collect(&owner, config(workers, Oracle::Auto));
        assert_eq!(view.outcome().completion(), Some(Completion::Exhausted));
        assert!(view.outcome().unsatisfiable());
        assert!(family(&view).is_empty());
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
