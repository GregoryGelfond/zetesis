use super::*;
use zetesis_ferraris::{AdmissionLimits, TightPlanLimits};

fn certificate() -> TightPlan {
    let theory = Theory::new(
        3,
        vec![
            Node::False,
            Node::Atom(0),
            Node::Atom(1),
            Node::Atom(2),
            Node::Implies(1, 0),
            Node::Or(1, 4),
            Node::And(1, 2),
            Node::Implies(6, 0),
            Node::Implies(4, 2),
        ],
        vec![8, 5, 7, 1],
        AdmissionLimits::default(),
    )
    .unwrap();
    TightPlan::compile(&theory, TightPlanLimits::default(), &Control::default()).unwrap()
}
fn graph() -> Graph {
    Graph::new(&certificate(), &wgpu::Limits::default()).unwrap()
}
fn plan(graph: &Graph, count: usize, fresh: bool) -> Plan {
    Plan::new(
        graph,
        count,
        TightGpuLimits::default(),
        &wgpu::Limits::default(),
        fresh,
        7,
    )
    .unwrap()
}

#[test]
fn certificate_packing_retains_original_formula_structure() {
    let certificate = certificate();
    let graph = Graph::new(&certificate, &wgpu::Limits::default()).unwrap();
    let packed = graph.pack(&certificate, &Control::default()).unwrap();
    assert_eq!(
        packed.nodes,
        vec![
            0, 0, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 1, 2, 0, 0, 4, 1, 0, 0, 3, 1, 4, 0, 2, 1, 2, 0, 4,
            6, 0, 0, 4, 4, 2, 0
        ]
    );
    assert_eq!(packed.roots, [8, 5, 7, 1]);
    assert_eq!(packed.producers, [1, 4, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(graph.work, 22);
    assert_eq!(
        plan(&graph, 2, true).params(&graph),
        [3, 9, 4, 3, 1, 2, 22, 7]
    );
}

#[test]
fn valid_rank_choices_do_not_change_cached_producers() {
    let original = certificate();
    let alternate = TightPlan::certify(
        original.theory(),
        &[2, 1, 0],
        TightPlanLimits::default(),
        &Control::default(),
    )
    .unwrap();
    assert_ne!(original.ranks(), alternate.ranks());
    let graph = Graph::new(&original, &wgpu::Limits::default()).unwrap();
    let first = graph.pack(&original, &Control::default()).unwrap();
    let second = graph.pack(&alternate, &Control::default()).unwrap();
    assert_eq!(first.producers, second.producers);
    assert_eq!(first.nodes, second.nodes);
    assert_eq!(first.roots, second.roots);
}

#[test]
fn packing_refuses_an_independent_equal_certificate() {
    assert_eq!(
        graph()
            .pack(&certificate(), &Control::default())
            .err()
            .unwrap()
            .kind(),
        GpuErrorKind::Seed
    );
}

#[test]
fn candidate_packing_preserves_repeated_occurrences() {
    let graph = graph();
    let input = [
        Interpretation::new(&graph.theory, [2, 0]).unwrap(),
        Interpretation::new(&graph.theory, [1]).unwrap(),
    ];
    let packed = plan(&graph, 3, true)
        .pack(
            &graph,
            &[input[0].clone(), input[1].clone(), input[0].clone()],
            &Control::default(),
        )
        .unwrap();
    assert_eq!(packed, [5, 2, 5]);
}

#[test]
fn candidate_identity_is_checked_before_encoding() {
    let graph = graph();
    let foreign = Interpretation::new(certificate().theory(), []).unwrap();
    assert_eq!(
        plan(&graph, 1, true)
            .pack(&graph, &[foreign], &Control::default())
            .unwrap_err()
            .kind(),
        GpuErrorKind::Seed
    );
}

#[test]
fn candidate_count_must_match_the_admitted_transport() {
    let graph = graph();
    assert_eq!(
        plan(&graph, 1, true)
            .pack(&graph, &[], &Control::default())
            .unwrap_err()
            .kind(),
        GpuErrorKind::Capacity
    );
}

#[test]
fn empty_carriers_keep_only_required_buffer_padding() {
    let theory = Theory::new(0, vec![], vec![], AdmissionLimits::default()).unwrap();
    let certificate =
        TightPlan::compile(&theory, TightPlanLimits::default(), &Control::default()).unwrap();
    let graph = Graph::new(&certificate, &wgpu::Limits::default()).unwrap();
    let packed = graph.pack(&certificate, &Control::default()).unwrap();
    assert_eq!(packed.nodes, [0; 4]);
    assert_eq!(packed.roots, [0]);
    assert_eq!(packed.producers, [0; 4]);
    assert_eq!(graph.bytes, 36);
    assert_eq!(graph.work, 0);
    assert_eq!(
        plan(&graph, 1, true)
            .pack(
                &graph,
                &[Interpretation::new(&theory, []).unwrap()],
                &Control::default()
            )
            .unwrap(),
        [0]
    );
}

#[test]
fn candidate_word_tails_never_introduce_atoms() {
    for count in [1, 31, 32, 33, 63, 64, 65] {
        let theory = Theory::new(count, vec![], vec![], AdmissionLimits::default()).unwrap();
        let certificate =
            TightPlan::compile(&theory, TightPlanLimits::default(), &Control::default()).unwrap();
        let graph = Graph::new(&certificate, &wgpu::Limits::default()).unwrap();
        let input = [
            Interpretation::new(&theory, 0..count).unwrap(),
            Interpretation::new(&theory, [count - 1]).unwrap(),
        ];
        let packed = plan(&graph, 2, true)
            .pack(&graph, &input, &Control::default())
            .unwrap();
        let width = count.div_ceil(32);
        assert_eq!(packed.len(), 2 * width);
        assert_eq!(packed[width - 1], u32::MAX >> (31 - (count - 1) % 32));
        assert_eq!(packed[2 * width - 1], 1 << ((count - 1) % 32));
        assert!(packed[width..2 * width - 1].iter().all(|word| *word == 0));
    }
}

#[test]
fn cold_bytes_include_simultaneous_host_graph_packing() {
    let graph = graph();
    let cold = plan(&graph, 2, true);
    let hot = plan(&graph, 2, false);
    assert_eq!(graph.bytes, 9 * 16 + 4 * 4 + 3 * 16);
    assert_eq!(hot.transport, 32 + 8 + 72 + 24 + 48 + 48);
    assert_eq!(cold.accounted - hot.accounted, graph.bytes);
    assert_eq!(
        hot.accounted,
        graph.bytes + hot.transport + 32 + 8 + 2 * std::mem::size_of::<TightGpuCheck>() as u64
    );
}

#[test]
fn byte_ceiling_includes_its_exact_boundary() {
    let graph = graph();
    for fresh in [false, true] {
        let bytes = plan(&graph, 2, fresh).accounted;
        for (max_batch_bytes, success) in [(bytes, true), (bytes - 1, false)] {
            let result = Plan::new(
                &graph,
                2,
                TightGpuLimits {
                    max_batch_bytes,
                    ..Default::default()
                },
                &wgpu::Limits::default(),
                fresh,
                1,
            );
            assert_eq!(result.is_ok(), success);
        }
    }
}

#[test]
fn full_scan_work_is_required_before_dispatch() {
    let graph = graph();
    for (max_work_per_candidate, success) in [(22, true), (21, false)] {
        assert_eq!(
            Plan::new(
                &graph,
                1,
                TightGpuLimits {
                    max_work_per_candidate,
                    ..Default::default()
                },
                &wgpu::Limits::default(),
                false,
                1
            )
            .is_ok(),
            success
        );
    }
}

#[test]
fn candidate_ceiling_includes_repeated_occurrences() {
    let graph = graph();
    assert!(
        Plan::new(
            &graph,
            2,
            TightGpuLimits {
                max_candidates: 2,
                ..Default::default()
            },
            &wgpu::Limits::default(),
            false,
            1
        )
        .is_ok()
    );
    assert!(
        Plan::new(
            &graph,
            3,
            TightGpuLimits {
                max_candidates: 2,
                ..Default::default()
            },
            &wgpu::Limits::default(),
            false,
            1
        )
        .is_err()
    );
}

#[test]
fn device_storage_limits_apply_before_host_packing() {
    let certificate = certificate();
    let tiny = wgpu::Limits {
        max_storage_buffer_binding_size: 143,
        ..Default::default()
    };
    assert_eq!(
        Graph::new(&certificate, &tiny).err().unwrap().kind(),
        GpuErrorKind::Capacity
    );
}

#[test]
fn transport_refuses_each_inadequate_device_limit() {
    let graph = graph();
    for field in 0..4 {
        let mut device = wgpu::Limits::default();
        match field {
            0 => device.max_compute_workgroups_per_dimension = 1,
            1 => device.max_storage_buffer_binding_size = 71,
            2 => device.max_uniform_buffer_binding_size = 31,
            _ => device.max_buffer_size = 71,
        }
        assert_eq!(
            Plan::new(&graph, 2, TightGpuLimits::default(), &device, false, 1)
                .err()
                .unwrap()
                .kind(),
            GpuErrorKind::Capacity
        );
    }
}

#[test]
fn integer_overflow_is_a_capacity_refusal() {
    if usize::BITS > 32 {
        assert!(address(usize::MAX).is_err());
    }
    assert!(sum(&[u64::MAX, 1]).is_err());
    let mut graph = graph();
    graph.nodes = u32::MAX;
    assert!(
        Plan::new(
            &graph,
            2,
            TightGpuLimits::default(),
            &wgpu::Limits::default(),
            false,
            1
        )
        .is_err()
    );
}

#[test]
fn zero_epoch_cannot_alias_cleared_result_storage() {
    assert!(
        Plan::new(
            &graph(),
            1,
            TightGpuLimits::default(),
            &wgpu::Limits::default(),
            false,
            0
        )
        .is_err()
    );
}

#[test]
fn false_root_witnesses_use_assertion_order() {
    let graph = graph();
    let plan = plan(&graph, 2, false);
    let records = [7, 0, 1, 0, 22, MAGIC, 7, 1, 1, 3, 22, MAGIC];
    let result = decode(&records, &graph, &plan, &Control::default()).unwrap();
    assert_eq!(result[0].verdict(), TightVerdict::NotModel { root: 8 });
    assert_eq!(result[1].verdict(), TightVerdict::NotModel { root: 1 });
    assert_eq!(result[0].work(), 22);
}

#[test]
fn all_verdict_kinds_decode_without_losing_witnesses() {
    let graph = graph();
    let plan = plan(&graph, 3, false);
    let records = [
        7, 0, 0, 0, 22, MAGIC, 7, 1, 2, 2, 22, MAGIC, 7, 2, 1, 1, 22, MAGIC,
    ];
    let result = decode(&records, &graph, &plan, &Control::default()).unwrap();
    assert_eq!(
        result
            .iter()
            .map(TightGpuCheck::verdict)
            .collect::<Vec<_>>(),
        [
            TightVerdict::Stable,
            TightVerdict::Residual {
                unsupported_atom: 2
            },
            TightVerdict::NotModel { root: 5 }
        ]
    );
}

#[test]
fn corrupt_records_never_produce_a_partial_batch() {
    let graph = graph();
    let plan = plan(&graph, 2, false);
    let valid = [7, 0, 0, 0, 22, MAGIC, 7, 1, 2, 2, 22, MAGIC];
    for (index, value) in [(6, 0), (7, 0), (8, 3), (9, 3), (10, 21), (11, 0), (8, 0)] {
        let mut corrupt = valid;
        corrupt[index] = value;
        assert_eq!(
            decode(&corrupt, &graph, &plan, &Control::default())
                .unwrap_err()
                .kind(),
            GpuErrorKind::Readback
        );
    }
    assert_eq!(
        decode(&valid[..11], &graph, &plan, &Control::default())
            .unwrap_err()
            .kind(),
        GpuErrorKind::Readback
    );
}

#[test]
fn out_of_range_root_ordinals_are_readback_failures() {
    let graph = graph();
    let plan = plan(&graph, 1, false);
    assert_eq!(
        decode(&[7, 0, 1, 4, 22, MAGIC], &graph, &plan, &Control::default())
            .unwrap_err()
            .kind(),
        GpuErrorKind::Readback
    );
}

#[test]
fn cancellation_prevents_packing_or_decoding_results() {
    let certificate = certificate();
    let graph = Graph::new(&certificate, &wgpu::Limits::default()).unwrap();
    let plan = plan(&graph, 1, true);
    let control = Control::default();
    control.cancel();
    let input = [Interpretation::new(&graph.theory, []).unwrap()];
    for error in [
        graph.pack(&certificate, &control).err().unwrap(),
        plan.pack(&graph, &input, &control).unwrap_err(),
        decode(&[7, 0, 0, 0, 22, MAGIC], &graph, &plan, &control).unwrap_err(),
    ] {
        assert_eq!(error.interruption, Some(zetesis_cpu::Stop::Cancelled));
    }
}
