//! Determined-interpretation membership and complete certificate allowances.

mod extrema_allowance;

use crate::{Cancellation, Incomplete, SearchLimits, SearchStatistics};
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

fn determined(plan: &PositivePlan) -> super::determined::Determined<'_> {
    super::determined::Determined {
        interpretation: plan.least_consequences(),
        failed_constraint: plan.failed_constraint(),
        retained_bytes: plan.statistics().retained_bytes,
    }
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
    let (result, _) = super::determined::check(
        determined(&plan),
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
    let (result, _) = super::determined::check(
        determined(&plan),
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
    let (result, _) = super::determined::check(
        determined(&plan),
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
        // A=2, N=4, E=width+2 and R=2. Tight checking reads P=2 and
        // one candidate word; the positive check compares both atoms.
        for (certificate, expected) in [
            (
                super::Certification::Tight {
                    plan: Arc::new(tight),
                    max_bytes: u64::MAX,
                },
                width + 11,
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
                    &mut super::Workspace::default(),
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
