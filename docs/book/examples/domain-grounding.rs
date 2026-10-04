//! Optional final-join domains, exact theory equality and conservative fallback.

// ANCHOR: example
use std::cell::Cell;

use zetesis_domain::{Domain, Status};
use zetesis_themelios::ProgramSite;
use zetesis_themelios::{
    AdmissionOptions, AdmittedFormula, DomainLimits, DomainObservation, ExpansionLimits,
    FormulaFailure, FormulaLimits, GroundingObserver, GroundingOutcome, GroundingPhase,
    GroundingWork, prepare_formula,
};

const SOURCE: &str = "
    a(1). a(2). a(3). a(4). b(1). b(2). b(3). b(4).
    c(3,3). c(3,4). c(3,5). c(3,6).
    c(4,3). c(4,4). c(4,5). c(4,6).
    c(5,3). c(5,4). c(5,5). c(5,6).
    c(6,3). c(6,4). c(6,5). c(6,6).
    r(X,Y) :- a(X), b(Y), c(X,Y).
";

#[derive(Default)]
struct Observation {
    status: Cell<Option<Status>>,
    unknown_argument: Cell<bool>,
    work: Cell<GroundingWork>,
}
impl GroundingObserver for Observation {
    fn enter(&self) {}
    fn exit(&self) {}
    fn details_enabled(&self) -> bool {
        true
    }

    fn domain_analysis(&self, observation: DomainObservation<'_, '_>) {
        if let DomainObservation::Analyzed(analysis) = observation {
            // Inspect the actual borrowed result during the synchronous callback.
            // Retain only the owned status and information this example needs.
            self.status.set(Some(analysis.status()));
            self.unknown_argument.set(
                analysis
                    .arguments()
                    .any(|(_, _, argument)| matches!(argument.domain(), Domain::Unknown)),
            );
        }
    }

    fn phase_exit(
        &self,
        _: GroundingPhase,
        _: Option<ProgramSite>,
        _: GroundingOutcome,
        work: GroundingWork,
    ) {
        self.work.set(self.work.get().checked_sum(work));
    }
}

fn ground(
    domains: Option<DomainLimits>,
    observer: &Observation,
) -> Result<AdmittedFormula, FormulaFailure> {
    prepare_formula(
        SOURCE.to_owned(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )?
    .with_domain_analysis(domains) // None is the default; Indexed joins stay unchanged.
    .ground_with_observer(Some(observer))
}

fn same_theory(left: &AdmittedFormula, right: &AdmittedFormula) {
    assert_eq!(left.atoms(), right.atoms());
    assert_eq!(left.theory().nodes(), right.theory().nodes());
    assert_eq!(left.theory().roots(), right.theory().roots());
    assert_eq!(left.formula_origins(), right.formula_origins());
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let off = Observation::default();
    let ordinary = ground(None, &off)?;
    let on = Observation::default();
    let narrowed = ground(Some(DomainLimits::default()), &on)?;
    same_theory(&ordinary, &narrowed);
    assert_eq!(on.status.get(), Some(Status::FixedPoint));
    assert!(!on.unknown_argument.get());
    assert_eq!(
        narrowed
            .atoms()
            .iter()
            .filter(|atom| atom.predicate().name() == "r")
            .count(),
        4
    );
    let work = on.work.get();
    assert!(work.domain_prepare_work.is_some_and(|count| count > 0));
    assert!(work.domain_guard_checks.is_some_and(|count| count > 0));
    // Indexed offers one c posting, then matches whole rows. In the final
    // instantiation the guards reject two a rows, four b rows and eight c
    // rows before copying their new bindings; the completion round before it
    // rejected the same a and b rows, and the sum over the phases is twenty.
    assert_eq!(work.domain_rejected_rows, Some(20));
    assert!(work.join_probes.unwrap() < off.work.get().join_probes.unwrap());

    for limits in [
        DomainLimits {
            max_values_per_argument: 0,
            ..DomainLimits::default()
        },
        DomainLimits {
            max_work: 7,
            ..DomainLimits::default()
        },
    ] {
        let fallback = Observation::default();
        same_theory(&ordinary, &ground(Some(limits), &fallback)?);
        if limits.max_work == 7 {
            assert!(matches!(fallback.status.get(), Some(Status::Stopped(stop))
                if stop.resource == zetesis_domain::Resource::Work && stop.limit == 7));
        } else {
            // Here every argument widens. In general another finite argument
            // can still supply a guard when one argument becomes Unknown.
            assert_eq!(fallback.status.get(), Some(Status::FixedPoint));
            assert!(fallback.unknown_argument.get());
        }
        assert_eq!(fallback.work.get().domain_guard_rows, Some(0));
    }
    Ok(())
}
// ANCHOR_END: example

#[cfg(test)]
mod tests {
    #[test]
    fn actual_domain_activity_and_fallback_preserve_theory() {
        super::main().unwrap();
    }
}
