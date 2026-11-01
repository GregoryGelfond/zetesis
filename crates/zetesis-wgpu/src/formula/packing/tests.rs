use super::super::preparation::{Preparation, PreparedGraph};
use super::*;
use zetesis_ferraris::{AdmissionLimits, Node};

fn prepared(theory: &Theory, device: &wgpu::Limits) -> Result<PreparedGraph, GpuError> {
    Preparation::new(theory, 2, FormulaLimits::default(), device, 7)?
        .finish(device, &zetesis_algebra::Cancellation::default())
        .map(|(prepared, _)| prepared)
}
fn graph(theory: &Theory, device: &wgpu::Limits) -> Result<Graph, GpuError> {
    prepared(theory, device).map(|prepared| prepared.graph)
}

fn theory() -> Theory {
    Theory::new(
        2,
        zetesis_ferraris::FormulaParts::new(
            vec![
                Node::falsum(),
                Node::atom(0),
                Node::atom(1),
                Node::and_pair([1, 2]),
                Node::or_pair([1, 2]),
                Node::implies(3, 4),
            ],
            Vec::new(),
        )
        .unwrap(),
        vec![5],
        AdmissionLimits::default(),
    )
    .unwrap()
}
fn device() -> wgpu::Limits {
    wgpu::Limits::default()
}
fn plan(graph: &Graph, limits: FormulaLimits, fresh: bool) -> Plan {
    Plan::new(graph, 2, limits, &device(), fresh, 7).unwrap()
}

fn candidate_words(
    plan: &Plan,
    graph: &Graph,
    candidates: &[Interpretation],
    cancellation: &zetesis_algebra::Cancellation,
) -> Result<Vec<u32>, GpuError> {
    let mut staging = crate::candidates::Staging::default();
    plan.pack(graph, candidates, &mut staging, u64::MAX, cancellation)?;
    Ok(staging.words().to_vec())
}

#[test]
fn formula_dispatch_requires_a_positive_wait() {
    let graph = graph(&theory(), &device()).unwrap();
    for fresh in [false, true] {
        for (timeout, admitted) in [
            (std::time::Duration::ZERO, false),
            (std::time::Duration::from_nanos(1), true),
        ] {
            let result = Plan::new(
                &graph,
                2,
                FormulaLimits {
                    timeout,
                    ..FormulaLimits::default()
                },
                &device(),
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

#[test]
fn packed_nodes_preserve_topology_shared_atom_ids_and_original_operators() {
    let prepared = prepared(&theory(), &device()).unwrap();
    let graph = prepared.graph;
    let (nodes, roots) = (prepared.nodes, prepared.roots);
    assert_eq!(
        nodes,
        vec![
            0, 0, 0, 2, 1, 0, 0, 0, 1, 1, 0, 1, 2, 1, 2, 3, 3, 1, 2, 4, 4, 3, 4, 5
        ]
    );
    assert_eq!(roots, vec![5, 0, 3, 5, 6, 0, 1, 2, 3, 4, 5]);
    assert_eq!(
        (graph.schedule.setup_work, graph.shape.sweep_work),
        (28, 133)
    );
    let limits = FormulaLimits::default();
    let plan = plan(&graph, limits, true);
    assert_eq!(
        plan.params(&graph),
        [2, 6, 1, 6, 1, 0, 64, 100_000_000, 28, 136, 7, 3, 0, 0, 0, 0]
    );
    let a = Interpretation::new(&graph.shape.theory, [0]).unwrap();
    let b = Interpretation::new(&graph.shape.theory, [1]).unwrap();
    assert_eq!(
        candidate_words(
            &plan,
            &graph,
            &[a, b],
            &zetesis_algebra::Cancellation::default()
        )
        .unwrap(),
        vec![1, 2]
    );
    let foreign = Interpretation::new(&theory(), []).unwrap();
    assert_eq!(
        candidate_words(
            &plan,
            &graph,
            &[foreign.clone(), foreign],
            &zetesis_algebra::Cancellation::default()
        )
        .unwrap_err()
        .kind(),
        GpuErrorKind::Seed
    );
}
#[test]
fn fresh_resident_and_transport_budgets_are_inclusive_and_fully_accounted() {
    let graph = graph(&theory(), &device()).unwrap();
    let cold = plan(&graph, FormulaLimits::default(), true);
    let hot = plan(&graph, FormulaLimits::default(), false);
    assert_eq!(cold.accounted - hot.accounted, graph.schedule.bytes);
    for fresh in [false, true] {
        let bytes = plan(&graph, FormulaLimits::default(), fresh).accounted;
        for (limit, ok) in [(bytes, true), (bytes - 1, false)] {
            assert_eq!(
                Plan::new(
                    &graph,
                    2,
                    FormulaLimits {
                        max_batch_bytes: limit,
                        ..Default::default()
                    },
                    &device(),
                    fresh,
                    1
                )
                .is_ok(),
                ok
            );
        }
    }
    let smaller = Plan::new(&graph, 1, FormulaLimits::default(), &device(), false, 1).unwrap();
    assert!(smaller.transport < hot.transport);
    assert!(
        Plan::new(
            &graph,
            2,
            FormulaLimits {
                max_candidates: 1,
                ..Default::default()
            },
            &device(),
            false,
            1
        )
        .is_err()
    );
    let work = graph.schedule.setup_work;
    assert!(
        Plan::new(
            &graph,
            2,
            FormulaLimits {
                max_work_per_candidate: work,
                ..Default::default()
            },
            &device(),
            false,
            1
        )
        .is_ok()
    );
    assert!(
        Plan::new(
            &graph,
            2,
            FormulaLimits {
                max_work_per_candidate: work - 1,
                ..Default::default()
            },
            &device(),
            false,
            1
        )
        .is_err()
    );
}
#[test]
fn device_limits_zero_atoms_and_word_boundaries_have_explicit_layouts() {
    for count in [0usize, 1, 31, 32, 33, 63, 64, 65, 4097] {
        let theory = Theory::new(
            count,
            zetesis_ferraris::FormulaParts::new(vec![], Vec::new()).unwrap(),
            vec![],
            AdmissionLimits::default(),
        )
        .unwrap();
        let prepared = prepared(&theory, &device()).unwrap();
        let graph = prepared.graph;
        let plan = Plan::new(&graph, 1, FormulaLimits::default(), &device(), true, 1).unwrap();
        let candidate = Interpretation::new(&theory, 0..count).unwrap();
        let packed = candidate_words(
            &plan,
            &graph,
            &[candidate],
            &zetesis_algebra::Cancellation::default(),
        )
        .unwrap();
        assert_eq!(packed.len(), count.div_ceil(32).max(1));
        if count > 0 {
            assert_eq!(
                packed[(count - 1) / 32],
                u32::MAX >> (31 - (count - 1) % 32)
            );
        }
        assert_eq!((prepared.nodes, prepared.roots), (vec![0; 4], vec![0]));
    }
    let mut small = device();
    small.max_storage_buffer_binding_size = 16;
    assert!(graph(&theory(), &small).is_err());
    let graph = graph(&theory(), &device()).unwrap();
    small = device();
    small.max_compute_workgroups_per_dimension = 1;
    assert!(Plan::new(&graph, 2, FormulaLimits::default(), &small, false, 1).is_err());
    small = device();
    small.max_uniform_buffer_binding_size = 63;
    assert!(Plan::new(&graph, 1, FormulaLimits::default(), &small, false, 1).is_err());
    small = device();
    small.max_buffer_size = 47;
    assert!(Plan::new(&graph, 2, FormulaLimits::default(), &small, false, 1).is_err());
    if usize::BITS > 32 {
        assert!(address(usize::MAX).is_err());
    }
    assert!(mul(u64::MAX, 2).is_err());
    assert!(sum(&[u64::MAX, 1]).is_err());
}
#[test]
fn result_records_validate_epoch_world_status_and_exact_charged_work() {
    let control = zetesis_algebra::Cancellation::default();
    let graph = graph(&theory(), &device()).unwrap();
    let plan = plan(&graph, FormulaLimits::default(), false);
    let valid = [7, 0, 1, 0, 26, MAGIC, 0, 0, 7, 1, 2, 1, 162, MAGIC, 0, 0];
    let checks = decode(&valid, &plan, &[3, 3], &control).unwrap();
    assert_eq!(checks[0].verdict(), &FormulaVerdict::NotModel);
    assert_eq!(checks[1].verdict(), &FormulaVerdict::NoProperSubset);
    assert_eq!(
        checks[1].statistics(),
        FormulaStatistics {
            work: 162,
            rounds: 1
        }
    );
    for (index, value) in [
        (0, 6),
        (1, 1),
        (2, 0),
        (3, 1),
        (4, 17),
        (5, 0),
        (10, 2),
        (11, 0),
        (12, 163),
    ] {
        let mut invalid = valid;
        invalid[index] = value;
        if invalid == valid {
            continue;
        }
        assert!(
            decode(&invalid, &plan, &[3, 3], &control).is_err(),
            "{index}={value}"
        );
    }
    assert!(decode(&valid[..15], &plan, &[3, 3], &control).is_err());
    for (limits, status, reason) in [
        (
            FormulaLimits {
                max_rounds: 0,
                ..Default::default()
            },
            4,
            ResidualReason::RoundLimit,
        ),
        (
            FormulaLimits {
                max_work_per_candidate: 28,
                ..Default::default()
            },
            5,
            ResidualReason::WorkLimit,
        ),
    ] {
        let bounded = Plan::new(&graph, 1, limits, &device(), false, 1).unwrap();
        let result = decode(
            &[1, 0, status, 0, 28, MAGIC, 0, 0],
            &bounded,
            &[3],
            &control,
        )
        .unwrap();
        assert_eq!(
            result[0].verdict(),
            &FormulaVerdict::Residual(reason, zetesis_algebra::region::Region::all_open(2))
        );
        assert!(decode(&[1, 0, 2, 0, 26, MAGIC, 0, 0], &bounded, &[3], &control).is_err());
    }
    let fixed = Plan::new(&graph, 1, FormulaLimits::default(), &device(), false, 1).unwrap();
    assert_eq!(
        decode(&[1, 0, 3, 1, 164, MAGIC, 0, 0], &fixed, &[3], &control).unwrap()[0].verdict(),
        &FormulaVerdict::Residual(
            ResidualReason::FixedPoint,
            zetesis_algebra::region::Region::all_open(2)
        )
    );
}

#[test]
fn summary_merge_work_is_required_before_a_complete_sweep() {
    let graph = graph(&theory(), &device()).unwrap();
    // Six nodes, two atoms, one root, three levels: setup=28, sweep=136.
    for (work, full_sweep) in [(163, false), (164, true)] {
        let plan = Plan::new(
            &graph,
            1,
            FormulaLimits {
                max_work_per_candidate: work,
                ..Default::default()
            },
            &device(),
            false,
            1,
        )
        .unwrap();
        assert_eq!(
            decode(
                &[1, 0, 2, 1, 162, MAGIC, 0, 0],
                &plan,
                &[3],
                &zetesis_algebra::Cancellation::default()
            )
            .is_ok(),
            full_sweep
        );
        assert_eq!(
            decode(
                &[1, 0, 5, 0, 28, MAGIC, 0, 0],
                &plan,
                &[3],
                &zetesis_algebra::Cancellation::default()
            )
            .is_ok(),
            !full_sweep
        );
    }
}

#[test]
fn level_visits_cannot_overflow_sweep_work() {
    let mut graph = graph(&theory(), &device()).unwrap();
    graph.shape.sweep_work = u32::MAX;
    assert!(Plan::new(&graph, 1, FormulaLimits::default(), &device(), false, 1).is_err());
}

#[test]
fn dense_outputs_preserve_leaf_aliases_and_original_children() {
    let theory = Theory::new(
        2,
        zetesis_ferraris::FormulaParts::new(
            vec![
                Node::atom(1),
                Node::falsum(),
                Node::atom(0),
                Node::atom(1),
                Node::and_pair([0, 2]),
                Node::or_pair([1, 4]),
                Node::implies(5, 3),
            ],
            Vec::new(),
        )
        .unwrap(),
        vec![6],
        AdmissionLimits::default(),
    )
    .unwrap();
    let prepared = prepared(&theory, &device()).unwrap();
    let graph = prepared.graph;
    assert_eq!(graph.shape.variables, 6); // 2 semantic atoms + 4 non-Atom nodes.
    let (nodes, roots) = (prepared.nodes, prepared.roots);
    assert_eq!(
        nodes,
        [
            1, 1, 0, 1, 0, 0, 0, 2, 1, 0, 0, 0, 1, 1, 0, 1, 2, 0, 2, 3, 3, 1, 4, 4, 4, 5, 3, 5,
        ]
    );
    assert_eq!(roots, [6, 0, 4, 5, 6, 7, 0, 1, 2, 3, 4, 5, 6]);
    // Each original node is still visited twice during setup. Omitting leaf
    // stores does not omit the visit or duplicate semantic-atom initialization.
    assert_eq!(
        (graph.schedule.setup_work, graph.shape.sweep_work),
        (31, 142)
    );
}

#[test]
fn leaf_only_domains_admit_the_exact_reduced_buffer() {
    let theory = Theory::new(
        100,
        zetesis_ferraris::FormulaParts::new(
            vec![Node::atom(0), Node::atom(99), Node::atom(0), Node::atom(51)],
            Vec::new(),
        )
        .unwrap(),
        vec![],
        AdmissionLimits::default(),
    )
    .unwrap();
    let graph = graph(&theory, &device()).unwrap();
    assert_eq!(graph.shape.variables, 100);
    let exact = 4 * 100 * 3; // Three worlds, one word per semantic atom only.
    for (ceiling, accepted) in [(exact, true), (exact - 1, false)] {
        let mut limits = device();
        limits.max_storage_buffer_binding_size = ceiling;
        let outcome = Plan::new(&graph, 3, FormulaLimits::default(), &limits, true, 1);
        assert_eq!(outcome.is_ok(), accepted);
        if let Ok(plan) = outcome {
            assert_eq!(plan.domains, exact);
            assert_eq!(4 * (100 + 4) * 3 - plan.domains, 48);
        }
    }
}

#[test]
fn empty_domains_keep_only_the_required_storage_padding() {
    let theory = Theory::new(
        0,
        zetesis_ferraris::FormulaParts::new(vec![], Vec::new()).unwrap(),
        vec![],
        AdmissionLimits::default(),
    )
    .unwrap();
    let prepared = prepared(&theory, &device()).unwrap();
    let graph = prepared.graph;
    assert_eq!(graph.shape.variables, 0);
    let plan = Plan::new(&graph, 3, FormulaLimits::default(), &device(), true, 1).unwrap();
    assert_eq!(plan.domains, 4);
    assert_eq!((prepared.nodes, prepared.roots), (vec![0; 4], vec![0]));
}

#[test]
fn prefix_admission_selects_the_complete_layout_boundary() {
    let device = device();
    let prepared = prepared(&theory(), &device).unwrap();
    for fresh in [false, true] {
        let layout = |count, limits| {
            if fresh {
                prepared.plan(count, limits, &device, 1)
            } else {
                Plan::new(&prepared.graph, count, limits, &device, false, 1)
            }
        };
        for maximum in 1..=8 {
            let bytes = layout(maximum, FormulaLimits::default()).unwrap().accounted;
            let limits = FormulaLimits {
                max_batch_bytes: bytes,
                ..Default::default()
            };
            let selected = crate::candidates::BatchSize::Prefix(9)
                .select(
                    limits.max_candidates,
                    &zetesis_algebra::Cancellation::default(),
                    |count| layout(count, limits),
                )
                .unwrap();
            assert_eq!(selected.worlds as usize, maximum);
            assert!(selected.accounted <= bytes);
            assert!(layout(maximum + 1, limits).is_err());
        }
        let bytes = layout(1, FormulaLimits::default()).unwrap().accounted;
        assert!(
            crate::candidates::BatchSize::Prefix(9)
                .select(
                    9,
                    &zetesis_algebra::Cancellation::default(),
                    |count| layout(
                        count,
                        FormulaLimits {
                            max_batch_bytes: bytes - 1,
                            ..Default::default()
                        }
                    )
                )
                .is_err()
        );
    }
}

#[test]
fn prefix_admission_includes_the_timestamp_readback_tail() {
    let mut device = device();
    let graph = graph(&theory(), &device).unwrap();
    // Six complete rows fit exactly, each with six metadata and two atom-mask words.
    let six_rows = 6 * (6 + 2) * 4;
    device.max_buffer_size = six_rows;
    device.max_storage_buffer_binding_size = six_rows;
    for (timed, expected) in [(false, 6), (true, 5)] {
        let selected = crate::candidates::BatchSize::Prefix(20)
            .select(20, &zetesis_algebra::Cancellation::default(), |count| {
                let plan = Plan::new(&graph, count, FormulaLimits::default(), &device, false, 1)?;
                crate::timing::readback_size(plan.results, timed, device.max_buffer_size)?;
                Ok(plan)
            })
            .unwrap();
        assert_eq!(selected.worlds, expected);
    }
}

#[test]
fn residual_geometry_is_bound_to_the_exact_candidate_and_terminal() {
    let graph = graph(&theory(), &device()).unwrap();
    let plan = plan(
        &graph,
        FormulaLimits {
            max_rounds: 0,
            ..FormulaLimits::default()
        },
        false,
    );
    let seeds = [0b01, 0b10];
    let row = [
        7, 0, 4, 0, 28, MAGIC, 0, 0b10, 7, 1, 4, 0, 28, MAGIC, 0b10, 0b01,
    ];
    let good = decode(
        &row,
        &plan,
        &seeds,
        &zetesis_algebra::Cancellation::default(),
    )
    .unwrap();
    let FormulaVerdict::Residual(ResidualReason::RoundLimit, r) = good[1].verdict() else {
        panic!("expected handed-over root")
    };
    assert_eq!(r.decision(0), Some(false));
    assert_eq!(r.decision(1), Some(true));
    for (i, v) in [
        (6, 0b10),  // held outside M and both decisions on one atom
        (7, 0),     // outside atom left open
        (6, 4),     // held padding
        (7, 6),     // cut padding
        (9, 0),     // duplicate local slot
        (14, 0b11), // contradiction on the second world's outside atom
    ] {
        let mut bad = row;
        bad[i] = v;
        assert_eq!(
            decode(
                &bad,
                &plan,
                &seeds,
                &zetesis_algebra::Cancellation::default()
            )
            .unwrap_err()
            .kind(),
            GpuErrorKind::Readback
        );
    }
    let mut stale_terminal = row;
    stale_terminal[2] = 1;
    stale_terminal[4] = plan.setup - plan.atoms;
    assert_eq!(
        decode(
            &stale_terminal,
            &plan,
            &seeds,
            &zetesis_algebra::Cancellation::default()
        )
        .unwrap_err()
        .kind(),
        GpuErrorKind::Readback
    );
    assert_eq!(
        decode(
            &row,
            &plan,
            &[seeds[1], seeds[0]],
            &zetesis_algebra::Cancellation::default()
        )
        .unwrap_err()
        .kind(),
        GpuErrorKind::Readback
    );
    assert_eq!(
        decode(
            &row[..row.len() - 1],
            &plan,
            &seeds,
            &zetesis_algebra::Cancellation::default()
        )
        .unwrap_err()
        .kind(),
        GpuErrorKind::Readback
    );
}

#[test]
fn projected_polarities_survive_both_wire_and_host_word_boundaries() {
    let theory = Theory::new(
        66,
        zetesis_ferraris::FormulaParts::default(),
        vec![],
        AdmissionLimits::default(),
    )
    .unwrap();
    let graph = graph(&theory, &device()).unwrap();
    let plan = Plan::new(
        &graph,
        1,
        FormulaLimits {
            max_rounds: 0,
            ..Default::default()
        },
        &device(),
        false,
        17,
    )
    .unwrap();
    let seeds = [1 << 31, 0x8000_0003, 3];
    let yes = [1 << 31, 1, 1];
    let no = [0x7fff_ffff, 0x7fff_fffe, 0];
    let mut words = vec![17, 0, 4, 0, plan.setup, MAGIC];
    words.extend(yes);
    words.extend(no);
    let checks = decode(
        &words,
        &plan,
        &seeds,
        &zetesis_algebra::Cancellation::default(),
    )
    .unwrap();
    let FormulaVerdict::Residual(_, root) = checks[0].verdict() else {
        panic!("residual")
    };
    assert_eq!(root.len(), 66);
    assert!(root.is_held(31) && root.is_held(32) && root.is_held(64));
    assert!(root.is_cut(30) && root.is_cut(33) && root.is_open(63) && root.is_open(65));
    words[6 + 2] |= 1 << 2;
    assert_eq!(
        decode(
            &words,
            &plan,
            &seeds,
            &zetesis_algebra::Cancellation::default()
        )
        .unwrap_err()
        .kind(),
        GpuErrorKind::Readback
    );
}

#[test]
fn a_host_polarity_swap_is_a_readback_error_even_for_a_full_candidate() {
    let mut swapped = zetesis_algebra::region::Region::all_open(2);
    assert!(swapped.cut(0) && swapped.hold(1));
    assert_eq!(
        verify_materialized(
            &swapped,
            &[1],
            &[2],
            &zetesis_algebra::Cancellation::default()
        )
        .unwrap_err()
        .kind(),
        GpuErrorKind::Readback
    );
}

#[test]
fn residual_decode_cannot_borrow_one_extra_byte_reserved_for_the_next_slate() {
    use zetesis_algebra::{Stop, region::Region};

    let graph = graph(&theory(), &device()).unwrap();
    let outer = FormulaLimits::default();
    let plan = plan(&graph, outer, true);
    let output = Output::reserve(&plan).unwrap();
    // One prospective mask pair and its temporary headers belong to this
    // dispatch. The larger outer limit may already cover an admitted lookahead.
    let mask_bytes = 2 * size_of::<u64>() * (plan.atoms as usize).div_ceil(64);
    let stage = u64::try_from(size_of::<Region>() + size_of::<Vec<u64>>() + mask_bytes).unwrap();
    let other = output.accounted.checked_sub(stage).unwrap();
    assert!(outer.max_batch_bytes > output.accounted);
    assert_eq!(
        admit_decode_storage(other, u128::from(stage), output.accounted),
        Ok(())
    );
    assert_eq!(
        admit_decode_storage(other, u128::from(stage) + 1, output.accounted),
        Err(Stop::StorageLimit)
    );
}
