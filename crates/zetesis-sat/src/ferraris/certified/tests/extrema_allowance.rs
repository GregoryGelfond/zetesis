//! Certificate reservations cover the complete fresh ordinal workspace.

use std::sync::Arc;

use super::super::{Certification, CertifiedStatistics, Verdict, Workspace, classify};
use crate::{Cancellation, Incomplete, SearchLimits, SearchStatistics};
use zetesis_ferraris::{
    AdmissionLimits, ExtremumCutDescriptor, ExtremumCutRule, FormulaParts, Interpretation, Node,
    NumericDomain, NumericRow, PositivePlan, PositivePlanLimits, StratifiedPlan,
    StratifiedPlanLimits, Theory, TightPlan, TightPlanLimits,
};

fn theory(rows: usize, empty: bool) -> Theory {
    let mut numeric_rows = (0..rows)
        .map(|index| NumericRow {
            start: 0,
            length: 0,
            domain: if index % 2 == 0 {
                NumericDomain::Weights
            } else {
                NumericDomain::Ordinal { rank_count: 3 }
            },
        })
        .collect::<Vec<_>>();
    numeric_rows[rows - 1] = NumericRow {
        start: 0,
        length: usize::from(!empty),
        domain: NumericDomain::Ordinal { rank_count: 3 },
    };
    let parts = FormulaParts::from_numeric_raw(
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::extremum_cut(0),
            Node::implies(2, 1),
        ],
        vec![],
        if empty { vec![] } else { vec![0] },
        if empty { vec![] } else { vec![1] },
        numeric_rows,
        vec![],
        vec![ExtremumCutDescriptor {
            row: rows - 1,
            // The empty-row case exercises the greatest empty sentinel.
            rule: ExtremumCutRule::Prefix(if empty { 3 } else { 2 }),
        }],
    )
    .unwrap();
    Theory::new(2, parts, vec![0, 3], AdmissionLimits::default()).unwrap()
}

fn certificates(theory: &Theory) -> [Certification; 3] {
    let control = Cancellation::default();
    [
        Certification::Tight {
            plan: Arc::new(
                TightPlan::compile(theory, TightPlanLimits::default(), &control).unwrap(),
            ),
            max_bytes: u64::MAX,
        },
        Certification::Positive {
            plan: PositivePlan::compile(theory, PositivePlanLimits::default(), &control).unwrap(),
            max_bytes: usize::MAX,
        },
        Certification::Stratified {
            plan: StratifiedPlan::compile(theory, StratifiedPlanLimits::default(), &control)
                .unwrap(),
            max_bytes: usize::MAX,
        },
    ]
}

fn classify_with_allowance(
    certificate: &Certification,
    workspace: &mut Workspace,
    candidate: &Interpretation,
    allowance: u64,
) -> (Result<Verdict, Incomplete>, u64) {
    let mut statistics = crate::Statistics {
        certified: Some(CertifiedStatistics::default()),
        ..Default::default()
    };
    let mut search = SearchStatistics::default();
    let result = classify(
        certificate,
        workspace,
        candidate,
        crate::Limits {
            search: SearchLimits {
                max_work: allowance,
                ..SearchLimits::default()
            },
            ..crate::Limits::default()
        },
        &Cancellation::default(),
        &mut statistics,
        &mut search,
    );
    assert_eq!(statistics.certified.unwrap().checking_work, search.work);
    (result, search.work)
}

#[test]
fn reserved_allowances_complete_fresh_extrema_checks() {
    for rows in [1_usize, 2, 64, 65, 130] {
        for empty in [false, true] {
            let theory = theory(rows, empty);
            let candidate = Interpretation::new(&theory, [0, 1]).unwrap();
            for certificate in certificates(&theory) {
                let bound = certificate.checking_work_bound().unwrap();
                let (result, work) = classify_with_allowance(
                    &certificate,
                    &mut Workspace::default(),
                    &candidate,
                    bound,
                );
                // This assertion fails under the old reservation, which omitted
                // every initialized row cell and validity-word reset.
                assert!(matches!(result, Ok(Verdict::Stable)));
                assert_eq!(work, bound);
            }
        }
    }
}

#[test]
fn fresh_extrema_checks_refuse_one_unit_short() {
    for certificate in certificates(&theory(65, false)) {
        let owner = match &certificate {
            Certification::Tight { plan, .. } => plan.theory(),
            Certification::Positive { plan, .. } => plan.theory(),
            Certification::Stratified { plan, .. } => plan.theory(),
        };
        let candidate = Interpretation::new(owner, [0, 1]).unwrap();
        let allowance = certificate.checking_work_bound().unwrap() - 1;
        let (result, work) = classify_with_allowance(
            &certificate,
            &mut Workspace::default(),
            &candidate,
            allowance,
        );
        assert!(matches!(result, Err(Incomplete::WorkLimit)));
        assert_eq!(work, allowance);
    }
}

#[test]
fn reused_checks_fit_the_fresh_reservation() {
    let rows = 65;
    let theory = theory(rows, false);
    let candidate = Interpretation::new(&theory, [0, 1]).unwrap();
    for certificate in certificates(&theory) {
        let bound = certificate.checking_work_bound().unwrap();
        let mut workspace = Workspace::default();
        for pass in 0..2 {
            let (result, work) =
                classify_with_allowance(&certificate, &mut workspace, &candidate, bound);
            assert!(matches!(result, Ok(Verdict::Stable)));
            let initialized = if pass == 1 && matches!(&certificate, Certification::Tight { .. }) {
                u64::try_from(rows).unwrap()
            } else {
                0
            };
            // Determined validation owns a fresh EvaluationWorkspace each time.
            assert_eq!(work, bound - initialized);
        }
    }
}
