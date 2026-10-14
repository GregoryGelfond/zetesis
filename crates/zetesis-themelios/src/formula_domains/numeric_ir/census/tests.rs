//! Source correspondence is by actual IR borrow, never equal sites or ordinals.

use crate::formula_support::{Computation, Counters, Support, testing};
use crate::grounding_observer::Work;

#[test]
fn mappings_borrow_distinct_original_occurrences() {
    let source = "d(1..3). p(1;2):-d(X),X<3.";
    let preparation = testing::prepare(source);
    let foreign = testing::prepare(source);
    let limits = preparation.limits;
    let location = preparation.location;
    let mut counters = Counters::resume(preparation.accounting, Work::default());
    let mut catalog = preparation.catalog;
    let (relations, mut append) = catalog.split(&limits, &mut counters, location).unwrap();
    let support = Support::indexed(&relations, &limits, &counters, location).unwrap();
    let computation = Computation::new(&mut append, &support);
    let components = computation.static_components(location).unwrap();
    let analysis = super::super::analyze(
        &preparation.program,
        components,
        computation.read(),
        crate::DomainLimits::default(),
        true,
    )
    .unwrap_or_else(|_| panic!("complete numeric analysis"));
    let rules = preparation.program.rules_without_facts();
    assert_eq!(analysis.occurrences.len(), 2);
    assert_eq!(rules[0].location, rules[1].location);
    for (index, occurrence) in analysis.occurrences.iter().enumerate() {
        assert!(std::ptr::eq(occurrence.source, &raw const rules[index]));
        assert!(!std::ptr::eq(
            occurrence.source,
            &raw const foreign.program.rules_without_facts()[index]
        ));
        assert_eq!(occurrence.requests.as_ref().unwrap().comparisons.len(), 1);
    }
    assert_ne!(
        analysis.occurrences[0].requests.as_ref().unwrap().rule,
        analysis.occurrences[1].requests.as_ref().unwrap().rule,
    );
    assert!(!preparation.program.analyzed.is_materialized());
}
