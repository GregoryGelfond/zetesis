//! Explicit physical API qualification of the partial frozen-query primitive.
//! No adapter absence or residual query is replaced by a CPU GPU-result claim.

#[path = "support/physical.rs"]
mod physical;

use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AdmissionLimits, FrozenReduct, Interpretation, Limits, Node, Theory, Verdict, check,
};
use zetesis_wgpu::{
    FormulaLimits, FormulaVerdict, GateProjection, GpuErrorKind, GpuFormulaOracle, GpuOptions,
    ResidualReason,
};

fn oracle(backend: physical::Backend, projection: GateProjection) -> GpuFormulaOracle {
    let oracle = GpuFormulaOracle::new_selected_with_projection(
        GpuOptions::default(),
        backend.selection(),
        projection,
    )
    .unwrap();
    assert_eq!(oracle.projection(), projection);
    backend.verify(oracle.info());
    oracle
}

fn theory(atoms: usize, nodes: Vec<Node>, roots: Vec<usize>) -> Theory {
    Theory::new(atoms, nodes, roots, AdmissionLimits::default()).unwrap()
}
fn candidates(theory: &Theory) -> Vec<Interpretation> {
    assert!(theory.atom_count() <= 3, "exhaustive fixture is bounded");
    (0..1usize << theory.atom_count())
        .map(|bits| {
            Interpretation::new(
                theory,
                (0..theory.atom_count()).filter(|a| bits & (1 << a) != 0),
            )
            .unwrap()
        })
        .collect()
}

fn setup_work(theory: &Theory) -> u32 {
    // Independent dependency depths over the original nodes; no production
    // schedule or packed output positions are consulted for this work receipt.
    let mut depths = Vec::<usize>::new();
    for node in theory.nodes() {
        depths.push(match *node {
            Node::False | Node::Atom(_) => 0,
            Node::And(a, b) | Node::Or(a, b) | Node::Implies(a, b) => 1 + depths[a].max(depths[b]),
        });
    }
    let levels = depths.iter().max().map_or(0, |depth| depth + 1);
    let barriers = if levels == depths.len() { 0 } else { levels };
    u32::try_from(2 * depths.len() + theory.atom_count() + theory.roots().len() + barriers).unwrap()
}

fn compare(oracle: &mut GpuFormulaOracle, theory: &Theory) -> (usize, usize) {
    let inputs = candidates(theory);
    let checks = oracle
        .propagate_batch(theory, &inputs, FormulaLimits::default())
        .unwrap();
    let cold = *oracle.last_batch_stats().unwrap();
    assert!(cold.theory_uploaded && cold.transport_allocated);
    let mut refutations = 0;
    let mut residuals = 0;
    for (candidate, result) in inputs.iter().zip(&checks) {
        let cpu = check(
            theory,
            candidate,
            Limits {
                max_subsets: 8,
                ..Limits::default()
            },
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(
            matches!(result.verdict(), FormulaVerdict::NotModel),
            matches!(cpu.verdict(), Verdict::NotModel { .. })
        );
        match result.verdict() {
            FormulaVerdict::NoProperSubset => {
                assert!(cpu.accepted());
                refutations += 1;
            }
            FormulaVerdict::Residual(_) => {
                assert!(!matches!(cpu.verdict(), Verdict::NotModel { .. }));
                residuals += 1;
            }
            FormulaVerdict::NotModel => {}
        }
        let setup = setup_work(theory);
        let sweep = u32::try_from(9 * theory.nodes().len() + theory.atom_count() + 65).unwrap();
        assert_eq!(
            result.statistics().work,
            setup + result.statistics().rounds * sweep
        );
    }
    let reversed: Vec<_> = inputs.into_iter().rev().collect();
    let again = oracle
        .propagate_batch(&theory.clone(), &reversed, FormulaLimits::default())
        .unwrap();
    // Scheduling can change the number of sweeps, but completed logical results
    // and quiescent residuals are independent of candidate lane order.
    assert!(
        checks
            .iter()
            .rev()
            .map(zetesis_wgpu::FormulaCheck::verdict)
            .eq(again.iter().map(zetesis_wgpu::FormulaCheck::verdict))
    );
    let hot = *oracle.last_batch_stats().unwrap();
    assert!(!hot.theory_uploaded && !hot.transport_allocated);
    // Cold admission includes actual retained host upload capacity. Legal
    // allocator slack can exceed the requested device graph size; exact actual
    // capacities are independently checked through the pure allocator seam.
    assert!(cold.accounted_bytes - hot.accounted_bytes >= hot.resident_theory_bytes);
    (refutations, residuals)
}

#[test]
#[ignore = "requires an actual Metal GPU; explicit hardware qualification only"]
fn metal_formula_queries_preserve_exact_frozen_semantics_and_residency() {
    for projection in GateProjection::ALL {
        qualify_frozen_queries(physical::Backend::Metal, projection);
    }
}

fn frozen_theories() -> [Theory; 9] {
    [
        theory(0, vec![], vec![]),
        theory(0, vec![Node::False], vec![0]),
        theory(1, vec![Node::Atom(0)], vec![0]),
        // Duplicate leaves retain one semantic slot even when non-Atom slots
        // interleave them. The original child references remain DAG indices.
        theory(
            2,
            vec![
                Node::Atom(0),
                Node::False,
                Node::Atom(0),
                Node::Atom(1),
                Node::And(0, 2),
                Node::Or(4, 3),
            ],
            vec![5],
        ),
        // In (not a)->a, M={a} satisfies the original formula, but empty J
        // satisfies the frozen reduct false->a. The false negation's original
        // connective must never force a while its dense output is false.
        theory(
            1,
            vec![
                Node::False,
                Node::Atom(0),
                Node::Implies(1, 0),
                Node::Implies(2, 1),
            ],
            vec![3],
        ),
        // Bare double negation: {p} is a model, but its reduct permits empty J.
        theory(
            1,
            vec![
                Node::False,
                Node::Atom(0),
                Node::Implies(1, 0),
                Node::Implies(2, 0),
            ],
            vec![3],
        ),
        // Choice p or not p retains both stable models.
        theory(
            1,
            vec![
                Node::False,
                Node::Atom(0),
                Node::Implies(1, 0),
                Node::Or(1, 2),
            ],
            vec![3],
        ),
        // Positive cycle admits only empty; disjunctive support admits both atoms.
        theory(
            2,
            vec![
                Node::Atom(0),
                Node::Atom(1),
                Node::Implies(0, 1),
                Node::Implies(1, 0),
            ],
            vec![2, 3],
        ),
        theory(
            2,
            vec![
                Node::Atom(0),
                Node::Atom(1),
                Node::Implies(0, 1),
                Node::Implies(1, 0),
                Node::Or(0, 1),
            ],
            vec![2, 3, 4],
        ),
    ]
}

fn qualify_frozen_queries(backend: physical::Backend, projection: GateProjection) {
    let mut oracle = oracle(backend, projection);
    println!(
        "formula projection={} adapter={}",
        projection.label(),
        oracle.info().name()
    );
    let mut totals = (0, 0);
    for graph in frozen_theories() {
        let count = compare(&mut oracle, &graph);
        totals.0 += count.0;
        totals.1 += count.1;
    }
    for graph in scheduled_theories() {
        let count = compare(&mut oracle, &graph);
        totals.0 += count.0;
        totals.1 += count.1;
    }
    // Every connective, repeated children and nested/default-negated children;
    // the third semantic atom is deliberately absent from every formula.
    for operator in 0..3 {
        for left in 0..4 {
            for right in 0..4 {
                let mut nodes = vec![
                    Node::False,
                    Node::Atom(0),
                    Node::Atom(1),
                    Node::Implies(1, 0),
                ];
                nodes.push(match operator {
                    0 => Node::And(left, right),
                    1 => Node::Or(left, right),
                    _ => Node::Implies(left, right),
                });
                nodes.push(Node::Implies(4, 1));
                let graph = theory(3, nodes, vec![5]);
                let count = compare(&mut oracle, &graph);
                totals.0 += count.0;
                totals.1 += count.1;
            }
        }
    }
    assert!(totals.0 > 0 && totals.1 > 0);
    println!("completed refutations={} residuals={}", totals.0, totals.1);
    qualify_subset_strides(&mut oracle);
}

fn scheduled_theories() -> [Theory; 2] {
    let mut chain = vec![Node::Atom(0)];
    chain.extend((1..128).map(|index| Node::And(index - 1, index - 1)));
    [
        theory(1, chain, vec![127]),
        theory(
            2,
            vec![
                Node::Atom(0),
                Node::And(0, 0),
                Node::Atom(1),
                Node::Or(1, 2),
                Node::False,
                Node::Implies(3, 4),
                Node::Atom(0),
                Node::And(2, 6),
            ],
            // a-or-b gives original models, exact refutations and a residual;
            // the unused deeper nodes still exercise scheduled frozen truth.
            vec![3],
        ),
    ]
}

fn qualify_subset_strides(oracle: &mut GpuFormulaOracle) {
    // Three strides, including a partial final stride, distinguish semantic
    // positions 0, 64 and 130. Duplicate leaves and False are not extra atoms.
    for available in [
        vec![],
        vec![0],
        vec![64],
        vec![130],
        vec![0, 64],
        vec![64, 130],
        vec![0, 64, 130],
        (0..131).collect(),
    ] {
        let mut nodes = vec![Node::False];
        nodes.extend((0..131).map(Node::Atom));
        nodes.push(Node::Atom(130));
        let roots = (0..131)
            .filter(|atom| !available.contains(atom))
            .map(|atom| atom + 1)
            .collect();
        let graph = theory(131, nodes, roots);
        let candidate = Interpretation::new(&graph, 0..131).unwrap();
        if let Some(omitted) = available.first() {
            // An independently evaluated proper-subset witness satisfies every
            // asserted fact. No enumeration of 2^131 subsets is needed.
            let witness = Interpretation::new(&graph, (0..131).filter(|a| a != omitted)).unwrap();
            let reduct =
                FrozenReduct::new(&candidate, Limits::default(), &Cancellation::default()).unwrap();
            assert!(
                reduct
                    .is_satisfied_by(&witness, Limits::default(), &Cancellation::default())
                    .unwrap()
            );
        }
        // Authored constants: 133 nodes, 131 atoms, 131-|available| facts.
        // Setup visits nodes twice, atoms once, roots once and one level. Each sweep
        // reserves 9 per node, one per atom, 64 summaries and one application.
        let setup = 529 - u32::try_from(available.len()).unwrap();
        let sweep = 1393;
        let expected = match available.len() {
            0 => FormulaVerdict::NoProperSubset,
            1 => FormulaVerdict::Residual(ResidualReason::RoundLimit),
            _ => FormulaVerdict::Residual(ResidualReason::FixedPoint),
        };
        for (work, verdict, rounds) in [
            (
                setup + sweep - 1,
                FormulaVerdict::Residual(ResidualReason::WorkLimit),
                0,
            ),
            (setup + sweep, expected, 1),
        ] {
            let results = oracle
                .propagate_batch(
                    &graph,
                    std::slice::from_ref(&candidate),
                    FormulaLimits {
                        max_rounds: 1,
                        max_work_per_candidate: work,
                        ..FormulaLimits::default()
                    },
                )
                .unwrap();
            assert_eq!(results[0].verdict(), verdict, "available={available:?}");
            assert_eq!(results[0].statistics().rounds, rounds);
            assert_eq!(results[0].statistics().work, setup + rounds * sweep);
        }
    }
    println!("strict-subset strides=3 final-stride=3 zero/unique/multiple=8 exact/below=16");
}

#[test]
#[ignore = "requires an actual Metal GPU; explicit hardware qualification only"]
fn metal_formula_limits_resize_identity_and_word_boundaries_remain_explicit() {
    for projection in GateProjection::ALL {
        qualify_resources(physical::Backend::Metal, projection);
    }
}

fn qualify_resources(backend: physical::Backend, projection: GateProjection) {
    let mut oracle = oracle(backend, projection);
    let graph = theory(1, vec![Node::Atom(0)], vec![0]);
    let inputs = candidates(&graph);
    for (limits, expected) in [
        (
            FormulaLimits {
                max_rounds: 0,
                ..FormulaLimits::default()
            },
            ResidualReason::RoundLimit,
        ),
        (
            FormulaLimits {
                max_work_per_candidate: 4,
                ..FormulaLimits::default()
            },
            ResidualReason::WorkLimit,
        ),
    ] {
        let results = oracle.propagate_batch(&graph, &inputs, limits).unwrap();
        assert_eq!(results[0].verdict(), FormulaVerdict::NotModel);
        assert_eq!(results[1].verdict(), FormulaVerdict::Residual(expected));
        assert_eq!(results[1].statistics().rounds, 0);
        assert_eq!(results[1].statistics().work, 4);
    }
    let full = oracle
        .propagate_batch(&graph, &inputs, FormulaLimits::default())
        .unwrap();
    assert_eq!(full[1].verdict(), FormulaVerdict::NoProperSubset);
    let hot = *oracle.last_batch_stats().unwrap();
    assert_zero_wait_preserves_residency(&mut oracle, &graph, &inputs, &full);
    for (bytes, succeeds) in [
        (hot.accounted_bytes - 1, false),
        (hot.accounted_bytes, true),
    ] {
        let result = oracle.propagate_batch(
            &graph,
            &inputs,
            FormulaLimits {
                max_batch_bytes: bytes,
                ..FormulaLimits::default()
            },
        );
        assert_eq!(result.is_ok(), succeeds);
        if let Ok(result) = result {
            assert_eq!(result, full);
        } else {
            assert!(oracle.last_batch_stats().is_none());
        }
    }
    let resized = oracle
        .propagate_batch(&graph, &inputs[..1], FormulaLimits::default())
        .unwrap();
    assert_eq!(resized[0], full[0]);
    let small = *oracle.last_batch_stats().unwrap();
    assert!(!small.theory_uploaded && small.transport_allocated);
    assert!(small.resident_transport_bytes < hot.resident_transport_bytes);
    let equal = theory(1, vec![Node::Atom(0)], vec![0]);
    let foreign = Interpretation::new(&equal, [0]).unwrap();
    assert_eq!(
        oracle
            .propagate_batch(&graph, &[foreign], FormulaLimits::default())
            .unwrap_err()
            .kind(),
        GpuErrorKind::Seed
    );
    assert!(oracle.last_batch_stats().is_none());
    oracle
        .propagate_batch(&equal, &candidates(&equal), FormulaLimits::default())
        .unwrap();
    assert!(oracle.last_batch_stats().unwrap().theory_uploaded);
    for count in [31, 32, 33, 63, 64, 65, 4097] {
        let graph = theory(count, vec![Node::Atom(count - 1)], vec![0]);
        let candidate = Interpretation::new(&graph, [count - 1]).unwrap();
        let result = oracle
            .propagate_batch(&graph, &[candidate], FormulaLimits::default())
            .unwrap();
        assert_eq!(result[0].verdict(), FormulaVerdict::NoProperSubset);
    }
    assert!(
        oracle
            .propagate_batch(
                &graph,
                &[],
                FormulaLimits {
                    max_work_per_candidate: 0,
                    max_batch_bytes: 0,
                    timeout: std::time::Duration::ZERO,
                    ..FormulaLimits::default()
                }
            )
            .unwrap()
            .is_empty()
    );
    assert!(oracle.last_batch_stats().is_none());
    assert_clears_residency(&mut oracle, &graph, &inputs);
}

fn assert_zero_wait_preserves_residency(
    oracle: &mut GpuFormulaOracle,
    graph: &Theory,
    inputs: &[Interpretation],
    expected: &[zetesis_wgpu::FormulaCheck],
) {
    let zero_wait = FormulaLimits {
        timeout: std::time::Duration::ZERO,
        ..FormulaLimits::default()
    };
    assert_eq!(
        oracle
            .propagate_batch(graph, inputs, zero_wait)
            .unwrap_err()
            .kind(),
        GpuErrorKind::Capacity
    );
    assert!(oracle.last_batch_stats().is_none());
    assert!(oracle.last_submission_candidates().is_none());
    let reused = oracle
        .propagate_batch(graph, inputs, FormulaLimits::default())
        .unwrap();
    assert_eq!(reused.as_slice(), expected);
    let stats = oracle.last_batch_stats().unwrap();
    assert!(!stats.theory_uploaded && !stats.transport_allocated);
}

fn assert_clears_residency(
    oracle: &mut GpuFormulaOracle,
    graph: &Theory,
    inputs: &[Interpretation],
) {
    // Reestablish this exact theory and transport before clearing. Otherwise
    // the last boundary graph above forces an upload even for a no-op clear.
    oracle
        .propagate_batch(graph, inputs, FormulaLimits::default())
        .unwrap();
    oracle
        .propagate_batch(graph, inputs, FormulaLimits::default())
        .unwrap();
    let warm = oracle.last_batch_stats().unwrap();
    assert!(!warm.theory_uploaded && !warm.transport_allocated);
    oracle.clear_residency();
    oracle
        .propagate_batch(graph, inputs, FormulaLimits::default())
        .unwrap();
    let cleared = oracle.last_batch_stats().unwrap();
    assert!(cleared.theory_uploaded && cleared.transport_allocated);
}

#[test]
#[ignore = "requires an actual Vulkan GPU; explicit physical qualification"]
fn vulkan_formula_queries_preserve_frozen_semantics() {
    for projection in GateProjection::ALL {
        qualify_frozen_queries(physical::Backend::Vulkan, projection);
    }
}

#[test]
#[ignore = "requires an actual Vulkan GPU; explicit physical qualification"]
fn vulkan_formula_resource_boundaries_remain_explicit() {
    for projection in GateProjection::ALL {
        qualify_resources(physical::Backend::Vulkan, projection);
    }
}
