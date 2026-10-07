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
    let theory = Theory::new(
        3,
        zetesis_ferraris::FormulaParts::new(vec![Node::atom(0)], vec![]).unwrap(),
        vec![0],
        FormulaLimits::default(),
    )
    .unwrap();
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
    let foreign = Theory::new(
        3,
        zetesis_ferraris::FormulaParts::new(vec![Node::atom(0)], vec![]).unwrap(),
        vec![0],
        FormulaLimits::default(),
    )
    .unwrap();
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

/// The fact a and (a and ... and a) -> b, with one native body node.
fn certificate_theory(width: usize) -> Theory {
    let (body, operands) = if width == 2 {
        (Node::and_pair([0, 0]), vec![])
    } else {
        (
            Node::and_span(zetesis_ferraris::OperandSpan {
                start: 0,
                length: width,
            }),
            vec![0; width],
        )
    };
    Theory::new(
        2,
        zetesis_ferraris::FormulaParts::new(
            vec![Node::atom(0), Node::atom(1), body, Node::implies(2, 1)],
            operands,
        )
        .unwrap(),
        vec![0, 3],
        FormulaLimits::default(),
    )
    .unwrap()
}

#[test]
fn complete_certificate_allowances_include_every_operand() {
    use std::sync::Arc;
    use zetesis_ferraris::{Interpretation, TightPlan, TightPlanLimits};
    let cancellation = Cancellation::default();
    for width in [2, 3, 65, 129] {
        let theory = certificate_theory(width);
        let candidate = Interpretation::new(&theory, [0, 1]).unwrap();
        let tight = TightPlan::compile(&theory, TightPlanLimits::default(), &cancellation).unwrap();
        let positive =
            PositivePlan::compile(&theory, PositivePlanLimits::default(), &cancellation).unwrap();
        // A=2, N=4, E=width+2 and R=2; tight checking also reads P=2.
        for (certificate, expected) in [
            (
                super::Certification::Tight {
                    plan: Arc::new(tight),
                    max_bytes: u64::MAX,
                },
                width + 12,
            ),
            (
                super::Certification::Positive {
                    plan: positive,
                    max_bytes: usize::MAX,
                },
                width + 10,
            ),
        ] {
            let expected = u64::try_from(expected).unwrap();
            assert_eq!(certificate.checking_work_bound().unwrap(), expected);
            for max_work in [expected - 1, expected] {
                let mut statistics = crate::Statistics {
                    certified: Some(super::CertifiedStatistics::default()),
                    ..Default::default()
                };
                let mut search = SearchStatistics::default();
                let result = super::classify(
                    &certificate,
                    &candidate,
                    crate::Limits {
                        search: SearchLimits {
                            max_work,
                            ..SearchLimits::default()
                        },
                        ..crate::Limits::default()
                    },
                    &cancellation,
                    &mut statistics,
                    &mut search,
                );
                assert_eq!(search.work, max_work);
                assert_eq!(statistics.certified.unwrap().checking_work, max_work);
                if max_work == expected {
                    assert!(matches!(result, Ok(super::Verdict::Stable)));
                } else {
                    assert!(matches!(result, Err(Incomplete::WorkLimit)));
                }
            }
        }
    }
}
