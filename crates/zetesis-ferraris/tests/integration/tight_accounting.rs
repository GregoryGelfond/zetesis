//! Every stopped prefix retains charged work for cumulative caller budgets.
use zetesis_cpu::{Cancellation, Stop};
use zetesis_ferraris::{
    Interpretation, TightCheckLimits, TightError, TightPlan, TightPlanLimits, TightResource,
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
