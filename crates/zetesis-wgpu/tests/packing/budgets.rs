//! Host admission contracts, using admitted programs and actual CPU semantics.
//! These tests do not instantiate or simulate a GPU device.

use super::{BatchPlan, GraphPlan, PackedGraph, PackedSeeds, decode};
use crate::{GpuErrorKind, GpuLimits};
use std::time::Duration;
use zetesis_core::{
    AdmissionLimits, AtomPattern, GroundProgram, Predicate, Program, Seed, StaticLimits, Template,
};
use zetesis_cpu::{Control, Limits, check_static};

fn fixture() -> GroundProgram {
    let atom = |name| AtomPattern::new(Predicate::new(name, 0).unwrap(), vec![]).unwrap();
    let a = atom("a");
    let b = atom("b");
    let program = Program::new(
        vec![
            Template::new(Some(a.clone()), vec![], vec![a.clone()], vec![], vec![]),
            Template::new(Some(b.clone()), vec![a], vec![], vec![], vec![]),
            Template::new(None, vec![b], vec![], vec![], vec![]),
        ],
        AdmissionLimits::default(),
    )
    .unwrap();
    GroundProgram::compile(&program, StaticLimits::default()).unwrap()
}

fn assert_same_plan_and_semantics(
    program: &GroundProgram,
    baseline: &BatchPlan,
    retried: &BatchPlan,
) {
    assert_eq!(
        (
            baseline.atom_count,
            baseline.word_count,
            baseline.rule_count,
            baseline.world_count
        ),
        (
            retried.atom_count,
            retried.word_count,
            retried.rule_count,
            retried.world_count
        ),
    );
    assert_eq!(
        (
            baseline.seed_bytes,
            baseline.result_bytes,
            baseline.transport_bytes,
            baseline.accounted_bytes
        ),
        (
            retried.seed_bytes,
            retried.result_bytes,
            retried.transport_bytes,
            retried.accounted_bytes
        ),
    );
    assert_eq!(program.atoms()[0].predicate().name(), "a");
    let seeds = [
        Seed::new(program.program(), []).unwrap(),
        Seed::new(program.program(), [program.atoms()[0].clone()]).unwrap(),
    ];
    let before = PackedSeeds::new(program, &seeds, baseline).unwrap();
    let after = PackedSeeds::new(program, &seeds, retried).unwrap();
    assert_eq!(before.params, after.params);
    assert_eq!(before.seeds, after.seeds);
    assert_eq!(after.params, [2, 1, 3, 2]);
    assert_eq!(after.seeds, [0, 1]);
    // Empty a leaves no consequences. Choosing a derives both a and b, then
    // violates the b constraint; the gate projection itself still matches.
    let words = [0, 0, 1, 3];
    let before = decode(&words, baseline).unwrap();
    let after = decode(&words, retried).unwrap();
    assert_eq!(before, after);
    assert!(after[0].accepted());
    assert!(after[1].constraint_violated());
    assert!(!after[1].seed_mismatch());
    for (seed, decoded) in seeds.iter().zip(&after) {
        let cpu = check_static(program, seed, Limits::default(), &Control::default()).unwrap();
        assert_eq!(decoded.closure_words(), cpu.closure_words());
        assert_eq!(decoded.accepted(), cpu.accepted());
        assert_eq!(decoded.constraint_violated(), cpu.constraint_violated());
        assert_eq!(decoded.seed_mismatch(), cpu.seed_mismatch());
    }
}

#[test]
fn zero_wait_timeout_is_refused_and_positive_retry_preserves_the_host_contract() {
    let program = fixture();
    let device = wgpu::Limits::default();
    let graph = GraphPlan::new(&program, &device).unwrap();
    let packed = PackedGraph::new(&program, &graph).unwrap();
    for upload in [false, true] {
        let baseline =
            BatchPlan::for_graph(&graph, 2, GpuLimits::default(), &device, upload).unwrap();
        let refused = BatchPlan::for_graph(
            &graph,
            2,
            GpuLimits {
                timeout: Duration::ZERO,
                ..GpuLimits::default()
            },
            &device,
            upload,
        );
        let error = refused
            .err()
            .expect("zero wait must not reach a device dispatch");
        assert_eq!(error.kind(), GpuErrorKind::Capacity);
        assert_eq!(error.detail(), "the GPU wait timeout must be positive");
        // This proves positive host admission, not that a device completes in 1ns.
        let retried = BatchPlan::for_graph(
            &graph,
            2,
            GpuLimits {
                timeout: Duration::from_nanos(1),
                ..GpuLimits::default()
            },
            &device,
            upload,
        )
        .unwrap();
        assert_same_plan_and_semantics(&program, &baseline, &retried);
    }
    let unchanged = PackedGraph::new(&program, &graph).unwrap();
    assert_eq!(packed.rules, unchanged.rules);
    assert_eq!(packed.antecedents, unchanged.antecedents);
    assert_eq!(packed.carrier, unchanged.carrier);
}

#[test]
fn uniform_binding_requires_all_sixteen_parameter_bytes_and_recovers_at_equality() {
    let program = fixture();
    let device = wgpu::Limits::default();
    let graph = GraphPlan::new(&program, &device).unwrap();
    for upload in [false, true] {
        let baseline =
            BatchPlan::for_graph(&graph, 2, GpuLimits::default(), &device, upload).unwrap();
        let insufficient = wgpu::Limits {
            max_uniform_buffer_binding_size: 15,
            ..device.clone()
        };
        let refused = BatchPlan::for_graph(&graph, 2, GpuLimits::default(), &insufficient, upload);
        let error = refused
            .err()
            .expect("15 bytes cannot bind the authored ABI");
        assert_eq!(error.kind(), GpuErrorKind::Capacity);
        assert_eq!(
            error.detail(),
            "device cannot bind the 16-byte parameter buffer"
        );
        let exact = wgpu::Limits {
            max_uniform_buffer_binding_size: 16,
            ..device.clone()
        };
        let retried =
            BatchPlan::for_graph(&graph, 2, GpuLimits::default(), &exact, upload).unwrap();
        assert_same_plan_and_semantics(&program, &baseline, &retried);
    }
}
