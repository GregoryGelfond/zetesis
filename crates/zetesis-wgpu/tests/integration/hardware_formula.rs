//! Explicit physical API qualification of the partial frozen-query primitive.
//! No adapter absence or residual query is replaced by a CPU GPU-result claim.

mod native;

use crate::support::physical;

use zetesis_backend::GpuApi;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    FrozenReduct, Interpretation, Limits, Node, NodeView, Theory, Verdict, check,
};
use zetesis_theory_support::theories::theory;
use zetesis_wgpu::{
    FormulaLimits, FormulaVerdict, GateProjection, GpuErrorKind, GpuFormulaOracle,
    GpuFormulaProfile, GpuKernelOptions, GpuOptions, GpuPropagation, GpuSelection,
    GpuWorkgroupSize, ResidualReason,
};

fn oracle(backend: GpuApi, projection: GateProjection) -> GpuFormulaOracle {
    let oracle = GpuFormulaOracle::new_selected_with_projection(
        GpuOptions::default(),
        GpuSelection { api: backend },
        projection,
    )
    .unwrap();
    assert_eq!(oracle.projection(), projection);
    physical::verify(backend, oracle.info());
    oracle
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

fn scheduled_levels(theory: &Theory) -> usize {
    // Independent dependency depths over the original nodes; no production
    // schedule or packed output positions are consulted for this work receipt.
    let mut depths = Vec::<usize>::new();
    for index in 0..theory.view().len() {
        depths.push(match theory.view().node(index).unwrap() {
            NodeView::False | NodeView::Atom(_) => 0,
            NodeView::Threshold(_) | NodeView::ExtremumCut(_) => {
                panic!("work-receipt helper requires Boolean fixtures")
            }
            NodeView::Implies(a, b) => 1 + depths[a].max(depths[b]),
            NodeView::And(children) | NodeView::Or(children) => {
                1 + children.iter().map(|&child| depths[child]).max().unwrap()
            }
        });
    }
    let levels = depths.iter().max().map_or(0, |depth| depth + 1);
    if levels == depths.len() { 0 } else { levels }
}

fn setup_work(theory: &Theory) -> u32 {
    u32::try_from(
        2 * theory.view().len()
            + theory.parts().occurrences()
            + theory.atom_count()
            + theory.roots().len()
            + scheduled_levels(theory),
    )
    .unwrap()
}

fn sweep_work(theory: &Theory) -> u32 {
    u32::try_from(
        9 * theory.nodes().len()
            + 2 * theory.parts().occurrences()
            + theory.atom_count()
            + 65
            + scheduled_levels(theory),
    )
    .unwrap()
}

fn propagation_work(theory: &Theory, propagation: GpuPropagation) -> (u32, u32) {
    let mut setup = setup_work(theory);
    let mut sweep = sweep_work(theory);
    if propagation == GpuPropagation::ChangedDependencies {
        let mut variables = theory.atom_count();
        let mut incidences = 0;
        for index in 0..theory.view().len() {
            match theory.view().node(index).unwrap() {
                NodeView::Atom(_) => {}
                NodeView::False => variables += 1,
                NodeView::Implies(..) => {
                    variables += 1;
                    incidences += 3;
                }
                NodeView::And(children) | NodeView::Or(children) => {
                    variables += 1;
                    incidences += children.len() + 1;
                }
                _ => panic!("work-receipt helper requires Boolean fixtures"),
            }
        }
        let nodes = theory.view().len();
        let words = nodes.div_ceil(32);
        setup += u32::try_from(2 * words).unwrap();
        sweep += u32::try_from(words + nodes + 4 * variables + 6 * incidences).unwrap();
    }
    (setup, sweep)
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
        let (setup, sweep) = propagation_work(theory, cold.propagation);
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
#[ignore = "requires Metal: formula queries preserve exact frozen semantics and residency"]
fn metal_formula_queries_preserve_exact_frozen_semantics_and_residency() {
    for projection in GateProjection::ALL {
        qualify_frozen_queries(GpuApi::Metal, projection);
    }
}

fn frozen_theories() -> Vec<Theory> {
    let mut theories = vec![
        theory(0, vec![], vec![]),
        theory(0, vec![Node::falsum()], vec![0]),
        theory(1, vec![Node::atom(0)], vec![0]),
        // Duplicate leaves retain one semantic slot even when non-Atom slots
        // interleave them. The original child references remain DAG indices.
        theory(
            2,
            vec![
                Node::atom(0),
                Node::falsum(),
                Node::atom(0),
                Node::atom(1),
                Node::and_pair([0, 2]),
                Node::or_pair([4, 3]),
            ],
            vec![5],
        ),
        // In (not a)->a, M={a} satisfies the original formula, but empty J
        // satisfies the frozen reduct false->a. The false negation's original
        // connective must never force a while its dense output is false.
        theory(
            1,
            vec![
                Node::falsum(),
                Node::atom(0),
                Node::implies(1, 0),
                Node::implies(2, 1),
            ],
            vec![3],
        ),
        // Bare double negation: {p} is a model, but its reduct permits empty J.
        theory(
            1,
            vec![
                Node::falsum(),
                Node::atom(0),
                Node::implies(1, 0),
                Node::implies(2, 0),
            ],
            vec![3],
        ),
        // Choice p or not p retains both stable models.
        theory(
            1,
            vec![
                Node::falsum(),
                Node::atom(0),
                Node::implies(1, 0),
                Node::or_pair([1, 2]),
            ],
            vec![3],
        ),
        // Positive cycle admits only empty; disjunctive support admits both atoms.
        theory(
            2,
            vec![
                Node::atom(0),
                Node::atom(1),
                Node::implies(0, 1),
                Node::implies(1, 0),
            ],
            vec![2, 3],
        ),
        theory(
            2,
            vec![
                Node::atom(0),
                Node::atom(1),
                Node::implies(0, 1),
                Node::implies(1, 0),
                Node::or_pair([0, 1]),
            ],
            vec![2, 3, 4],
        ),
    ];
    theories.extend(crate::formula::wide::theories());
    theories
}

fn qualify_frozen_queries(backend: GpuApi, projection: GateProjection) {
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
                    Node::falsum(),
                    Node::atom(0),
                    Node::atom(1),
                    Node::implies(1, 0),
                ];
                nodes.push(match operator {
                    0 => Node::and_pair([left, right]),
                    1 => Node::or_pair([left, right]),
                    _ => Node::implies(left, right),
                });
                nodes.push(Node::implies(4, 1));
                let graph = theory(3, nodes, vec![5]);
                let count = compare(&mut oracle, &graph);
                totals.0 += count.0;
                totals.1 += count.1;
            }
        }
    }
    assert!(totals.0 > 0 && totals.1 > 0);
    println!("completed refutations={} residuals={}", totals.0, totals.1);
    qualify_root_propagation(&mut oracle);
    qualify_kernel_options(&mut oracle);
}

fn qualify_kernel_options(oracle: &mut GpuFormulaOracle) {
    let context = oracle.context().clone();
    println!("formula granted features={:?}", context.features());
    for workgroup_size in [GpuWorkgroupSize::Threads32, GpuWorkgroupSize::Threads64] {
        for optional_features in [false, true] {
            for propagation in [GpuPropagation::Dense, GpuPropagation::ChangedDependencies] {
                let options = GpuKernelOptions {
                    workgroup_size,
                    optional_features,
                    propagation,
                };
                let profile = GpuFormulaProfile::from_context_with_kernel(
                    &context,
                    oracle.projection(),
                    options,
                )
                .unwrap();
                assert!(profile.context().same_instance(&context));
                assert_eq!(profile.kernel_options(), options);
                let mut configured = GpuFormulaOracle::from_profile(&profile).unwrap();
                assert!(configured.context().same_instance(&context));
                assert_eq!(configured.compiled_profile().kernel_options(), options);
                println!("formula qualified options={options:?}");
                qualify_subset_strides(&mut configured);
                qualify_bidirectional_propagation(&mut configured);
                for theory in scheduled_theories() {
                    compare(&mut configured, &theory);
                    assert_eq!(
                        configured.last_batch_stats().unwrap().propagation,
                        propagation
                    );
                }
                native::qualify(&mut configured);
                native::qualify_shared_ordinals(&mut configured);
                if propagation == GpuPropagation::ChangedDependencies {
                    for theory in frozen_theories() {
                        compare(&mut configured, &theory);
                    }
                    qualify_next_generation(&mut configured);
                    qualify_incidence_fallback(&mut configured);
                }
            }
        }
    }
}

fn qualify_next_generation(oracle: &mut GpuFormulaOracle) {
    let graph = theory(
        2,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::or_pair([1, 1]),
            Node::implies(0, 2),
            Node::and_pair([0, 0]),
        ],
        vec![3, 4],
    );
    let candidate = Interpretation::new(&graph, [0, 1]).unwrap();
    assert!(
        check(
            &graph,
            &candidate,
            Limits::default(),
            &Cancellation::default()
        )
        .unwrap()
        .accepted()
    );
    // The implication precedes the lower-level conjunction that forces a.
    // Strict-subset projection then forces b false. Only the next generation
    // can revisit the implication and or(b,b) and discover the contradiction.
    // N=5, E=6, A=2, R=2, L=3 gives dense setup23/sweep127.
    // W=1, V=5, I=9 adds setup2 and sweep(1+5+20+54)=80.
    for (rounds, expected) in [
        (1, FormulaVerdict::Residual(ResidualReason::RoundLimit)),
        (2, FormulaVerdict::NoProperSubset),
    ] {
        let work = 25 + rounds * 207;
        let checks = oracle
            .propagate_batch(
                &graph,
                std::slice::from_ref(&candidate),
                FormulaLimits {
                    max_rounds: rounds,
                    max_work_per_candidate: work,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            oracle.last_batch_stats().unwrap().propagation,
            GpuPropagation::ChangedDependencies
        );
        assert_eq!(checks[0].verdict(), expected);
        assert_eq!(checks[0].statistics().rounds, rounds);
        assert_eq!(checks[0].statistics().work, work);
    }
}

fn qualify_incidence_fallback(oracle: &mut GpuFormulaOracle) {
    let theory = theory(
        2,
        vec![Node::atom(0), Node::atom(1), Node::and_pair([0, 1])],
        vec![2],
    );
    let inputs = candidates(&theory);
    oracle
        .propagate_batch(&theory, &inputs, FormulaLimits::default())
        .unwrap();
    assert_eq!(
        oracle.last_batch_stats().unwrap().propagation,
        GpuPropagation::ChangedDependencies
    );
    // Tighten mandatory work below indexed setup but exactly to dense setup.
    // The owned indexed graph must retire without converting refusal into proof.
    let results = oracle
        .propagate_batch(
            &theory,
            &inputs,
            FormulaLimits {
                max_work_per_candidate: setup_work(&theory),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        oracle.last_batch_stats().unwrap().propagation,
        GpuPropagation::Dense
    );
    for result in &results[..3] {
        assert_eq!(result.verdict(), FormulaVerdict::NotModel);
    }
    assert_eq!(
        results[3].verdict(),
        FormulaVerdict::Residual(ResidualReason::WorkLimit)
    );
    assert_eq!(
        oracle.compiled_profile().kernel_options().propagation,
        GpuPropagation::ChangedDependencies
    );
}

fn scheduled_theories() -> [Theory; 2] {
    let mut chain = vec![Node::atom(0)];
    chain.extend((1..128).map(|index| Node::and_pair([index - 1, index - 1])));
    [
        theory(1, chain, vec![127]),
        theory(
            2,
            vec![
                Node::atom(0),
                Node::and_pair([0, 0]),
                Node::atom(1),
                Node::or_pair([1, 2]),
                Node::falsum(),
                Node::implies(3, 4),
                Node::atom(0),
                Node::and_pair([2, 6]),
            ],
            // a-or-b gives original models, exact refutations and a residual;
            // the unused deeper nodes still exercise scheduled frozen truth.
            vec![3],
        ),
    ]
}

fn qualify_root_propagation(oracle: &mut GpuFormulaOracle) {
    let mut chain = vec![Node::atom(0)];
    chain.extend((1..128).map(|index| Node::and_pair([index - 1, index - 1])));
    let chain = theory(1, chain, vec![127]);

    // Each of the eight layers spans more than one lane stride. A true root
    // requires both semantic atoms, despite shared and repeated child slots.
    let mut nodes = vec![Node::atom(0), Node::atom(1)];
    let mut prior = vec![0, 1];
    for _ in 0..8 {
        let start = nodes.len();
        for index in 0..65 {
            nodes.push(Node::and_pair([
                prior[index % prior.len()],
                prior[(index + 1) % prior.len()],
            ]));
        }
        prior = (start..nodes.len()).collect();
    }
    let root = nodes.len() - 1;
    let layered = theory(2, nodes, vec![root]);
    for graph in [chain, layered] {
        let candidate = Interpretation::new(&graph, 0..graph.atom_count()).unwrap();
        assert!(
            check(
                &graph,
                &candidate,
                Limits::default(),
                &Cancellation::default()
            )
            .unwrap()
            .accepted()
        );
        let work = setup_work(&graph) + sweep_work(&graph);
        let results = oracle
            .propagate_batch(
                &graph,
                &[candidate],
                FormulaLimits {
                    max_rounds: 1,
                    max_work_per_candidate: work,
                    ..FormulaLimits::default()
                },
            )
            .unwrap();
        assert_eq!(results[0].verdict(), FormulaVerdict::NoProperSubset);
        assert_eq!(results[0].statistics().rounds, 1);
        assert_eq!(results[0].statistics().work, work);
    }
}

fn qualify_bidirectional_propagation(oracle: &mut GpuFormulaOracle) {
    let mut nodes = vec![Node::atom(0), Node::atom(1)];
    let mut prior = 0;
    for _ in 0..128 {
        nodes.push(Node::and_pair([prior, prior]));
        prior = nodes.len() - 1;
    }
    nodes.push(Node::implies(prior, 1));
    let root = nodes.len() - 1;
    let graph = theory(2, nodes, vec![0, root]);
    let candidate = Interpretation::new(&graph, [0, 1]).unwrap();
    assert!(
        check(
            &graph,
            &candidate,
            Limits::default(),
            &Cancellation::default()
        )
        .unwrap()
        .accepted()
    );
    // a and (C -> b), where C is a long chain of repeated conjunctions of a.
    // The downward sweep settles only the first conjunction; strict subset
    // requires b false. The upward sweep carries a through C and refutes that
    // requirement. Reverse-only sweeps would advance one chain link at a time.
    let propagation = oracle.compiled_profile().kernel_options().propagation;
    let (setup, sweep) = propagation_work(&graph, propagation);
    for (rounds, work, expected, completed_rounds) in [
        (
            1,
            setup + sweep,
            FormulaVerdict::Residual(ResidualReason::RoundLimit),
            1,
        ),
        (
            2,
            setup + 2 * sweep - 1,
            FormulaVerdict::Residual(ResidualReason::WorkLimit),
            1,
        ),
        (2, setup + 2 * sweep, FormulaVerdict::NoProperSubset, 2),
    ] {
        let checks = oracle
            .propagate_batch(
                &graph,
                std::slice::from_ref(&candidate),
                FormulaLimits {
                    max_rounds: rounds,
                    max_work_per_candidate: work,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(oracle.last_batch_stats().unwrap().propagation, propagation);
        assert_eq!(checks[0].verdict(), expected);
        assert_eq!(checks[0].statistics().rounds, completed_rounds);
        assert_eq!(
            checks[0].statistics().work,
            setup + completed_rounds * sweep
        );
    }
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
        let mut nodes = vec![Node::falsum()];
        nodes.extend((0..131).map(Node::atom));
        nodes.push(Node::atom(130));
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
        // reserves 9 per node, one per atom, 64 summaries, one application and
        // one dependency level.
        let setup = 529 - u32::try_from(available.len()).unwrap();
        let sweep = 1394;
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
#[ignore = "requires Metal: formula limits resize identity and word boundaries remain explicit"]
fn metal_formula_limits_resize_identity_and_word_boundaries_remain_explicit() {
    for projection in GateProjection::ALL {
        qualify_resources(GpuApi::Metal, projection);
    }
}

fn qualify_resources(backend: GpuApi, projection: GateProjection) {
    let mut oracle = oracle(backend, projection);
    qualify_prefixes(&mut oracle);
    let graph = theory(1, vec![Node::atom(0)], vec![0]);
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
    let equal = theory(1, vec![Node::atom(0)], vec![0]);
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
        let graph = theory(count, vec![Node::atom(count - 1)], vec![0]);
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
#[ignore = "requires Vulkan: formula queries preserve frozen semantics"]
fn vulkan_formula_queries_preserve_frozen_semantics() {
    for projection in GateProjection::ALL {
        qualify_frozen_queries(GpuApi::Vulkan, projection);
    }
}

#[test]
#[ignore = "requires Vulkan: formula resource boundaries remain explicit"]
fn vulkan_formula_resource_boundaries_remain_explicit() {
    for projection in GateProjection::ALL {
        qualify_resources(GpuApi::Vulkan, projection);
    }
}

fn qualify_prefixes(oracle: &mut GpuFormulaOracle) {
    let graph = theory(
        2,
        vec![Node::atom(0), Node::atom(1), Node::implies(0, 1)],
        vec![2],
    );
    let mut input = candidates(&graph);
    input.push(input[1].clone());
    let exact = oracle
        .propagate_batch(&graph, &input, FormulaLimits::default())
        .unwrap();
    oracle.clear_residency();
    oracle
        .propagate_batch(&graph, &input[..2], FormulaLimits::default())
        .unwrap();
    let cold_bytes = oracle.last_batch_stats().unwrap().accounted_bytes;
    oracle.clear_residency();
    let first = oracle
        .propagate_prefix_with_cancellation(
            &graph,
            &input,
            FormulaLimits {
                max_batch_bytes: cold_bytes,
                ..Default::default()
            },
            &Cancellation::default(),
        )
        .unwrap();
    assert_eq!(first.checks(), &exact[..2]);
    assert!(std::ptr::eq(first.candidates(), &raw const input[..2]));
    assert!(oracle.last_batch_stats().unwrap().theory_uploaded);
    assert_eq!(oracle.last_submission_candidates(), Some(2));
    let mut completed = first.checks().to_vec();
    while completed.len() < input.len() {
        let suffix = &input[completed.len()..];
        let next = oracle
            .propagate_prefix_with_cancellation(
                &graph,
                suffix,
                FormulaLimits {
                    max_candidates: 2,
                    max_batch_bytes: cold_bytes,
                    ..Default::default()
                },
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(next.checks().len(), suffix.len().min(2));
        assert!(std::ptr::eq(
            next.candidates(),
            &raw const suffix[..next.checks().len()]
        ));
        assert!(!oracle.last_batch_stats().unwrap().theory_uploaded);
        completed.extend_from_slice(next.checks());
    }
    assert_eq!(completed, exact);
    let one_bytes = oracle.last_batch_stats().unwrap().accounted_bytes;
    let error = oracle
        .propagate_prefix_with_cancellation(
            &graph,
            &input,
            FormulaLimits {
                max_batch_bytes: one_bytes - 1,
                ..Default::default()
            },
            &Cancellation::default(),
        )
        .err()
        .unwrap();
    assert_eq!(error.kind(), GpuErrorKind::Capacity);
    assert_eq!(oracle.last_submission_candidates(), None);
    assert!(oracle.last_batch_stats().is_none());
    let empty = oracle
        .propagate_prefix_with_cancellation(
            &graph,
            &[],
            FormulaLimits {
                max_candidates: 0,
                max_batch_bytes: 0,
                ..Default::default()
            },
            &Cancellation::default(),
        )
        .unwrap();
    assert!(empty.checks().is_empty());
    assert_eq!(oracle.last_submission_candidates(), None);
}
