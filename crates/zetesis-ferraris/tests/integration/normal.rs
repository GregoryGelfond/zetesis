//! The normal-program definition, computed directly as the least model of the
//! reduct, and general subset-minimality agree after lowering.

use proptest::prelude::*;
use zetesis_cpu::Cancellation;
use zetesis_ferraris::{Interpretation, Node, Theory};

const ATOMS: [&str; 3] = ["a", "b", "c"];

// Head 3 denotes a constraint; body masks select among a, b and c.
type MaskRule = (u8, u8, u8, u8);

/// Whether `mask` equals the least consequences of its frozen normal reduct
/// and satisfies its constraints. This calculation reads only the rule masks,
/// not the translated formula, its evaluator or its membership checker.
/// Each changed scan adds an atom, so at most three changed scans are possible.
fn least_model_of_reduct(rules: &[MaskRule], mask: u8) -> bool {
    let kept: Vec<_> = rules
        .iter()
        .copied()
        .filter(|&(_, _, gate_true, gate_false)| {
            mask & gate_true == gate_true && mask & gate_false == 0
        })
        .collect();
    let mut least = 0u8;
    loop {
        let before = least;
        for &(head, positive, _, _) in &kept {
            if head < 3 && least & positive == positive {
                least |= 1 << head;
            }
        }
        if least == before {
            break;
        }
    }
    least == mask
        && kept
            .iter()
            .all(|&(head, positive, _, _)| head < 3 || mask & positive != positive)
}

/// The normalized-rule translation over a fixed three-atom vocabulary, even
/// when some atoms never occur. True gates are doubly negated, false gates
/// singly negated; positive body atoms stay positive. Conjunctions are folded
/// in source order as in `NormalFerraris.rule`, independently of grounding.
fn formula(rules: &[MaskRule]) -> Theory {
    let mut nodes: Vec<_> = (0..ATOMS.len()).map(Node::atom).collect();
    let falsum = nodes.len();
    nodes.push(Node::falsum());
    let truth = nodes.len();
    nodes.push(Node::implies(falsum, falsum));
    let mut roots = Vec::new();
    for &(head, positive, gate_true, gate_false) in rules {
        let mut body: Vec<_> = (0..ATOMS.len())
            .filter(|atom| positive & (1 << atom) != 0)
            .collect();
        for atom in (0..ATOMS.len()).filter(|atom| gate_true & (1 << atom) != 0) {
            let negative = nodes.len();
            nodes.push(Node::implies(atom, falsum));
            body.push(nodes.len());
            nodes.push(Node::implies(negative, falsum));
        }
        for atom in (0..ATOMS.len()).filter(|atom| gate_false & (1 << atom) != 0) {
            body.push(nodes.len());
            nodes.push(Node::implies(atom, falsum));
        }
        let body = body
            .into_iter()
            .reduce(|left, right| {
                let conjunction = nodes.len();
                nodes.push(Node::and_pair([left, right]));
                conjunction
            })
            .unwrap_or(truth);
        let head = if head < 3 { usize::from(head) } else { falsum };
        roots.push(nodes.len());
        nodes.push(Node::implies(body, head));
    }
    zetesis_theory_support::theories::theory(ATOMS.len(), nodes, roots)
}

fn compare(rules: &[MaskRule]) {
    let theory = formula(rules);
    assert_eq!(theory.atom_count(), ATOMS.len());
    for mask in 0u8..(1 << ATOMS.len()) {
        let candidate = Interpretation::new(
            &theory,
            (0..ATOMS.len()).filter(|atom| mask & (1 << atom) != 0),
        )
        .unwrap();
        let checked = zetesis_ferraris::check(
            &theory,
            &candidate,
            zetesis_ferraris::Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(
            checked.accepted(),
            least_model_of_reduct(rules, mask),
            "rules {rules:?}, candidate {mask:03b}"
        );
    }
}

#[test]
fn choice_support_cycles_constraints_and_unused_atoms_keep_their_semantics() {
    for rules in [
        vec![],
        vec![(0, 0, 1, 0)],               // {a}: two models, unlike a :- a.
        vec![(0, 1, 0, 0)],               // Unsupported positive self-loop.
        vec![(0, 2, 0, 0), (1, 1, 0, 0)], // Unsupported positive cycle.
        vec![(0, 0, 0, 2), (1, 0, 0, 1)], // Default-negation cycle.
        vec![(0, 0, 1, 0), (3, 1, 0, 0)], // Choice constrained false.
        vec![(0, 0, 1, 0), (3, 0, 0, 1)], // Choice constrained true.
        vec![(3, 0, 0, 0)],               // Empty constraint.
        vec![(0, 0, 1, 1)],               // Contradictory gates do not support a.
        vec![(0, 0, 0, 0), (2, 4, 0, 0)], // Unsupported carrier c.
    ] {
        compare(&rules);
    }
}

proptest! {
    #[test]
    fn generated_three_atom_normal_programs_preserve_stability(
        rules in prop::collection::vec((0u8..4, 0u8..8, 0u8..8, 0u8..8), 0..12)
    ) {
        compare(&rules);
    }
}
