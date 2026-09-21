//! Shared device composition without changing either primitive's semantic subject.

#[path = "support/physical.rs"]
mod physical;

use zetesis_core::{
    Atom, Predicate, Value,
    relation::{Limits, Relation},
};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{AdmissionLimits, Interpretation, Node, Theory};
use zetesis_wgpu::{
    FormulaLimits, FormulaVerdict, GpuContext, GpuFormulaOracle, GpuOptions, GpuRelationExecutor,
    RelationGpuLimits,
};

fn compose(backend: physical::Backend) {
    let context = GpuContext::new_selected(GpuOptions::default(), backend.selection()).unwrap();
    backend.verify(context.info());
    println!(
        "optional features: advertised={:?}; enabled={:?}",
        context.info().features(),
        context.features()
    );
    assert!(context.info().features().contains(context.features()));
    let mut relation_executor = GpuRelationExecutor::from_context(&context).unwrap();
    let mut formula = GpuFormulaOracle::from_context(&context.clone()).unwrap();
    assert!(context.same_instance(relation_executor.context()));
    assert!(context.same_instance(formula.context()));

    let predicate = Predicate::new("value", 1).unwrap();
    let atoms = [
        Atom::new(predicate.clone(), vec![Value::Number(7)]).unwrap(),
        Atom::new(predicate.clone(), vec![Value::String("7".into())]).unwrap(),
    ];
    let indices = [1, 0, 0];
    let relation = Relation::from_catalog(&predicate, &atoms, &indices, Limits::default()).unwrap();
    let value = Value::Number(7);
    let queries = [relation.query(&[(0, &value)], Limits::default()).unwrap()];
    let mut prepared = relation_executor
        .prepare(
            &relation,
            RelationGpuLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    // These are two different subjects on the same device. Equal bit widths are
    // not used to reinterpret row masks as candidate or aggregate membership.
    let theory = Theory::new(1, vec![Node::Atom(0)], vec![0], AdmissionLimits::default()).unwrap();
    let candidates = [
        Interpretation::new(&theory, []).unwrap(),
        Interpretation::new(&theory, [0]).unwrap(),
    ];
    for repetition in 0..2 {
        let checks = formula
            .propagate_batch(&theory, &candidates, FormulaLimits::default())
            .unwrap();
        assert_eq!(checks[0].verdict(), FormulaVerdict::NotModel);
        assert_eq!(checks[1].verdict(), FormulaVerdict::NoProperSubset);
        let stats = formula.last_batch_stats().unwrap();
        assert_eq!(stats.theory_uploaded, repetition == 0);
        assert_eq!(stats.transport_allocated, repetition == 0);
        let masks = prepared
            .filter(
                &queries,
                RelationGpuLimits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        assert!(relation.same_owner(masks.relation()));
        let selected = masks.selection(0, Limits::default()).unwrap();
        assert_eq!(selected.positions(), [1, 2]);
        assert_eq!(selected.row(0).unwrap().source_index(), 0);
        assert_eq!(selected.row(1).unwrap().source_index(), 0);
        assert_eq!(prepared.activity().submissions, 1);
        assert_eq!(prepared.activity().completed_queries, 1);
    }
    // Dropping the original handle does not remove the device from its clients.
    drop(context);
    let checks = formula
        .propagate_batch(&theory, &candidates, FormulaLimits::default())
        .unwrap();
    assert_eq!(checks[1].verdict(), FormulaVerdict::NoProperSubset);
    assert!(!formula.last_batch_stats().unwrap().theory_uploaded);
}

#[test]
#[ignore = "requires actual Metal; explicit physical qualification"]
fn metal_formula_executes_while_relation_columns_remain_prepared() {
    compose(physical::Backend::Metal);
}

#[test]
#[ignore = "requires an actual Vulkan GPU; explicit physical qualification"]
fn vulkan_formula_executes_while_relation_columns_remain_prepared() {
    compose(physical::Backend::Vulkan);
}
