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

const STATIC_TOTALITY: &str =
    include_str!("../../tests/fixtures/numeric-domains/static-totality.lp");

fn total_bodies(source: &str) -> usize {
    total_bodies_with_limits(source, DomainLimits::default())
}

fn total_bodies_with_limits(source: &str, domains: DomainLimits) -> usize {
    let preparation = testing::prepare(source);
    let limits = preparation.limits;
    let location = preparation.location;
    let mut counters = Counters::resume(preparation.accounting, Work::default());
    let mut catalog = preparation.catalog;
    let (relations, mut append) = catalog.split(&limits, &mut counters, location).unwrap();
    let support = Support::indexed(&relations, &limits, &counters, location).unwrap();
    let mut computation = Computation::new(&mut append, &support);
    let domains = crate::formula_domains::analyze(
        &preparation.program,
        Some(domains),
        &Profile::new(None),
        Context::new(&mut computation, &limits, &mut counters, location),
    )
    .unwrap();
    preparation
        .program
        .rules_without_facts()
        .iter()
        .enumerate()
        .filter(|(index, rule)| {
            domains
                .as_ref()
                .is_some_and(|domains| domains.total_body(*index, rule).unwrap().is_some())
        })
        .count()
}

#[test]
fn static_totality_is_available_without_diagnostics() {
    assert_eq!(total_bodies(STATIC_TOTALITY), 1);
}

#[test]
fn choice_producers_prevent_an_unsound_nonzero_certificate() {
    assert_eq!(
        total_bodies(include_str!(
            "../../tests/fixtures/numeric-domains/static-totality-choice-zero.lp"
        )),
        0
    );
}

#[test]
fn disjunctive_producers_prevent_an_unsound_nonzero_certificate() {
    assert_eq!(
        total_bodies(include_str!(
            "../../tests/fixtures/numeric-domains/static-totality-disjunction-zero.lp"
        )),
        0
    );
}

#[test]
fn mixed_argument_values_decline_static_totality() {
    assert_eq!(
        total_bodies(include_str!(
            "../../tests/fixtures/numeric-domains/static-totality-mixed.lp"
        )),
        0
    );
}

#[test]
fn an_independent_overflow_declines_static_totality() {
    assert_eq!(
        total_bodies(include_str!(
            "../../tests/fixtures/numeric-domains/static-totality-overflow.lp"
        )),
        0
    );
}

#[test]
fn exceptional_signed_remainder_declines_static_totality() {
    assert_eq!(
        total_bodies(include_str!(
            "../../tests/fixtures/numeric-domains/static-totality-minimum.lp"
        )),
        0
    );
}

#[test]
fn an_empty_binding_scope_grants_no_static_totality() {
    assert_eq!(
        total_bodies(include_str!(
            "../../tests/fixtures/numeric-domains/static-totality-empty.lp"
        )),
        0
    );
}

#[test]
fn stopped_analysis_grants_no_static_totality() {
    assert_eq!(
        total_bodies_with_limits(
            STATIC_TOTALITY,
            DomainLimits {
                max_work: 1,
                ..DomainLimits::default()
            }
        ),
        0
    );
}

#[test]
fn complete_binding_totality_does_not_type_every_prefix() {
    assert_eq!(
        total_bodies(include_str!(
            "../../tests/fixtures/numeric-domains/static-totality-prefix.lp"
        )),
        1
    );
}

#[test]
fn static_selection_keeps_its_exact_source_authority() {
    let preparation = testing::prepare(STATIC_TOTALITY);
    let foreign = testing::prepare(STATIC_TOTALITY);
    let limits = preparation.limits;
    let location = preparation.location;
    let mut counters = Counters::resume(preparation.accounting, Work::default());
    let mut catalog = preparation.catalog;
    let (relations, mut append) = catalog.split(&limits, &mut counters, location).unwrap();
    let support = Support::indexed(&relations, &limits, &counters, location).unwrap();
    let mut computation = Computation::new(&mut append, &support);
    let domains = crate::formula_domains::analyze(
        &preparation.program,
        Some(DomainLimits::default()),
        &Profile::new(None),
        Context::new(&mut computation, &limits, &mut counters, location),
    )
    .unwrap()
    .unwrap();
    let (index, rule) = preparation
        .program
        .rules_without_facts()
        .iter()
        .enumerate()
        .find(|(index, rule)| domains.total_body(*index, rule).unwrap().is_some())
        .unwrap();
    let totality = domains.total_body(index, rule).unwrap().unwrap();
    assert!(
        domains
            .total_body(index, &foreign.program.rules_without_facts()[index])
            .is_err()
    );
    let mut join = super::Join::variant_rule(
        rule,
        super::delta::Variant::Full,
        &support,
        None,
        &mut testing::budget(),
        Context::new(&computation, &limits, &mut counters, rule.location),
    )
    .unwrap();
    assert!(join.coverage == super::Coverage::Complete);
    join.select_static_total_body(rule, totality, &limits, &mut counters)
        .unwrap();
    assert!(join.coverage == super::Coverage::Selected);
    assert!(
        !join.certified_total,
        "static totality grants no computed-key authority"
    );
    join.evidence();
    assert!(join.coverage == super::Coverage::Complete);
    assert!(
        join.select_static_total_body(rule, totality, &limits, &mut counters)
            .is_err()
    );
}

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
