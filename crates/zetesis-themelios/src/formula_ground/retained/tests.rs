use super::*;
use crate::formula_support::{GroundingWork, testing};
use crate::grounding_observer::Work;

#[test]
fn retained_grounding_keeps_accepted_source_history() {
    let preparation = testing::prepare("seed(1). result(X) :- seed(X).");
    let before = preparation.accounting.work;
    let grounded = ground_retained(preparation, None, false).unwrap();
    assert!(grounded.accounting.work > before);
    assert_eq!(grounded.budget.usage(), grounded.compiled.expansion);
    assert!(grounded.output_storage.bytes() >= grounded.envelope_bytes());
}

#[test]
fn retained_publication_shares_its_closed_source() {
    let preparation = testing::prepare("seed(1). result(X) :- seed(X).");
    let limits = preparation.limits;
    let location = preparation.location;
    let grounded = ground_retained(preparation, None, false).unwrap();
    let RetainedGrounding {
        compiled,
        catalog,
        accounting,
        budget: _,
        streamed: _,
        output_storage,
    } = grounded;
    let mut counters = Counters::resume(accounting, Work::default());
    let closed = catalog
        .into_closed(0, GroundingWork::new(&limits, &mut counters, location))
        .unwrap();
    let metadata = closed
        .storage
        .prior_publication_metadata_bytes(&compiled.atoms)
        .unwrap();
    assert!(metadata >= compiled.atoms.publication_bytes());
    assert!(counters.workspace_bytes() >= output_storage.bytes());
    drop(output_storage);
    assert!(counters.into_accounting().into_flat_baseline().is_ok());
}

#[test]
fn retained_grounding_preserves_the_materialized_theory() {
    let source = "{pick(1); pick(2)}. result(X) :- pick(X).";
    let ordinary = crate::formula_ground::ground(testing::prepare(source), None, None).unwrap();
    let retained = ground_retained(testing::prepare(source), None, false).unwrap();
    assert_eq!(ordinary.theory.nodes(), retained.compiled.theory.nodes());
    assert_eq!(ordinary.theory.roots(), retained.compiled.theory.roots());
    assert_eq!(ordinary.atoms.atoms(), retained.compiled.atoms.atoms());
}
