//! Cyclic-head projection preserves the original ordered activation relation.

use super::*;

/// a and b form a cycle. A body and a fact mix cyclic and acyclic heads,
/// repeated producers retain repeated a occurrences, and an opaque seed can
/// independently activate the cycle. Additional producers only derive c.
fn shared_heads(fact: bool, extra: usize) -> Theory {
    let mut nodes = vec![
        Node::atom(0),
        Node::atom(1),
        Node::atom(2),
        Node::falsum(),
        Node::implies(2, 3),
        Node::or_pair([0, 2]),
        Node::implies(1, 5),
        Node::implies(0, 1),
        Node::implies(1, 0),
        Node::implies(4, 0),
    ];
    let mut roots = vec![6, 7, 8, 9];
    if fact {
        roots.push(5);
    }
    for _ in 0..extra {
        roots.push(nodes.len());
        nodes.push(Node::implies(1, 2));
    }
    theory(3, nodes, roots)
}

/// An independent flattening of all original rows: no selected body cone or
/// cyclic-head filter. The existing activation operation rejects noncyclic heads.
pub(super) fn original_rows(owner: &Theory, producers: &Producers) -> Adjacency {
    let nodes = owner.nodes().len();
    let bodies = (0..nodes).flat_map(|node| {
        producers.by_body[node].iter().flat_map(move |&producer| {
            producers.rules[producer]
                .heads
                .iter()
                .map(move |&head| (node, head))
        })
    });
    let facts = producers
        .rules
        .iter()
        .filter(|producer| producer.body.is_none())
        .flat_map(|producer| producer.heads.iter().map(move |&head| (nodes, head)));
    Adjacency::build(nodes + 1, bodies.chain(facts)).unwrap()
}

pub(super) fn assert_rows(scratch: &Scratch, owner: &Theory, producers: &Producers) {
    assert!(scratch.owner.as_ref().unwrap().same_instance(owner));
    let projected = &scratch.projected.as_ref().unwrap().heads;
    let original = original_rows(owner, producers);
    assert_eq!(projected.len(), owner.nodes().len() + 1);
    for node in 0..projected.len() {
        let included = node == owner.nodes().len() || scratch.body_nodes.contains(&node);
        let expected: Vec<_> = original[node]
            .iter()
            .copied()
            .filter(|&head| included && bit(&scratch.cyclic_atoms, head))
            .collect();
        assert_eq!(projected[node], expected, "head row {node}");
    }
}

#[test]
fn head_projection_keeps_order_and_multiplicity() {
    let owner = shared_heads(true, 2);
    let index = Narrower::new(&owner);
    let producers = producers(&owner);
    let mut scratch = Scratch::default();
    scratch.prepare_projected(&owner, &index, &producers);
    assert_rows(&scratch, &owner, &producers);
    assert_eq!(scratch.cyclic_atoms, [0b011]);
    let projected = scratch.projected.as_ref().unwrap();
    assert_eq!(&projected.heads[1], &[0, 0]);
    assert_eq!(projected.fact_heads(), &[0]);
    let original = original_rows(&owner, &producers);
    assert!(original[1].len() > projected.heads[1].len());
    assert!(original[owner.nodes().len()].len() > projected.fact_heads().len());
}

#[test]
fn every_small_mask_matches_original_heads() {
    let mut supported = false;
    let mut unsupported = false;
    for fact in [false, true] {
        let owner = shared_heads(fact, 2);
        let index = Narrower::new(&owner);
        let producers = producers(&owner);
        let mut projected = Scratch::default();
        let mut original = Scratch::default();
        for scratch in [&mut projected, &mut original] {
            scratch.prepare_projected(&owner, &index, &producers);
        }
        original.projected.as_mut().unwrap().heads = original_rows(&owner, &producers);
        let body_nodes = projected.body_nodes.clone();
        assert_eq!(body_nodes.len(), 3);
        for false_bodies in 0..1_usize << body_nodes.len() {
            let mut never = vec![0; flag_words(owner.nodes().len())];
            for (position, &node) in body_nodes.iter().enumerate() {
                if false_bodies & (1 << position) != 0 {
                    never[node / 64] |= 1 << (node % 64);
                }
            }
            for cut_atoms in 0..1_u64 << owner.atom_count() {
                for scratch in [&mut projected, &mut original] {
                    scratch
                        .complete(
                            &owner,
                            &index,
                            &producers,
                            &never,
                            &[cut_atoms],
                            &mut Work::new(u64::MAX),
                        )
                        .unwrap();
                }
                assert_eq!(
                    projected.atoms, original.atoms,
                    "fact {fact}, body mask {false_bodies}, atom mask {cut_atoms}"
                );
                assert_eq!(
                    projected.missing_in_word(0, cut_atoms),
                    original.missing_in_word(0, cut_atoms)
                );
                if cut_atoms == 0 {
                    supported |= projected.atoms == [0b011];
                    unsupported |= projected.atoms == [0];
                }
            }
        }
        assert_rows(&projected, &owner, &producers);
        assert!(original.projected.as_ref().unwrap().heads[1].len() > 2);
    }
    assert!(supported && unsupported);
}

fn warm(extra: usize, projected: bool) -> u64 {
    let owner = shared_heads(false, extra);
    let index = Narrower::new(&owner);
    let producers = producers(&owner);
    let mut scratch = Scratch::default();
    scratch.prepare_projected(&owner, &index, &producers);
    if !projected {
        scratch.projected.as_mut().unwrap().heads = original_rows(&owner, &producers);
    }
    let mut work = Work::new(u64::MAX);
    scratch
        .complete(
            &owner,
            &index,
            &producers,
            &vec![0; flag_words(owner.nodes().len())],
            &[0],
            &mut work,
        )
        .unwrap();
    assert_eq!(scratch.atoms, [0b011]);
    assert_eq!(scratch.body_nodes.len(), 3);
    work.spent
}

#[test]
fn noncyclic_heads_add_no_warm_work() {
    assert_eq!(warm(0, true), warm(200, true));
    assert!(warm(200, false) > warm(0, false));
}
