//! A head's choice depth is the longest chain of choosing components below it.

use super::{choose, prepare, theory};
use crate::{Node, Region};

/// `atoms` atoms: each atom set in `chosen` has an atomic choice, and each
/// pair `(from, to)` in `rules` is the rule `to :- from`.
fn dependent(atoms: usize, chosen: u8, rules: &[(usize, usize)]) -> crate::Theory {
    let falsum = atoms;
    let mut nodes: Vec<Node> = (0..atoms).map(Node::atom).collect();
    nodes.push(Node::falsum());
    let mut roots = Vec::new();
    for atom in 0..atoms {
        if chosen & (1 << atom) != 0 {
            let absent = nodes.len();
            nodes.push(Node::implies(atom, falsum));
            roots.push(nodes.len());
            nodes.push(Node::or_pair([absent, atom]));
        }
    }
    for &(from, to) in rules {
        roots.push(nodes.len());
        nodes.push(Node::implies(from, to));
    }
    theory(atoms, nodes, roots)
}

#[test]
fn every_small_dependency_graph_has_correct_choice_depths() {
    // All 64 directed graphs on three atoms, under each of the seven nonempty
    // sets of atoms that have a choice. An independent closure decides
    // reachability; a component chooses when one of its atoms does; and an
    // atom's depth is the longest chain of choosing components that reach its
    // own from outside it. Three atoms have chains of two steps at most, so
    // three relaxations reach every depth.
    let pairs = [(0, 1), (0, 2), (1, 0), (1, 2), (2, 0), (2, 1)];
    for chosen in 1_u8..8 {
        for selected in 0_u8..64 {
            let rules: Vec<(usize, usize)> = pairs
                .iter()
                .enumerate()
                .filter(|(bit, _)| selected & (1 << bit) != 0)
                .map(|(_, &pair)| pair)
                .collect();
            let mut reachable = [[false; 3]; 3];
            for &(from, to) in &rules {
                reachable[from][to] = true;
            }
            for via in 0..3 {
                for from in 0..3 {
                    for to in 0..3 {
                        reachable[from][to] |= reachable[from][via] && reachable[via][to];
                    }
                }
            }
            let together = |a: usize, b: usize| a == b || reachable[a][b] && reachable[b][a];
            let chooses = |a: usize| (0..3).any(|b| together(a, b) && chosen & (1 << b) != 0);
            let mut depth = [0_usize; 3];
            for _ in 0..3 {
                for to in 0..3 {
                    for from in 0..3 {
                        if reachable[from][to] && !together(from, to) {
                            depth[to] = depth[to].max(depth[from] + usize::from(chooses(from)));
                        }
                    }
                }
            }
            let theory = dependent(3, chosen, &rules);
            let order = prepare(&theory).result.unwrap().unwrap();
            assert_eq!(
                order.depths, depth,
                "graph {selected:06b}, choices {chosen:03b}"
            );
        }
    }
}

#[test]
fn independent_choices_share_depth_zero() {
    let theory = dependent(4, 0b1111, &[]);
    let order = prepare(&theory).result.unwrap().unwrap();
    assert_eq!(order.depths, [0, 0, 0, 0]);
}

#[test]
fn a_depth_is_the_longest_chain_not_the_number_of_choices_below() {
    // 3 reads the choices 0, 1 and 2, and only 1 reads another choice: the
    // longest chain below 3 is 0, 1, so its depth is two, not three.
    let theory = dependent(4, 0b1111, &[(0, 1), (0, 3), (1, 3), (2, 3)]);
    let order = prepare(&theory).result.unwrap().unwrap();
    assert_eq!(order.depths, [0, 1, 0, 2]);
}

#[test]
fn a_derived_atom_between_two_choices_adds_no_step() {
    // 0 is a choice, 1 is derived from it and chooses nothing, and the choice
    // 2 reads 1: one choosing component lies below 2, and below 1.
    let theory = dependent(3, 0b101, &[(0, 1), (1, 2)]);
    let order = prepare(&theory).result.unwrap().unwrap();
    assert_eq!(order.depths, [0, 1, 1]);
}

#[test]
fn an_open_head_of_lesser_depth_precedes_an_earlier_component() {
    // 1 is derived from the choice 2, so its component follows 2's and
    // precedes that of the independent choice 0. With 2 decided, 0 has the
    // lesser depth and 1 the earlier component.
    let theory = dependent(3, 0b111, &[(2, 1)]);
    let order = prepare(&theory).result.unwrap().unwrap();
    assert_eq!(order.depths, [0, 1, 0]);
    assert!(order.ranks[2] < order.ranks[1] && order.ranks[1] < order.ranks[0]);
    let mut region = Region::all_open(3);
    assert_eq!(choose(&order, &theory, &region), Some(2));
    assert!(region.cut(2));
    assert_eq!(choose(&order, &theory, &region), Some(0));
}
