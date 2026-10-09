//! Direct stratified evaluation retains complete original answer identity.

use std::{collections::BTreeSet, num::NonZeroUsize};

use zetesis_cpu::Cancellation;
use zetesis_reference_support::formula;
use zetesis_sat::CertificatePlanStatistics;
use zetesis_solve::{
    Backend, Completion, Grounder, Oracle, PreparedInput, Session, SolveConfig, WorldView,
    WorldViewLimits,
};
use zetesis_themelios::AdmittedFormula;

type Family = BTreeSet<(zetesis_core::Model, Option<Vec<(i32, i64)>>)>;

fn collect(owner: &AdmittedFormula, workers: usize, oracle: Oracle) -> WorldView {
    Session::builder(
        PreparedInput::formula(owner),
        SolveConfig {
            backend: Backend::Cpu,
            grounder: Grounder::Eager,
            models: 0,
            workers: NonZeroUsize::new(workers).unwrap(),
            oracle,
            ..SolveConfig::default()
        },
        Cancellation::default(),
    )
    .collect(WorldViewLimits::default())
    .unwrap()
}

fn family(view: &WorldView) -> Family {
    view.answer_sets()
        .iter()
        .map(|answer| {
            assert!(answer.subject().same_instance(view.subject()));
            (
                answer.interpretation().clone(),
                answer.score().map(|score| score.costs().to_vec()),
            )
        })
        .collect()
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
        let reference = collect(&owner, 1, Oracle::Countermodel);
        assert_eq!(
            reference.outcome().completion(),
            Some(Completion::Exhausted)
        );
        for workers in [1, 3] {
            let actual = collect(&owner, workers, Oracle::Auto);
            assert_eq!(actual.outcome().completion(), Some(Completion::Exhausted));
            let statistics = actual.outcome().countermodel_statistics().unwrap();
            assert!(matches!(
                statistics.certified.unwrap().plan,
                Some(CertificatePlanStatistics::Stratified(_))
            ));
            assert_eq!(statistics.countermodel_queries, 0);
            assert_eq!(statistics.search.decisions, 0);
            assert!(statistics.candidates <= 1);
            assert_eq!(
                family(&actual),
                family(&reference),
                "workers={workers}; {source}"
            );
        }
    }
}

#[test]
fn negative_recursion_keeps_the_existing_checker() {
    let owner = formula(include_str!("../fixtures/stratified/negative-cycle.lp"));
    let actual = collect(&owner, 3, Oracle::Auto);
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
