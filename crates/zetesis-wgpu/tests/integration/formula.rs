//! Portable formula layout, shader validation and independent propagation laws.

use zetesis_ferraris::{AdmissionLimits, Node, NodeView, Theory};
use zetesis_wgpu::{FormulaLimits, FormulaVerdict, GateProjection, ResidualReason};

mod gate_transfer;
mod projection;
pub(crate) mod wide;

#[test]
fn cooperative_formula_shader_validates_without_optional_capabilities() {
    let module = naga::front::wgsl::parse_str(include_str!("../../src/formula.wgsl")).unwrap();
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
    for index in 0..theory.view().len() {
        let truth = match theory.view().node(index).unwrap() {
            NodeView::False => false,
            NodeView::Atom(a) => bits & (1 << a) != 0,
            NodeView::And(children) => children.iter().all(|&child| out[child]),
            NodeView::Or(children) => children.iter().any(|&child| out[child]),
            NodeView::Implies(a, b) => !out[a] || out[b],
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
    if let NodeView::Atom(atom) = theory.view().node(node).unwrap() {
        atom
    } else {
        theory.atom_count() + node
    }
}
// A scalar TEST-ONLY specification of propagation. The independent model
// definition above checks its completed refutations; this is not GPU execution.
fn row_support(node: NodeView<'_>, ids: [usize; 3], domains: &[u8]) -> [u8; 3] {
    let mut permitted = [0u8; 3];
    for x in [false, true] {
        for y in [false, true] {
            let z = match node {
                NodeView::And(..) => x && y,
                NodeView::Or(..) => x || y,
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
    permitted
}

fn bitwise_support(node: NodeView<'_>, ids: [usize; 3], domains: &[u8]) -> [u8; 3] {
    use gate_transfer::{Aliases, Domains, Operation, bitwise};
    let operation = match node {
        NodeView::And(..) => Operation::And,
        NodeView::Or(..) => Operation::Or,
        NodeView::Implies(..) => Operation::Implies,
        _ => panic!("only enabled connectives enter projection"),
    };
    let [left, right, output] = ids.map(|id| u32::try_from(id).unwrap());
    let observed = ids.map(|id| domains[id]);
    bitwise(
        operation,
        Aliases::from_slots(left, right, output),
        Domains::new(observed[0], observed[1], observed[2]).unwrap(),
    )
    .masks()
}

// Same two-pass native transfer as the shader; checked below against independently
// enumerated Boolean completions, including repeated physical child aliases.
fn group_allowed(
    conjunction: bool,
    ids: &[usize],
    observed: &[u8],
    output: u8,
) -> (u8, Option<(usize, u8)>) {
    let all_bit = if conjunction { 2 } else { 1 };
    let witness_bit = 3 ^ all_bit;
    let all_possible = observed.iter().all(|d| d & all_bit != 0);
    let mut witness = None;
    let mut multiple = false;
    for (&slot, &domain) in ids.iter().zip(observed) {
        if domain & witness_bit != 0 {
            multiple |= witness.is_some_and(|old| old != slot);
            witness = Some(slot);
        }
    }
    let allowed = if observed.contains(&0) {
        0
    } else {
        (if all_possible { all_bit } else { 0 }) | (if witness.is_some() { witness_bit } else { 0 })
    };
    let forced = match output & allowed {
        value if value == all_bit => Some((usize::MAX, all_bit)),
        value if value == witness_bit && !multiple => witness.map(|slot| (slot, witness_bit)),
        _ => None,
    };
    (allowed, forced)
}
fn group_support(conjunction: bool, ids: &[usize], output: usize, domains: &mut [u8]) {
    let observed: Vec<_> = ids.iter().map(|&id| domains[id]).collect();
    let (allowed, forced) = group_allowed(conjunction, ids, &observed, domains[output]);
    domains[output] &= allowed;
    if let Some((slot, bit)) = forced {
        if slot == usize::MAX {
            for &id in ids {
                domains[id] &= bit;
            }
        } else {
            domains[slot] &= bit;
        }
    }
}

fn propagate_with(
    theory: &Theory,
    candidate: usize,
    round_limit: usize,
    projection: GateProjection,
) -> FormulaVerdict {
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
        for (n, enabled) in mask.iter().copied().enumerate() {
            if !enabled {
                continue;
            }
            let node = theory.view().node(n).unwrap();
            let (a, b) = match node {
                NodeView::And(children) | NodeView::Or(children) if children.len() > 2 => {
                    let ids: Vec<_> = children
                        .iter()
                        .map(|&child| variable(theory, child))
                        .collect();
                    group_support(
                        matches!(node, NodeView::And(_)),
                        &ids,
                        variable(theory, n),
                        &mut domains,
                    );
                    continue;
                }
                NodeView::And(children) | NodeView::Or(children) => (children[0], children[1]),
                NodeView::Implies(a, b) => (a, b),
                _ => continue,
            };
            let ids = [
                variable(theory, a),
                variable(theory, b),
                variable(theory, n),
            ];
            let permitted = match projection {
                GateProjection::Enumerated => row_support(node, ids, &domains),
                GateProjection::Bitwise => bitwise_support(node, ids, &domains),
            };
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
fn projections_preserve_frozen_query_completions() {
    let mut checked = 0;
    let mut residuals = 0;
    let mut refuted = 0;
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
                // Context mixes positive recursion, choices and shared children.
                nodes.extend([Node::implies(4, 1), Node::or_pair([2, 3])]);
                for roots in [vec![4], vec![5], vec![4, 6], vec![]] {
                    let theory = Theory::new(
                        3,
                        zetesis_ferraris::FormulaParts::new(nodes.clone(), Vec::new()).unwrap(),
                        roots,
                        AdmissionLimits::default(),
                    )
                    .unwrap();
                    for candidate in 0..8 {
                        let mask = values(&theory, candidate, None);
                        let stable = model(&theory, candidate, None)
                            && (0..8).all(|inner| {
                                inner == candidate
                                    || inner & !candidate != 0
                                    || !model(&theory, inner, Some(&mask))
                            });
                        for projection in GateProjection::ALL {
                            match propagate_with(&theory, candidate, 32, projection) {
                                FormulaVerdict::NotModel => {
                                    assert!(!model(&theory, candidate, None));
                                }
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
    }
    assert_eq!(checked, 2 * 1536);
    assert!(residuals > 0);
    assert!(refuted > 0);
}
#[test]
fn empty_queries_and_inactive_composites_do_not_forge_stability() {
    let empty = Theory::new(
        0,
        zetesis_ferraris::FormulaParts::new(vec![], Vec::new()).unwrap(),
        vec![],
        AdmissionLimits::default(),
    )
    .unwrap();
    for projection in GateProjection::ALL {
        assert_eq!(
            propagate_with(&empty, 0, 1, projection),
            FormulaVerdict::NoProperSubset
        );
        assert_eq!(
            propagate_with(&empty, 0, 0, projection),
            FormulaVerdict::Residual(ResidualReason::RoundLimit)
        );
    }
    let theory = Theory::new(
        1,
        zetesis_ferraris::FormulaParts::new(
            vec![
                Node::falsum(),
                Node::atom(0),
                Node::implies(1, 0),
                Node::implies(2, 0),
            ],
            Vec::new(),
        )
        .unwrap(),
        vec![3],
        AdmissionLimits::default(),
    )
    .unwrap();
    // Bare not not p supplies no reduct support for p; its false inner not-p
    // connective must be disabled. The proper empty subset remains possible.
    for projection in GateProjection::ALL {
        assert_eq!(
            propagate_with(&theory, 1, 16, projection),
            FormulaVerdict::Residual(ResidualReason::FixedPoint)
        );
    }
    assert!(!model(&theory, 0, None));
    assert!(model(&theory, 0, Some(&values(&theory, 1, None))));
    let limits = FormulaLimits::default();
    assert!(limits.max_rounds > 0);
    assert!(limits.max_work_per_candidate > 0);
}
