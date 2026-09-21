//! An explicit caller-supplied partition experiment; no source recognizer.

use std::collections::{BTreeMap, BTreeSet};

use zetesis_ferraris::{Theory, TightPlanLimits};
use zetesis_sat::{
    Cancellation, Limits, StableModels, Statistics,
    partition::{Group, Plan, Premises, Restriction, RestrictionLimits},
};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

const SOURCES: [&str; 6] = [
    include_str!("../../../examples/kr-domains/standalone/n-queens/variant-01.lp"),
    include_str!("../../../examples/kr-domains/standalone/n-queens/variant-02.lp"),
    include_str!("../../../examples/kr-domains/standalone/n-queens/variant-03.lp"),
    include_str!("../../../examples/kr-domains/standalone/n-queens/variant-04.lp"),
    include_str!("../../../examples/kr-domains/standalone/n-queens/variant-05.lp"),
    include_str!("../../../examples/kr-domains/standalone/n-queens/variant-06.lp"),
];

fn enumerate(theory: &Theory, restrictions: &[Restriction]) -> (BTreeSet<Vec<usize>>, Statistics) {
    let mut search = StableModels::new(theory, Limits::default(), Cancellation::default()).unwrap();
    for restriction in restrictions {
        search.restrict_candidates(restriction.theory()).unwrap();
    }
    assert!(
        search
            .enable_certified_checking(TightPlanLimits::default())
            .unwrap()
    );
    let mut models = BTreeSet::new();
    for model in search.by_ref() {
        let model = model.unwrap();
        assert!(model.theory().same_instance(theory));
        assert!(models.insert(model.atoms().collect()));
    }
    assert!(search.exhausted());
    let statistics = search.statistics();
    assert_eq!(statistics.countermodel_queries, 0);
    assert_eq!(statistics.candidates, 92);
    assert_eq!(statistics.stable_models, 92);
    assert_eq!(models.len(), 92);
    (models, statistics)
}

#[test]
#[ignore = "manual descriptor work experiment; no clock, automatic recognition or RSS measurement"]
fn report_queens_partition_work() {
    for (index, source) in SOURCES.into_iter().enumerate() {
        let admitted = admit_formula(
            source.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )
        .unwrap();
        let theory = admitted.theory();
        let members: Vec<_> = admitted
            .atoms()
            .iter()
            .enumerate()
            .filter(|(_, atom)| atom.predicate().name() == "queen_at")
            .map(|(index, atom)| {
                assert_eq!(atom.values().len(), 2);
                index
            })
            .collect();
        assert_eq!(members.len(), 64);
        let (baseline, before) = enumerate(theory, &[]);
        let mut plans = Vec::new();
        let mut restrictions = Vec::new();
        for coordinate in 0..2 {
            let mut rows = BTreeMap::<_, Vec<usize>>::new();
            for &atom in &members {
                rows.entry(admitted.atoms()[atom].values()[coordinate].clone())
                    .or_default()
                    .push(atom);
            }
            assert_eq!(rows.len(), 8);
            let groups: Vec<_> = rows
                .values()
                .map(|members| Group { members, upper: 1 })
                .collect();
            // All original classical candidates were exhausted above and each
            // received an exact tight certificate. Check the supplied premises
            // on that complete family before installing any restriction.
            for candidate in &baseline {
                assert_eq!(
                    members
                        .iter()
                        .filter(|atom| candidate.contains(atom))
                        .count(),
                    8
                );
                assert!(groups.iter().all(|group| {
                    group
                        .members
                        .iter()
                        .filter(|atom| candidate.contains(atom))
                        .count()
                        <= group.upper
                }));
            }
            let plan = Plan::new(
                Premises {
                    atom_count: theory.atom_count(),
                    members: &members,
                    lower: 8,
                    groups: &groups,
                },
                zetesis_sat::partition::Limits::default(),
                &Cancellation::default(),
            )
            .unwrap();
            restrictions.push(
                plan.restriction(RestrictionLimits::default(), &Cancellation::default())
                    .unwrap(),
            );
            plans.push(plan);
        }
        let (actual, after) = enumerate(theory, &restrictions);
        assert_eq!(actual, baseline);
        report(index + 1, &before, &after, &plans, &restrictions);
    }
}

fn report(
    variant: usize,
    before: &Statistics,
    after: &Statistics,
    plans: &[Plan],
    restrictions: &[Restriction],
) {
    let plan_work: u64 = plans.iter().map(|plan| plan.statistics().work).sum();
    let plan_resident: u64 = plans
        .iter()
        .map(|plan| plan.statistics().resident_bytes)
        .sum();
    let emission_work: u64 = restrictions.iter().map(Restriction::work).sum();
    let nodes: usize = restrictions
        .iter()
        .map(|restriction| restriction.theory().nodes().len())
        .sum();
    let roots: usize = restrictions
        .iter()
        .map(|restriction| restriction.theory().roots().len())
        .sum();
    println!(
        "PARTITION_WORK variant={variant:02} baseline_work={} planned_search_work={} planning_work={plan_work} emission_work={emission_work} baseline_decisions={} planned_decisions={} plan_resident_bytes={plan_resident} emitted_nodes={nodes} emitted_roots={roots} original_candidates={} planned_candidates={} full_models_equal=true",
        before.search.work,
        after.search.work,
        before.search.decisions,
        after.search.decisions,
        before.candidates,
        after.candidates
    );
}
