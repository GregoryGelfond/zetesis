//! Portable formula layout, shader validation and independent propagation laws.

use zetesis_ferraris::{AdmissionLimits, Node, Theory};
use zetesis_wgpu::{FormulaLimits, FormulaVerdict, ResidualReason};

#[test]
fn cooperative_formula_shader_validates_without_optional_capabilities() {
    let module = naga::front::wgsl::parse_str(include_str!("../src/formula.wgsl")).unwrap();
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .unwrap();
    assert_eq!(module.entry_points[0].name, "propagate");
    assert_eq!(module.entry_points[0].workgroup_size, [64, 1, 1]);
    assert_eq!(
        module
            .global_variables
            .iter()
            .filter(|(_, v)| matches!(v.space, naga::AddressSpace::Storage { .. }))
            .count(),
        6
    );
}
fn values(theory: &Theory, bits: usize, frozen: Option<&[bool]>) -> Vec<bool> {
    let mut out = Vec::new();
    for (index, node) in theory.nodes().iter().enumerate() {
        let truth = match *node {
            Node::False => false,
            Node::Atom(a) => bits & (1 << a) != 0,
            Node::And(a, b) => out[a] && out[b],
            Node::Or(a, b) => out[a] || out[b],
            Node::Implies(a, b) => !out[a] || out[b],
        };
        out.push(truth && frozen.is_none_or(|mask| mask[index]));
    }
    out
}
fn model(theory: &Theory, bits: usize, frozen: Option<&[bool]>) -> bool {
    let out = values(theory, bits, frozen);
    theory.roots().iter().all(|&root| out[root])
}
fn variable(theory: &Theory, node: usize) -> usize {
    if let Node::Atom(atom) = theory.nodes()[node] {
        atom
    } else {
        theory.atom_count() + node
    }
}
// A scalar TEST-ONLY specification of propagation. The independent model
// definition above checks its completed refutations; this is not GPU execution.
fn propagate(theory: &Theory, candidate: usize, round_limit: usize) -> FormulaVerdict {
    let mask = values(theory, candidate, None);
    if !model(theory, candidate, None) {
        return FormulaVerdict::NotModel;
    }
    let mut domains = vec![3u8; theory.atom_count() + theory.nodes().len()];
    for (a, domain) in domains.iter_mut().take(theory.atom_count()).enumerate() {
        if candidate & (1 << a) == 0 {
            *domain = 1;
        }
    }
    for (n, truth) in mask.iter().enumerate() {
        if !truth {
            domains[variable(theory, n)] = 1;
        }
    }
    for &root in theory.roots() {
        domains[variable(theory, root)] &= 2;
    }
    for _ in 0..round_limit {
        let before = domains.clone();
        for (n, node) in theory.nodes().iter().enumerate() {
            if !mask[n] {
                continue;
            }
            let (Node::And(a, b) | Node::Or(a, b) | Node::Implies(a, b)) = *node else {
                continue;
            };
            let ids = [
                variable(theory, a),
                variable(theory, b),
                variable(theory, n),
            ];
            let mut permitted = [0u8; 3];
            for x in [false, true] {
                for y in [false, true] {
                    let z = match node {
                        Node::And(..) => x && y,
                        Node::Or(..) => x || y,
                        _ => !x || y,
                    };
                    let tuple = [x, y, z];
                    if (0..3).any(|i| {
                        domains[ids[i]] & (1 << u8::from(tuple[i])) == 0
                            || (0..i).any(|j| ids[i] == ids[j] && tuple[i] != tuple[j])
                    }) {
                        continue;
                    }
                    for i in 0..3 {
                        permitted[i] |= 1 << u8::from(tuple[i]);
                    }
                }
            }
            for i in 0..3 {
                domains[ids[i]] &= permitted[i];
            }
        }
        let available: Vec<_> = (0..theory.atom_count())
            .filter(|a| candidate & (1 << a) != 0 && domains[*a] & 1 != 0)
            .collect();
        for inner in 0..1usize << theory.atom_count() {
            if inner == candidate || inner & !candidate != 0 || !model(theory, inner, Some(&mask)) {
                continue;
            }
            let evaluation = values(theory, inner, Some(&mask));
            for (atom, domain) in domains.iter().take(theory.atom_count()).enumerate() {
                assert_ne!(
                    domain & (1 << u8::from(inner & (1 << atom) != 0)),
                    0,
                    "a real frozen-query completion lost a semantic atom value"
                );
            }
            for (node, truth) in evaluation.iter().enumerate() {
                assert_ne!(
                    domains[variable(theory, node)] & (1 << u8::from(*truth)),
                    0,
                    "a real frozen-query completion lost a node value"
                );
            }
        }
        if domains.contains(&0) || available.is_empty() {
            return FormulaVerdict::NoProperSubset;
        }
        if available.len() == 1 {
            domains[available[0]] &= 1;
        }
        if domains == before {
            return FormulaVerdict::Residual(ResidualReason::FixedPoint);
        }
    }
    FormulaVerdict::Residual(ResidualReason::RoundLimit)
}
#[test]
fn independent_frozen_queries_validate_all_reference_propagation_refutations() {
    let mut checked = 0;
    let mut residuals = 0;
    let mut refuted = 0;
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
                // Context mixes positive recursion, choices and shared children.
                nodes.extend([Node::Implies(4, 1), Node::Or(2, 3)]);
                for roots in [vec![4], vec![5], vec![4, 6], vec![]] {
                    let theory =
                        Theory::new(3, nodes.clone(), roots, AdmissionLimits::default()).unwrap();
                    for candidate in 0..8 {
                        let mask = values(&theory, candidate, None);
                        let stable = model(&theory, candidate, None)
                            && (0..8).all(|inner| {
                                inner == candidate
                                    || inner & !candidate != 0
                                    || !model(&theory, inner, Some(&mask))
                            });
                        match propagate(&theory, candidate, 32) {
                            FormulaVerdict::NotModel => assert!(!model(&theory, candidate, None)),
                            FormulaVerdict::NoProperSubset => {
                                assert!(stable);
                                refuted += 1;
                            }
                            FormulaVerdict::Residual(_) => {
                                assert!(model(&theory, candidate, None));
                                residuals += 1;
                            }
                        }
                        checked += 1;
                    }
                }
            }
        }
    }
    assert_eq!(checked, 1536);
    assert!(residuals > 0);
    assert!(refuted > 0);
}
#[test]
fn empty_queries_and_inactive_composites_do_not_forge_stability() {
    let empty = Theory::new(0, vec![], vec![], AdmissionLimits::default()).unwrap();
    assert_eq!(propagate(&empty, 0, 1), FormulaVerdict::NoProperSubset);
    assert_eq!(
        propagate(&empty, 0, 0),
        FormulaVerdict::Residual(ResidualReason::RoundLimit)
    );
    let theory = Theory::new(
        1,
        vec![
            Node::False,
            Node::Atom(0),
            Node::Implies(1, 0),
            Node::Implies(2, 0),
        ],
        vec![3],
        AdmissionLimits::default(),
    )
    .unwrap();
    // Bare not not p supplies no reduct support for p; its false inner not-p
    // connective must be disabled. The proper empty subset remains possible.
    assert_eq!(
        propagate(&theory, 1, 16),
        FormulaVerdict::Residual(ResidualReason::FixedPoint)
    );
    assert!(!model(&theory, 0, None));
    assert!(model(&theory, 0, Some(&values(&theory, 1, None))));
    let limits = FormulaLimits::default();
    assert!(limits.max_rounds > 0);
    assert!(limits.max_work_per_candidate > 0);
}
