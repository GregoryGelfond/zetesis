//! Diagnostic requests leave the shared finite-domain consumer unchanged.

use super::observation::Observation;
use zetesis_reference_support::prepare;
use zetesis_themelios::{
    DomainLimits, DomainObservation, GroundingObserver, GroundingOutcome, GroundingPhase,
    GroundingWork, ProgramSite,
};

#[derive(Default)]
struct FiniteOnly(Observation);
impl GroundingObserver for FiniteOnly {
    fn enter(&self) {}
    fn exit(&self) {}
    fn details_enabled(&self) -> bool {
        true
    }
    fn domain_diagnostics_enabled(&self) -> bool {
        false
    }
    fn domain_analysis(&self, _: DomainObservation<'_, '_>) {
        panic!("this observer requested counters, not numeric diagnostics");
    }
    fn phase_exit(
        &self,
        phase: GroundingPhase,
        site: Option<ProgramSite>,
        outcome: GroundingOutcome,
        work: GroundingWork,
    ) {
        self.0.phase_exit(phase, site, outcome, work);
    }
}

#[test]
fn declining_diagnostics_preserves_finite_guard_execution() {
    let source = include_str!("../../fixtures/numeric-domains/finite-guard.lp");
    let finite = FiniteOnly::default();
    let reported = Observation::default();
    let ground = |observer: &dyn GroundingObserver| {
        prepare(source)
            .unwrap()
            .with_domain_analysis(Some(DomainLimits::default()))
            .ground_with_observer(Some(observer))
            .unwrap()
    };
    let plain = ground(&finite);
    let diagnostic = ground(&reported);
    super::equal(&plain, &diagnostic);
    reported.completed();
    let plain_work = finite.0.work.get();
    let diagnostic_work = reported.work.get();
    assert!(plain_work.domain_excluded_values.unwrap() > 0);
    assert_eq!(
        plain_work.domain_guard_rows,
        diagnostic_work.domain_guard_rows
    );
    assert_eq!(
        plain_work.domain_guard_checks,
        diagnostic_work.domain_guard_checks
    );
    assert_eq!(
        plain_work.domain_rejected_rows,
        diagnostic_work.domain_rejected_rows
    );
    assert_eq!(
        plain_work.domain_excluded_values,
        diagnostic_work.domain_excluded_values
    );
    assert!(plain_work.domain_prepare_work.unwrap() < diagnostic_work.domain_prepare_work.unwrap());
}
