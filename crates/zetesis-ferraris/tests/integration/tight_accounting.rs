//! Every stopped prefix retains charged work for cumulative caller budgets.
use std::time::Instant;

use zetesis_cpu::{Cancellation, CancellationSlot, Stop};
use zetesis_ferraris::{
    AdmissionLimits, FormulaParts, Interpretation, Node, OperandSpan, Theory, TightCheckLimits,
    TightError, TightPlan, TightPlanLimits, TightResource, TightVerdict,
};
use zetesis_theory_support::theories::fact as theory;

#[test]
fn construction_and_candidate_prefixes_retain_exact_work_on_failure() {
    let t = theory();
    let c = Cancellation::default();
    let full = TightPlan::compile_accounted(&t, TightPlanLimits::default(), &c);
    let work = full.work;
    let plan = full.result.unwrap();
    assert_eq!(work, plan.statistics().work);
    for max_work in 0..=work {
        let attempt = TightPlan::compile_accounted(
            &t,
            TightPlanLimits {
                max_work,
                ..Default::default()
            },
            &c,
        );
        assert_eq!(attempt.work, max_work);
        if max_work == work {
            assert!(attempt.result.is_ok());
        } else {
            assert!(matches!(
                attempt.result,
                Err(TightError::Limit(TightResource::Work))
            ));
        }
    }
    let candidate = Interpretation::new(&t, [0]).unwrap();
    let work = plan
        .check_accounted(&candidate, TightCheckLimits::default(), &c)
        .work;
    for max_work in 0..=work {
        let attempt = plan.check_accounted(
            &candidate,
            TightCheckLimits {
                max_work,
                ..Default::default()
            },
            &c,
        );
        assert_eq!(attempt.work, max_work);
        if max_work == work {
            assert_eq!(attempt.result.unwrap().work, work);
        } else {
            assert!(matches!(
                attempt.result,
                Err(TightError::Limit(TightResource::Work))
            ));
        }
    }
    let foreign = Interpretation::new(&theory(), [0]).unwrap();
    let attempt = plan.check_accounted(&foreign, TightCheckLimits::default(), &c);
    assert_eq!(attempt.work, 0);
    assert_eq!(attempt.result, Err(TightError::Stopped(Stop::WrongProgram)));
    c.cancel();
    let attempt = TightPlan::compile_accounted(&t, TightPlanLimits::default(), &c);
    assert_eq!(attempt.work, 0);
    assert!(matches!(
        attempt.result,
        Err(TightError::Stopped(Stop::Cancelled))
    ));
    let attempt = plan.check_accounted(&candidate, TightCheckLimits::default(), &c);
    assert_eq!(attempt.work, 0);
    assert_eq!(attempt.result, Err(TightError::Stopped(Stop::Cancelled)));
}

fn mixed_theory() -> Theory {
    let nodes = vec![
        Node::atom(0),
        Node::atom(1),
        Node::atom(2),
        Node::falsum(),
        Node::implies(0, 3),
        Node::or_pair([0, 4]),
        Node::and_span(OperandSpan {
            start: 0,
            length: 4,
        }),
        Node::or_span(OperandSpan {
            start: 4,
            length: 3,
        }),
        Node::implies(7, 1),
        Node::implies(1, 2),
        Node::implies(2, 3),
    ];
    Theory::new(
        4,
        FormulaParts::new(nodes, vec![0, 0, 0, 0, 3, 6, 6]).unwrap(),
        vec![5, 8, 9, 8],
        AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn mixed_checks_retain_every_exact_work_prefix() {
    let theory = mixed_theory();
    let slot = CancellationSlot::default();
    let run = slot.open(None).unwrap();
    let cancellation = run.cancellation();
    let plan = TightPlan::compile(&theory, TightPlanLimits::default(), cancellation).unwrap();
    // Every node is evaluated, including the unused final implication. Wide
    // groups retain repeated operands; the root and producer scans retain the
    // repeated original root. A complete check charges 11 + 17 + 4 + 4 + 4.
    for (atoms, verdict, complete_work) in [
        (&[][..], TightVerdict::Stable, 40),
        (&[0][..], TightVerdict::NotModel { root: 8 }, 30),
        (&[0, 1][..], TightVerdict::NotModel { root: 9 }, 31),
        (&[0, 1, 2][..], TightVerdict::Stable, 40),
        (
            &[0, 1, 2, 3][..],
            TightVerdict::Residual {
                unsupported_atom: 3,
            },
            40,
        ),
    ] {
        let candidate = Interpretation::new(&theory, atoms.iter().copied()).unwrap();
        for max_work in (0..=complete_work).chain([u64::MAX]) {
            let attempt = plan.check_accounted(
                &candidate,
                TightCheckLimits {
                    max_work,
                    ..Default::default()
                },
                cancellation,
            );
            assert_eq!(attempt.work, max_work.min(complete_work));
            if max_work < complete_work {
                assert_eq!(attempt.result, Err(TightError::Limit(TightResource::Work)));
            } else {
                let check = attempt.result.unwrap();
                assert_eq!(check.work, complete_work);
                assert_eq!(check.verdict, verdict);
            }
        }
    }
}

#[test]
fn cancellation_precedes_tight_resource_refusal() {
    let theory = mixed_theory();
    let plan = TightPlan::compile(
        &theory,
        TightPlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let candidate = Interpretation::new(&theory, [0, 1, 2]).unwrap();
    let slot = CancellationSlot::default();
    let run = slot.open(Some(Instant::now())).unwrap();
    let cancellation = run.cancellation();
    let limits = TightCheckLimits {
        max_bytes: 0,
        max_work: 0,
    };
    for expected in [Stop::Deadline, Stop::Cancelled] {
        if expected == Stop::Cancelled {
            slot.cancel();
        }
        let attempt = plan.check_accounted(&candidate, limits, cancellation);
        assert_eq!(attempt.work, 0);
        assert_eq!(attempt.result, Err(TightError::Stopped(expected)));
    }
}

#[test]
fn foreign_identity_precedes_tight_cancellation() {
    let theory = mixed_theory();
    let plan = TightPlan::compile(
        &theory,
        TightPlanLimits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let foreign = Interpretation::new(&mixed_theory(), [0, 1, 2]).unwrap();
    let cancellation = Cancellation::with_deadline(Instant::now()).unwrap();
    cancellation.cancel();
    let attempt = plan.check_accounted(&foreign, TightCheckLimits::default(), &cancellation);
    assert_eq!(attempt.work, 0);
    assert_eq!(attempt.result, Err(TightError::Stopped(Stop::WrongProgram)));
}
