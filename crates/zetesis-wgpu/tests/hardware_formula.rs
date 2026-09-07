//! Explicit Metal qualification of the partial frozen-query primitive.
//! No adapter absence or residual query is replaced by a CPU GPU-result claim.

use zetesis_cpu::Control;
use zetesis_ferraris::{AdmissionLimits, Interpretation, Limits, Node, Theory, Verdict, check};
use zetesis_wgpu::{
    FormulaLimits, FormulaVerdict, GateProjection, GpuErrorKind, GpuFormulaOracle, GpuOptions,
    ResidualReason,
};

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
            &Control::default(),
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
        let setup =
            u32::try_from(2 * theory.nodes().len() + theory.atom_count() + theory.roots().len())
                .unwrap();
        let sweep = u32::try_from(9 * theory.nodes().len() + theory.atom_count() + 1).unwrap();
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
    assert_eq!(
        cold.accounted_bytes - hot.accounted_bytes,
        hot.resident_theory_bytes
    );
    (refutations, residuals)
}

#[test]
#[ignore = "requires an actual Metal GPU; explicit hardware qualification only"]
fn metal_formula_queries_preserve_exact_frozen_semantics_and_residency() {
    for projection in GateProjection::ALL {
        qualify_frozen_queries(projection);
    }
}

fn qualify_frozen_queries(projection: GateProjection) {
    let mut oracle =
        GpuFormulaOracle::new_metal_with_projection(GpuOptions::default(), projection).unwrap();
    assert_eq!(oracle.projection(), projection);
    assert_eq!(oracle.info().backend(), "Metal");
    assert!(oracle.info().is_hardware_gpu());
    println!(
        "formula projection={} adapter={}",
        projection.label(),
        oracle.info().name()
    );
    let mut totals = (0, 0);
    for graph in [
        theory(0, vec![], vec![]),
        theory(0, vec![Node::False], vec![0]),
        theory(1, vec![Node::Atom(0)], vec![0]),
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
    ] {
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
}

#[test]
#[ignore = "requires an actual Metal GPU; explicit hardware qualification only"]
fn metal_formula_limits_resize_identity_and_word_boundaries_remain_explicit() {
    for projection in GateProjection::ALL {
        qualify_resources(projection);
    }
}

fn qualify_resources(projection: GateProjection) {
    let mut oracle =
        GpuFormulaOracle::new_metal_with_projection(GpuOptions::default(), projection).unwrap();
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
                    ..FormulaLimits::default()
                }
            )
            .unwrap()
            .is_empty()
    );
    assert!(oracle.last_batch_stats().is_none());
    oracle.clear_residency();
    oracle
        .propagate_batch(&graph, &inputs, FormulaLimits::default())
        .unwrap();
    assert!(oracle.last_batch_stats().unwrap().theory_uploaded);
}
