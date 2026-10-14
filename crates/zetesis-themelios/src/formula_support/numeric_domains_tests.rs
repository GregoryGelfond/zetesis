//! The real domain phase uses compact facts and leaves full source views lazy.

use super::{Computation, Context, Counters, Support, testing};
use crate::{DomainLimits, DomainObservation, GroundingObserver, NumericCensus};
use std::cell::Cell;

#[derive(Default)]
struct Observer(Cell<Option<NumericCensus>>);
impl GroundingObserver for Observer {
    fn enter(&self) {}
    fn exit(&self) {}
    fn domain_analysis(&self, observation: DomainObservation<'_, '_>) {
        let DomainObservation::Analyzed(view) = observation else {
            panic!("requested numeric analysis");
        };
        self.0.set(view.census());
        assert_eq!(view.rules().len(), view.census().unwrap().rule_occurrences);
    }
}
use crate::grounding_observer::{Profile, Work};

fn leaves_full_program_lazy(source: &str, guards_expected: bool) {
    let preparation = testing::prepare(source);
    assert!(!preparation.program.analyzed.is_materialized());
    let limits = preparation.limits;
    let location = preparation.location;
    let mut counters = Counters::resume(preparation.accounting, Work::default());
    let mut catalog = preparation.catalog;
    let (relations, mut append) = catalog.split(&limits, &mut counters, location).unwrap();
    let support = Support::indexed(&relations, &limits, &counters, location).unwrap();
    let mut computation = Computation::new(&mut append, &support);
    let observer = Observer::default();
    let domains = crate::formula_domains::analyze(
        &preparation.program,
        Some(DomainLimits::default()),
        &Profile::new(Some(&observer)),
        Context::new(&mut computation, &limits, &mut counters, location),
    )
    .unwrap();
    assert_eq!(domains.is_some(), guards_expected);
    assert!(observer.0.get().is_some());
    assert!(
        !preparation.program.analyzed.is_materialized(),
        "numeric lowering and finite-guard certification must not reconstruct facts as a Program"
    );
}

#[test]
fn finite_domain_phase_keeps_the_full_program_lazy() {
    leaves_full_program_lazy(
        include_str!("../../tests/fixtures/numeric-domains/finite-guard.lp"),
        true,
    );
}

#[test]
fn broader_numeric_analysis_keeps_the_full_program_lazy() {
    leaves_full_program_lazy(
        include_str!("../../tests/fixtures/numeric-domains/all-producers.lp"),
        false,
    );
}

#[test]
fn unrequested_certificates_leave_shared_bounds_unchanged() {
    let preparation = testing::prepare(include_str!(
        "../../tests/fixtures/numeric-domains/finite-guard.lp"
    ));
    let limits = preparation.limits;
    let location = preparation.location;
    let mut counters = Counters::resume(preparation.accounting, Work::default());
    let mut catalog = preparation.catalog;
    let (relations, mut append) = catalog.split(&limits, &mut counters, location).unwrap();
    let support = Support::indexed(&relations, &limits, &counters, location).unwrap();
    let computation = Computation::new(&mut append, &support);
    let components = computation.static_components(location).unwrap();
    let analyze = |diagnostics| {
        crate::formula_domains::numeric_ir::analyze(
            &preparation.program,
            components,
            computation.read(),
            DomainLimits::default(),
            diagnostics,
        )
        .unwrap_or_else(|failure| panic!("numeric analysis failed: {:?}", failure.error))
    };
    let plain = analyze(false);
    let requested = analyze(true);
    assert_eq!(
        plain.values.status(),
        zetesis_domain::numeric::Status::FixedPoint
    );
    assert_eq!(plain.values.status(), requested.values.status());
    assert_eq!(
        plain.values.arguments().collect::<Vec<_>>(),
        requested.values.arguments().collect::<Vec<_>>()
    );
    for (argument, _) in plain.values.arguments() {
        let values = |analysis: &crate::formula_domains::numeric_ir::Analysis<'_>| {
            analysis.values.values(argument).map(|values| {
                values
                    .map(|key| computation.read().term(key).unwrap().to_string())
                    .collect::<Vec<_>>()
            })
        };
        assert_eq!(values(&plain), values(&requested));
        assert_eq!(
            plain.values.producers(argument),
            requested.values.producers(argument)
        );
    }
    let plain_work = plain.values.statistics();
    let requested_work = requested.values.statistics();
    // The one comparison has two requests and two certificate-result cells.
    assert_eq!(requested_work.links, plain_work.links + 4);
    assert!(requested_work.work > plain_work.work);
    assert_eq!(plain.view(computation.read()).rules().len(), 0);
    assert_eq!(requested.view(computation.read()).rules().len(), 1);
}

struct Declined;
impl GroundingObserver for Declined {
    fn enter(&self) {}
    fn exit(&self) {}
    fn domain_diagnostics_enabled(&self) -> bool {
        false
    }
    fn domain_analysis(&self, _: DomainObservation<'_, '_>) {
        panic!("declined diagnostics must not call the observer");
    }
}

#[test]
fn absent_and_declining_observers_have_the_same_domain_work() {
    let preparation = testing::prepare(include_str!(
        "../../tests/fixtures/numeric-domains/finite-guard.lp"
    ));
    let limits = preparation.limits;
    let location = preparation.location;
    let mut counters = Counters::resume(preparation.accounting, Work::default());
    let mut catalog = preparation.catalog;
    let (relations, mut append) = catalog.split(&limits, &mut counters, location).unwrap();
    let support = Support::indexed(&relations, &limits, &counters, location).unwrap();
    let mut computation = Computation::new(&mut append, &support);
    let requested = Observer::default();
    let profiles = [
        Profile::new(None),
        Profile::new(Some(&Declined)),
        Profile::new(Some(&requested)),
    ];
    let mut work = Vec::new();
    // Every attempt borrows the same source and canonical owner; only its
    // diagnostic request changes. No local-plan address ordering is involved.
    for profile in profiles {
        let before = counters.accounting.work;
        let domains = crate::formula_domains::analyze(
            &preparation.program,
            Some(DomainLimits::default()),
            &profile,
            Context::new(&mut computation, &limits, &mut counters, location),
        )
        .unwrap()
        .expect("finite-domain guards remain prepared");
        for (index, rule) in preparation.program.rules_without_facts().iter().enumerate() {
            assert!(domains.for_rule(index, rule).is_ok());
        }
        work.push(counters.accounting.work - before);
    }
    assert_eq!(work[0], work[1]);
    assert!(work[2] > work[0]);
    assert_eq!(requested.0.get().unwrap().integer_queries, 2);
}
