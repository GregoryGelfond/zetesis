//! Shared device composition without changing either primitive's semantic subject.

use crate::support::physical;

use zetesis_backend::{ComputeTimingStatus, GpuApi};
use zetesis_core::{
    Atom, Predicate, Value,
    relation::{Limits, Relation},
};
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{AdmissionLimits, Interpretation, Node, Theory};
use zetesis_wgpu::{
    FormulaCheck, FormulaLimits, FormulaVerdict, GpuContext, GpuFormulaOracle, GpuOptions,
    GpuRelationExecutor, GpuSelection, RelationGpuLimits,
};

fn qualify_timing(context: &GpuContext) -> bool {
    println!(
        "optional features: advertised={:?}; enabled={:?}",
        context.info().features(),
        context.features()
    );
    assert!(context.info().features().contains(context.features()));
    let timing_enabled = context
        .info()
        .features()
        .contains(wgpu::Features::TIMESTAMP_QUERY);
    assert_eq!(
        context.compute_timing(),
        if timing_enabled {
            ComputeTimingStatus::Enabled
        } else {
            ComputeTimingStatus::Unsupported
        }
    );
    assert_eq!(
        context.features().contains(wgpu::Features::TIMESTAMP_QUERY),
        timing_enabled
    );
    timing_enabled
}

fn compose(backend: GpuApi) {
    let context =
        GpuContext::new_selected_with_timing(GpuOptions::default(), GpuSelection { api: backend })
            .unwrap();
    physical::verify(backend, context.info());
    let timing_enabled = qualify_timing(&context);
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
    let queries = [relation
        .query(&[(0, (&value).into())], Limits::default())
        .unwrap()];
    let mut prepared = relation_executor
        .prepare(
            &relation,
            RelationGpuLimits::default(),
            &Cancellation::default(),
        )
        .unwrap();
    // These are two different subjects on the same device. Equal bit widths are
    // not used to reinterpret row masks as candidate or aggregate membership.
    let theory = Theory::new(
        1,
        zetesis_ferraris::FormulaParts::new(vec![Node::atom(0)], Vec::new()).unwrap(),
        vec![0],
        AdmissionLimits::default(),
    )
    .unwrap();
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
        // Zero is a valid observation at the adapter's timestamp resolution.
        assert_eq!(formula.last_compute_time().is_some(), timing_enabled);
        let stats = formula.last_batch_stats().unwrap();
        assert_eq!(stats.theory_uploaded, repetition == 0);
        assert_eq!(stats.transport_allocated, repetition == 0);
        if repetition == 0 {
            compare_timing_accounting(backend, &theory, &candidates, &checks, &formula);
        }
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
    assert_eq!(formula.last_compute_time().is_some(), timing_enabled);
    assert!(
        formula
            .propagate_batch(&theory, &[], FormulaLimits::default())
            .unwrap()
            .is_empty()
    );
    assert_eq!(formula.last_compute_time(), None);
}

fn compare_timing_accounting(
    backend: GpuApi,
    theory: &Theory,
    candidates: &[Interpretation],
    checks: &[FormulaCheck],
    formula: &GpuFormulaOracle,
) {
    let mut untimed =
        GpuFormulaOracle::new_selected(GpuOptions::default(), GpuSelection { api: backend })
            .unwrap();
    let untimed_checks = untimed
        .propagate_batch(theory, candidates, FormulaLimits::default())
        .unwrap();
    assert_eq!(
        untimed.context().compute_timing(),
        ComputeTimingStatus::NotRequested
    );
    assert_eq!(untimed.last_compute_time(), None);
    let untimed_cold = untimed.last_batch_stats().unwrap();
    // Two query slots, a resolve buffer and the appended readback: 48 logical
    // bytes, excluding any driver-private query allocation.
    let timing_bytes = if formula.context().compute_timing() == ComputeTimingStatus::Enabled {
        48
    } else {
        0
    };

    let stats = formula.last_batch_stats().unwrap();
    assert_eq!(
        stats.resident_transport_bytes,
        untimed_cold.resident_transport_bytes + timing_bytes
    );
    assert_eq!(
        stats.accounted_bytes,
        untimed_cold.accounted_bytes + timing_bytes
    );
    assert_eq!(checks.len(), untimed_checks.len());
    for (actual, expected) in checks.iter().zip(&untimed_checks) {
        assert_eq!(actual.verdict(), expected.verdict());
    }
}

#[test]
#[ignore = "requires Metal: formula executes while relation columns remain prepared"]
fn metal_formula_executes_while_relation_columns_remain_prepared() {
    compose(GpuApi::Metal);
}

#[test]
#[ignore = "requires Vulkan: formula executes while relation columns remain prepared"]
fn vulkan_formula_executes_while_relation_columns_remain_prepared() {
    compose(GpuApi::Vulkan);
}
