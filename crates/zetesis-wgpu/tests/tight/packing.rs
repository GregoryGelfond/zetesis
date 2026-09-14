use super::*;
use zetesis_ferraris::{AdmissionLimits, TightPlanLimits};

fn atomic(device: &wgpu::Limits) -> Packing<'_> {
    Packing {
        device,
        support: TightSupport::Atomic,
    }
}

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
    Graph::new(&certificate(), atomic(&wgpu::Limits::default())).unwrap()
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
fn tight_dispatch_requires_a_positive_wait() {
    let graph = graph();
    for fresh in [false, true] {
        for (timeout, admitted) in [
            (std::time::Duration::ZERO, false),
            (std::time::Duration::from_nanos(1), true),
        ] {
            let result = Plan::new(
                &graph,
                2,
                TightGpuLimits {
                    timeout,
                    ..TightGpuLimits::default()
                },
                &wgpu::Limits::default(),
                fresh,
                1,
            );
            assert_eq!(result.is_ok(), admitted);
            if let Err(error) = result {
                assert_eq!(error.kind(), GpuErrorKind::Capacity);
            }
        }
    }
}

// The wire-decoder tests below use candidates containing this fixture's entire
// carrier; dedicated controls supply absent bits and malformed seed shapes.
fn decode_present(
    records: &[u32],
    graph: &Graph,
    plan: &Plan,
    control: &Control,
) -> Result<Vec<TightGpuCheck>, GpuError> {
    let seeds = vec![7; usize::try_from(plan.seeds / 4).unwrap()];
    super::decode(records, graph, plan, &seeds, control)
}

#[test]
fn certificate_packing_retains_original_formula_structure() {
    let certificate = certificate();
    let graph = Graph::new(&certificate, atomic(&wgpu::Limits::default())).unwrap();
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
    assert_eq!(graph.work, 20);
    assert_eq!(
        plan(&graph, 2, true).params(&graph),
        [3, 9, 4, 3, 1, 2, 20, 7]
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
    for support in [TightSupport::Atomic, TightSupport::Grouped] {
        let graph = Graph::new(
            &original,
            Packing {
                device: &wgpu::Limits::default(),
                support,
            },
        )
        .unwrap();
        let first = graph.pack(&original, &Control::default()).unwrap();
        let second = graph.pack(&alternate, &Control::default()).unwrap();
        assert_eq!(first.producers, second.producers);
        assert_eq!(first.nodes, second.nodes);
        assert_eq!(first.roots, second.roots);
    }
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
    let graph = Graph::new(&certificate, atomic(&wgpu::Limits::default())).unwrap();
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
        let graph = Graph::new(&certificate, atomic(&wgpu::Limits::default())).unwrap();
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
fn support_storage_uses_packed_world_rows() {
    for (atoms, width) in [
        (0, 0),
        (1, 1),
        (31, 1),
        (32, 1),
        (33, 2),
        (63, 2),
        (64, 2),
        (65, 3),
    ] {
        let theory = Theory::new(atoms, vec![], vec![], AdmissionLimits::default()).unwrap();
        let certificate =
            TightPlan::compile(&theory, TightPlanLimits::default(), &Control::default()).unwrap();
        let graph = Graph::new(&certificate, atomic(&wgpu::Limits::default())).unwrap();
        for worlds in [1, 3, 65] {
            let expected = u64::try_from((width * worlds).max(1)).unwrap() * 4;
            assert_eq!(plan(&graph, worlds, false).support, expected);
        }
    }
}

#[test]
fn support_work_charges_word_initialization() {
    for (atoms, expected) in [
        (0, 0),
        (1, 2),
        (31, 32),
        (32, 33),
        (33, 35),
        (63, 65),
        (64, 66),
        (65, 68),
    ] {
        let theory = Theory::new(atoms, vec![], vec![], AdmissionLimits::default()).unwrap();
        let certificate =
            TightPlan::compile(&theory, TightPlanLimits::default(), &Control::default()).unwrap();
        let graph = Graph::new(&certificate, atomic(&wgpu::Limits::default())).unwrap();
        assert_eq!(graph.work, expected);
    }
}

#[test]
fn cold_bytes_include_simultaneous_host_graph_packing() {
    let graph = graph();
    let cold = plan(&graph, 2, true);
    let hot = plan(&graph, 2, false);
    assert_eq!(graph.bytes, 9 * 16 + 4 * 4 + 3 * 16);
    assert_eq!(hot.transport, 32 + 8 + 72 + 8 + 48 + 48);
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
    for (max_work_per_candidate, success) in [(20, true), (19, false)] {
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
        Graph::new(&certificate, atomic(&tiny))
            .err()
            .unwrap()
            .kind(),
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
    let records = [7, 0, 1, 0, 20, RESULT_MAGIC, 7, 1, 1, 3, 20, RESULT_MAGIC];
    let result = decode_present(&records, &graph, &plan, &Control::default()).unwrap();
    assert_eq!(result[0].verdict(), TightVerdict::NotModel { root: 8 });
    assert_eq!(result[1].verdict(), TightVerdict::NotModel { root: 1 });
    assert_eq!(result[0].work(), 20);
}

#[test]
fn all_verdict_kinds_decode_without_losing_witnesses() {
    let graph = graph();
    let plan = plan(&graph, 3, false);
    let records = [
        7,
        0,
        0,
        0,
        20,
        RESULT_MAGIC,
        7,
        1,
        2,
        2,
        20,
        RESULT_MAGIC,
        7,
        2,
        1,
        1,
        20,
        RESULT_MAGIC,
    ];
    let result = decode_present(&records, &graph, &plan, &Control::default()).unwrap();
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
    let valid = [7, 0, 0, 0, 20, RESULT_MAGIC, 7, 1, 2, 2, 20, RESULT_MAGIC];
    for (index, value) in [(6, 0), (7, 0), (8, 3), (9, 3), (10, 19), (11, 0), (8, 0)] {
        let mut corrupt = valid;
        corrupt[index] = value;
        assert_eq!(
            decode_present(&corrupt, &graph, &plan, &Control::default())
                .unwrap_err()
                .kind(),
            GpuErrorKind::Readback
        );
    }
    assert_eq!(
        decode_present(&valid[..11], &graph, &plan, &Control::default())
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
        decode_present(
            &[7, 0, 1, 4, 20, RESULT_MAGIC],
            &graph,
            &plan,
            &Control::default()
        )
        .unwrap_err()
        .kind(),
        GpuErrorKind::Readback
    );
}

#[test]
fn cancellation_prevents_completed_results() {
    let certificate = certificate();
    let graph = Graph::new(&certificate, atomic(&wgpu::Limits::default())).unwrap();
    let plan = plan(&graph, 1, true);
    let control = Control::default();
    control.cancel();
    let input = [Interpretation::new(&graph.theory, []).unwrap()];
    for error in [
        graph.pack(&certificate, &control).err().unwrap(),
        plan.pack(&graph, &input, &control).unwrap_err(),
        decode_present(&[7, 0, 0, 0, 20, RESULT_MAGIC], &graph, &plan, &control).unwrap_err(),
    ] {
        assert_eq!(error.interruption, Some(zetesis_cpu::Stop::Cancelled));
    }
}

#[test]
fn absent_residual_witnesses_are_readback_failures() {
    let graph = graph();
    let plan = plan(&graph, 2, false);
    let records = [7, 0, 2, 2, 20, RESULT_MAGIC, 7, 1, 2, 2, 20, RESULT_MAGIC];
    let error = super::decode(&records, &graph, &plan, &[7, 3], &Control::default()).unwrap_err();
    assert_eq!(error.kind(), GpuErrorKind::Readback);
    assert!(super::decode(&records, &graph, &plan, &[7, 4], &Control::default()).is_ok());
}

#[test]
fn malformed_candidate_storage_cannot_validate_receipts() {
    let graph = graph();
    let plan = plan(&graph, 1, false);
    assert_eq!(
        super::decode(
            &[7, 0, 0, 0, 20, RESULT_MAGIC],
            &graph,
            &plan,
            &[],
            &Control::default()
        )
        .unwrap_err()
        .kind(),
        GpuErrorKind::Readback
    );
}

fn empty_graph() -> Graph {
    let theory = Theory::new(0, vec![], vec![], AdmissionLimits::default()).unwrap();
    let certificate =
        TightPlan::compile(&theory, TightPlanLimits::default(), &Control::default()).unwrap();
    Graph::new(&certificate, atomic(&wgpu::Limits::default())).unwrap()
}

#[test]
fn empty_storage_padding_never_becomes_a_witness() {
    let graph = empty_graph();
    let empty = plan(&graph, 1, false);
    let control = Control::default();
    let stable = super::decode(
        &[7, 0, STATUS_STABLE, 0, 0, RESULT_MAGIC],
        &graph,
        &empty,
        &[0],
        &control,
    )
    .unwrap();
    assert_eq!(stable[0].verdict(), TightVerdict::Stable);
    assert_eq!(stable[0].work(), 0);
    for status in [STATUS_NOT_MODEL, STATUS_RESIDUAL] {
        let error = super::decode(
            &[7, 0, status, 0, 0, RESULT_MAGIC],
            &graph,
            &empty,
            &[0],
            &control,
        )
        .unwrap_err();
        assert_eq!(error.kind(), GpuErrorKind::Readback);
    }

    // Zero is a valid ordinal when an actual asserted root occupies it.
    let theory = Theory::new(0, vec![Node::False], vec![0], AdmissionLimits::default()).unwrap();
    let certificate = TightPlan::compile(&theory, TightPlanLimits::default(), &control).unwrap();
    let graph = Graph::new(&certificate, atomic(&wgpu::Limits::default())).unwrap();
    let asserted = plan(&graph, 1, false);
    let result = super::decode(
        &[7, 0, STATUS_NOT_MODEL, 0, 2, RESULT_MAGIC],
        &graph,
        &asserted,
        &[0],
        &control,
    )
    .unwrap();
    assert_eq!(result[0].verdict(), TightVerdict::NotModel { root: 0 });
}

#[test]
fn uniform_storage_requires_its_full_size() {
    let graph = empty_graph();
    for (max_buffer_size, admitted) in [(32, true), (31, false)] {
        let device = wgpu::Limits {
            max_buffer_size,
            ..Default::default()
        };
        let result = Plan::new(&graph, 1, TightGpuLimits::default(), &device, false, 1);
        assert_eq!(result.is_ok(), admitted);
        match result {
            Ok(plan) => {
                // Every storage buffer fits even the smaller limit. Only the
                // uniform's independent 32-byte requirement separates the cases.
                assert_eq!(
                    (plan.seeds, plan.truth, plan.support, plan.results),
                    (4, 4, 4, 24)
                );
            }
            Err(error) => {
                assert_eq!(error.kind(), GpuErrorKind::Capacity);
                assert_eq!(
                    error.detail(),
                    "tight uniform buffer exceeds granted device limits"
                );
            }
        }
    }
}

#[test]
fn result_addressing_has_an_independent_ceiling() {
    let graph = empty_graph();
    let device = wgpu::Limits {
        max_compute_workgroups_per_dimension: u32::MAX,
        max_buffer_size: u64::MAX,
        max_storage_buffer_binding_size: u64::MAX,
        ..Default::default()
    };
    let limits = TightGpuLimits {
        max_candidates: usize::MAX,
        max_batch_bytes: u64::MAX,
        ..Default::default()
    };
    // Only arithmetic plans are created. No candidate or result buffer is
    // allocated for these deliberately hypothetical device dimensions.
    let largest = usize::try_from(u32::MAX).unwrap() / RESULT_WORDS;
    let boundary = Plan::new(&graph, largest, limits, &device, false, 1).unwrap();
    assert_eq!(boundary.results, u64::try_from(largest).unwrap() * 6 * 4);
    assert_eq!(
        (boundary.seeds, boundary.truth, boundary.support),
        (4, 4, 4)
    );
    let error = Plan::new(&graph, largest + 1, limits, &device, false, 1)
        .err()
        .unwrap();
    assert_eq!(error.kind(), GpuErrorKind::Capacity);
    assert_eq!(error.detail(), "tight world offset exceeds u32");
}
