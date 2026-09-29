//! Exact least-model units are one transaction over the candidate CNF owner.

use crate::search::{Budget, LocalQuota};
use crate::{
    AdmissionError, AdmissionLimits, Cancellation, Cnf, Incomplete, Literal, Resource,
    SearchLimits, SearchStatistics, Solve,
};
use zetesis_ferraris::{
    AdmissionLimits as FormulaLimits, Node, PositivePlan, PositivePlanLimits, Theory,
};

fn plan() -> PositivePlan {
    let theory = Theory::new(3, vec![Node::Atom(0)], vec![0], FormulaLimits::default()).unwrap();
    PositivePlan::compile(
        &theory,
        PositivePlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap()
}

fn original(limits: AdmissionLimits) -> Cnf {
    Cnf::new(3, vec![vec![Literal::new(0, true)]], limits).unwrap()
}

fn contents(cnf: &Cnf) -> Vec<Vec<Literal>> {
    cnf.clauses()
        .map(|clause| clause.iter().collect())
        .collect()
}

#[test]
fn unit_admission_refusal_preserves_the_original_cnf() {
    let plan = plan();
    let cancellation = Cancellation::default();
    let mut cnf = original(AdmissionLimits {
        max_clauses: 2,
        ..Default::default()
    });
    let before = contents(&cnf);
    let mut budget = Budget {
        quota: LocalQuota,
        limits: SearchLimits::default(),
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    assert_eq!(
        super::positive::restrict(&plan, &mut cnf, &mut budget),
        Err(Incomplete::Admission(AdmissionError::Limit {
            resource: Resource::Clauses,
            observed: 3,
            limit: 2,
        }))
    );
    assert_eq!(contents(&cnf), before);
    assert_eq!(
        budget.statistics.work, 4,
        "two identity reads and two attempted unit writes"
    );
    assert_eq!(cnf.variables(), 3);
}

#[test]
fn stopped_unit_construction_rolls_back_every_partial_prefix() {
    let plan = plan();
    let cancellation = Cancellation::default();
    let required = 2 * plan.theory().atom_count() as u64;
    for limit in 0..required {
        let mut cnf = original(AdmissionLimits::default());
        let before = contents(&cnf);
        let mut budget = Budget {
            quota: LocalQuota,
            limits: SearchLimits {
                max_work: limit,
                ..Default::default()
            },
            cancellation: &cancellation,
            statistics: SearchStatistics::default(),
        };
        assert_eq!(
            super::positive::restrict(&plan, &mut cnf, &mut budget),
            Err(Incomplete::WorkLimit)
        );
        assert_eq!(budget.statistics.work, limit);
        assert_eq!(contents(&cnf), before);
        budget.limits.max_work = limit + required;
        assert_eq!(
            super::positive::restrict(&plan, &mut cnf, &mut budget),
            Ok(3)
        );
        assert_eq!(budget.statistics.work, limit + required);
    }
}

#[test]
fn committed_units_select_exactly_the_least_interpretation() {
    let plan = plan();
    let cancellation = Cancellation::default();
    let mut cnf = original(AdmissionLimits::default());
    let mut budget = Budget {
        quota: LocalQuota,
        limits: SearchLimits::default(),
        cancellation: &cancellation,
        statistics: SearchStatistics::default(),
    };
    assert_eq!(
        super::positive::restrict(&plan, &mut cnf, &mut budget),
        Ok(3)
    );
    assert_eq!(cnf.variables(), 3, "no auxiliary identity was added");
    assert_eq!(
        contents(&cnf),
        vec![
            vec![Literal::new(0, true)],
            vec![Literal::new(0, true)],
            vec![Literal::new(1, false)],
            vec![Literal::new(2, false)]
        ]
    );
    let Solve::Sat(assignment) = crate::solve(&cnf, SearchLimits::default(), &cancellation) else {
        panic!("least model must satisfy its units");
    };
    assert_eq!(
        (0..3)
            .filter(|atom| assignment.value(*atom).unwrap())
            .collect::<Vec<_>>(),
        vec![0]
    );
}

#[test]
fn positive_membership_rejects_equal_looking_foreign_owners() {
    let plan = plan();
    let foreign = Theory::new(3, vec![Node::Atom(0)], vec![0], FormulaLimits::default()).unwrap();
    let candidate = zetesis_ferraris::Interpretation::new(&foreign, [0]).unwrap();
    let mut search = SearchStatistics::default();
    let (result, _) = super::positive::check(
        &plan,
        &candidate,
        usize::MAX,
        crate::Limits::default(),
        &Cancellation::default(),
        &mut search,
    );
    assert!(matches!(result, Err(Incomplete::WrongTheory)));
    assert_eq!(search.work, 0);
}

#[test]
fn positive_membership_refutes_a_nonleast_model() {
    let plan = plan();
    let candidate = zetesis_ferraris::Interpretation::new(plan.theory(), [0, 2]).unwrap();
    let mut search = SearchStatistics::default();
    let (result, _) = super::positive::check(
        &plan,
        &candidate,
        usize::MAX,
        crate::Limits::default(),
        &Cancellation::default(),
        &mut search,
    );
    assert!(matches!(result, Ok(super::Verdict::NonMinimal)));
    assert_eq!(
        search.work, 5,
        "atom comparison and original satisfaction both complete"
    );
}

#[test]
fn positive_membership_identifies_a_nonmodel() {
    let plan = plan();
    let candidate = zetesis_ferraris::Interpretation::new(plan.theory(), []).unwrap();
    let mut search = SearchStatistics::default();
    let (result, _) = super::positive::check(
        &plan,
        &candidate,
        usize::MAX,
        crate::Limits::default(),
        &Cancellation::default(),
        &mut search,
    );
    assert!(matches!(result, Ok(super::Verdict::NotModel)));
    assert_eq!(search.work, 5);
}
