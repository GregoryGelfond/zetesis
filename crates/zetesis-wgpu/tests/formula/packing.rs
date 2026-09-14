use super::super::preparation::{Preparation, PreparedGraph};
use super::*;
use zetesis_ferraris::AdmissionLimits;

fn prepared(theory: &Theory, device: &wgpu::Limits) -> Result<PreparedGraph, GpuError> {
    Preparation::new(theory, 2, FormulaLimits::default(), device, 7)?
        .finish(device, &zetesis_cpu::Control::default())
        .map(|(prepared, _)| prepared)
}
fn graph(theory: &Theory, device: &wgpu::Limits) -> Result<Graph, GpuError> {
    prepared(theory, device).map(|prepared| prepared.graph)
}

fn theory() -> Theory {
    Theory::new(
        2,
        vec![
            Node::False,
            Node::Atom(0),
            Node::Atom(1),
            Node::And(1, 2),
            Node::Or(1, 2),
            Node::Implies(3, 4),
        ],
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
        (18, 121)
    );
    let limits = FormulaLimits::default();
    let plan = plan(&graph, limits, true);
    assert_eq!(
        plan.params(&graph),
        [2, 6, 1, 6, 1, 2, 64, 100_000_000, 18, 121, 7, 3]
    );
    let a = Interpretation::new(&graph.shape.theory, [0]).unwrap();
    let b = Interpretation::new(&graph.shape.theory, [1]).unwrap();
    assert_eq!(plan.pack(&graph, &[a, b]).unwrap(), vec![1, 2]);
    let foreign = Interpretation::new(&theory(), []).unwrap();
    assert_eq!(
        plan.pack(&graph, &[foreign.clone(), foreign])
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
        let theory = Theory::new(count, vec![], vec![], AdmissionLimits::default()).unwrap();
        let prepared = prepared(&theory, &device()).unwrap();
        let graph = prepared.graph;
        let plan = Plan::new(&graph, 1, FormulaLimits::default(), &device(), true, 1).unwrap();
        let candidate = Interpretation::new(&theory, 0..count).unwrap();
        let packed = plan.pack(&graph, &[candidate]).unwrap();
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
    small.max_uniform_buffer_binding_size = 47;
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
    let graph = graph(&theory(), &device()).unwrap();
    let plan = plan(&graph, FormulaLimits::default(), false);
    let valid = [7, 0, 1, 0, 18, MAGIC, 7, 1, 2, 1, 139, MAGIC];
    let checks = decode(&valid, &plan).unwrap();
    assert_eq!(checks[0].verdict(), FormulaVerdict::NotModel);
    assert_eq!(checks[1].verdict(), FormulaVerdict::NoProperSubset);
    assert_eq!(
        checks[1].statistics(),
        FormulaStatistics {
            work: 139,
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
        (8, 2),
        (9, 0),
        (10, 138),
    ] {
        let mut invalid = valid;
        invalid[index] = value;
        if invalid == valid {
            continue;
        }
        assert!(decode(&invalid, &plan).is_err(), "{index}={value}");
    }
    assert!(decode(&valid[..11], &plan).is_err());
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
                max_work_per_candidate: 18,
                ..Default::default()
            },
            5,
            ResidualReason::WorkLimit,
        ),
    ] {
        let bounded = Plan::new(&graph, 1, limits, &device(), false, 1).unwrap();
        let result = decode(&[1, 0, status, 0, 18, MAGIC], &bounded).unwrap();
        assert_eq!(result[0].verdict(), FormulaVerdict::Residual(reason));
        assert!(decode(&[1, 0, 2, 0, 18, MAGIC], &bounded).is_err());
    }
    let fixed = Plan::new(&graph, 1, FormulaLimits::default(), &device(), false, 1).unwrap();
    assert_eq!(
        decode(&[1, 0, 3, 1, 139, MAGIC], &fixed).unwrap()[0].verdict(),
        FormulaVerdict::Residual(ResidualReason::FixedPoint)
    );
}

#[test]
fn summary_merge_work_is_required_before_a_complete_sweep() {
    let graph = graph(&theory(), &device()).unwrap();
    // Six nodes, two atoms, one root, three levels: setup=18, sweep=121.
    for (work, full_sweep) in [(138, false), (139, true)] {
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
        assert_eq!(decode(&[1, 0, 2, 1, 139, MAGIC], &plan).is_ok(), full_sweep);
        assert_eq!(decode(&[1, 0, 5, 0, 18, MAGIC], &plan).is_ok(), !full_sweep);
    }
}

#[test]
fn dense_outputs_preserve_leaf_aliases_and_original_children() {
    let theory = Theory::new(
        2,
        vec![
            Node::Atom(1),
            Node::False,
            Node::Atom(0),
            Node::Atom(1),
            Node::And(0, 2),
            Node::Or(1, 4),
            Node::Implies(5, 3),
        ],
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
        (21, 130)
    );
}

#[test]
fn leaf_only_domains_admit_the_exact_reduced_buffer() {
    let theory = Theory::new(
        100,
        vec![Node::Atom(0), Node::Atom(99), Node::Atom(0), Node::Atom(51)],
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
    let theory = Theory::new(0, vec![], vec![], AdmissionLimits::default()).unwrap();
    let prepared = prepared(&theory, &device()).unwrap();
    let graph = prepared.graph;
    assert_eq!(graph.shape.variables, 0);
    let plan = Plan::new(&graph, 3, FormulaLimits::default(), &device(), true, 1).unwrap();
    assert_eq!(plan.domains, 4);
    assert_eq!((prepared.nodes, prepared.roots), (vec![0; 4], vec![0]));
}
