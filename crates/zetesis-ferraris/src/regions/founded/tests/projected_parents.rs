//! Retained parent rows change traversal cost, never the completed relaxation.

use super::*;

/// Two nodes denote a. Both are required by the flattened conjunction. The
/// cycle also has an OR body, an opaque negative seed, a disjunctive producer,
/// and parent incidences outside the selected body cone.
fn shared_bodies(extra: usize) -> Theory {
    let mut nodes = vec![
        Node::atom(0),
        Node::atom(1),
        Node::atom(2),
        Node::atom(0),
        Node::falsum(),
        Node::implies(0, 4),
        Node::and_pair([0, 3]),
        Node::and_pair([6, 0]),
        Node::or_pair([0, 5]),
        Node::and_pair([8, 3]),
        Node::or_pair([0, 1]),
        Node::implies(7, 10),
        Node::implies(8, 1),
        Node::implies(9, 0),
        Node::implies(1, 0),
        Node::and_pair([0, 2]),
        Node::implies(15, 4),
        Node::implies(0, 2),
    ];
    let mut roots = vec![11, 12, 13, 14, 16, 17];
    for _ in 0..extra {
        let body = nodes.len();
        nodes.push(Node::and_pair([0, 2]));
        roots.push(nodes.len());
        nodes.push(Node::implies(body, 4));
        roots.push(nodes.len());
        nodes.push(Node::implies(0, 4));
    }
    theory(3, nodes, roots)
}

/// Compare with the already admitted ordered incidence rows, not a regenerated
/// formula graph. In particular, chain formation may have absorbed an interior
/// or removed repeated identical leaves before this projection is prepared.
pub(super) fn assert_rows(scratch: &Scratch, owner: &Theory, index: &Narrower) {
    assert!(scratch.owner.as_ref().unwrap().same_instance(owner));
    let projected = &scratch.projected.as_ref().unwrap().parents;
    assert_eq!(projected.len(), owner.nodes().len());
    for node in 0..owner.nodes().len() {
        let expected: Vec<_> = index.parents[node]
            .iter()
            .copied()
            .filter(|&parent| {
                scratch.body_nodes.contains(&parent)
                    && !index.absorbed[parent]
                    && matches!(
                        owner.view().node(parent).unwrap(),
                        NodeView::And(_) | NodeView::Or(_)
                    )
            })
            .collect();
        assert_eq!(projected[node], expected, "source node {node}");
    }
}

#[test]
fn parent_projection_retains_ordered_alias_occurrences() {
    let owner = shared_bodies(2);
    let index = Narrower::new(&owner);
    let producers = producers(&owner);
    let mut scratch = Scratch::default();
    scratch.prepare_projected(&owner, &index, &producers);
    assert_rows(&scratch, &owner, &index);
    assert_eq!(scratch.cyclic_atoms, [0b011]);
    assert!(index.absorbed[6]);
    assert!(!scratch.body_nodes.contains(&6));
    let projected = &scratch.projected.as_ref().unwrap().parents;
    assert_eq!(&projected[0], &[7, 8]);
    assert_eq!(&projected[3], &[7, 9]);
    assert_eq!(&projected[5], &[8], "opaque seed still feeds its OR");
    let aliases: Vec<_> = index.atom_nodes[0]
        .iter()
        .flat_map(|&node| projected[node].iter().copied())
        .collect();
    assert_eq!(aliases, [7, 8, 7, 9]);
    assert!(index.parents[0].len() > projected[0].len());
}

#[test]
fn every_small_mask_matches_original_parent_postings() {
    let owner = shared_bodies(2);
    let index = Narrower::new(&owner);
    let producers = producers(&owner);
    let mut projected = Scratch::default();
    let mut original = Scratch::default();
    for (scratch, blocked) in [(&mut projected, true), (&mut original, false)] {
        let never = vec![if blocked { u64::MAX } else { 0 }; flag_words(owner.nodes().len())];
        scratch
            .complete(
                &owner,
                &index,
                &producers,
                &never,
                &[0],
                &mut Work::new(u64::MAX),
            )
            .unwrap();
        assert!(scratch.projected.is_none(), "first pass uses original rows");
        scratch
            .prepare_bodies(&owner, &index, &producers, &mut Work::new(u64::MAX))
            .unwrap();
        assert_rows(scratch, &owner, &index);
    }
    // Preparation must retain included destinations independently of the
    // completed region's transient false/true state.
    assert!(matches!(projected.states[8], State::Inactive));
    assert!(matches!(original.states[8], State::True));
    assert_rows(&projected, &owner, &index);
    assert_eq!(projected.cyclic_atoms, original.cyclic_atoms);
    assert_eq!(projected.body_nodes, original.body_nodes);
    // Keep the same exact owner, cyclic heads and body cone. Only the parent
    // source differs. An explicit original-row clone also prevents later
    // preparation from materializing filtered rows in the reference.
    original.projected.as_mut().unwrap().parents = index.parents.clone();
    let body_nodes = projected.body_nodes.clone();
    assert_eq!(body_nodes.len(), 7);
    let mut supports_cycle = false;
    let mut excludes_cycle = false;
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
            assert!(projected.projected.is_some());
            assert!(original.projected.is_some());
            assert_eq!(
                projected.atoms, original.atoms,
                "body mask {false_bodies}, atom mask {cut_atoms}"
            );
            assert_eq!(
                projected.missing_in_word(0, cut_atoms),
                original.missing_in_word(0, cut_atoms)
            );
            if cut_atoms == 0 {
                supports_cycle |= projected.atoms == [0b011];
                excludes_cycle |= projected.atoms == [0];
            }
        }
    }
    assert!(supports_cycle && excludes_cycle);
    let reference = &original.projected.as_ref().unwrap().parents;
    assert_eq!(&reference[0], &index.parents[0]);
    assert!(reference[0].len() > projected.projected.as_ref().unwrap().parents[0].len());
}

fn warm(extra: usize, projected: bool) -> u64 {
    let owner = shared_bodies(extra);
    let index = Narrower::new(&owner);
    let producers = producers(&owner);
    let mut scratch = Scratch::default();
    scratch.prepare_projected(&owner, &index, &producers);
    if !projected {
        scratch.projected.as_mut().unwrap().parents = index.parents.clone();
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
    assert_eq!(scratch.atoms, [0b011], "fanout source a actually activates");
    assert_eq!(scratch.body_nodes.len(), 7);
    work.spent
}

#[test]
fn irrelevant_parent_fanout_adds_no_warm_work() {
    assert_eq!(warm(0, true), warm(200, true));
    assert!(
        warm(200, false) > warm(0, false),
        "the reference actually visits the added parent incidences"
    );
}

#[test]
fn public_scratch_bytes_include_projected_payload() {
    let owner = shared_bodies(2);
    let mut scratch = NarrowingScratch::default();
    scratch
        .founded
        .prepare_projected(&owner, &Narrower::new(&owner), &producers(&owner));
    let projected = scratch.founded.projected.as_ref().unwrap();
    assert!(projected.parents.iter().any(|row| !row.is_empty()));
    assert!(projected.heads.iter().any(|row| !row.is_empty()));
    let payload = projected.payload_bytes();
    assert!(payload > 0);
    let before = scratch.retained_bytes();
    let capacities = |founded: &Scratch| {
        [
            founded.states.capacity(),
            founded.atoms.capacity(),
            founded.cyclic_atoms.capacity(),
            founded.queue.capacity(),
            founded.dependencies.capacity(),
            founded.body_nodes.capacity(),
            founded.fact_rows.capacity(),
        ]
    };
    let retained = capacities(&scratch.founded);
    let foreign = shared_bodies(2);
    assert!(!owner.same_instance(&foreign));
    let mut none = Work::new(0);
    // A same-shape owner change clears the obsolete index before the first
    // charged read. Other scratch vectors keep their existing capacities.
    assert_eq!(
        scratch.founded.prepare_bodies(
            &foreign,
            &Narrower::new(&foreign),
            &producers(&foreign),
            &mut none,
        ),
        Err(Stop::WorkLimit)
    );
    assert_eq!(none.spent, 0);
    assert!(scratch.founded.owner.is_none());
    assert!(scratch.founded.projected.is_none());
    assert_eq!(capacities(&scratch.founded), retained);
    assert_eq!(before, scratch.retained_bytes() + payload);
}

#[test]
fn prepared_initialization_matches_fresh_region_masks() {
    let owner = shared_bodies(2);
    let index = Narrower::new(&owner);
    let producers = producers(&owner);
    let mut prepared = Scratch::default();
    prepared.prepare_projected(&owner, &index, &producers);
    let selected = prepared.body_nodes.clone();
    for false_bodies in 0..1_usize << selected.len() {
        let mut never = vec![0; flag_words(owner.nodes().len())];
        for (position, &node) in selected.iter().enumerate() {
            if false_bodies & (1 << position) != 0 {
                never[node / 64] |= 1 << (node % 64);
            }
        }
        for cut_atoms in 0..1_u64 << owner.atom_count() {
            let mut fresh = Scratch::default();
            for scratch in [&mut prepared, &mut fresh] {
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
            assert!(
                fresh.projected.is_none(),
                "reference decodes current formula roles directly"
            );
            assert!(
                prepared.projected.is_some(),
                "warm route reads retained initialization"
            );
            assert_eq!(
                prepared.atoms, fresh.atoms,
                "body mask {false_bodies}, atoms {cut_atoms}"
            );
            assert_eq!(
                prepared.queue, fresh.queue,
                "ordered activated nodes and atoms"
            );
            for &node in &selected {
                assert_eq!(
                    format!("{:?}", prepared.states[node]),
                    format!("{:?}", fresh.states[node]),
                    "node {node}"
                );
            }
        }
    }
}

#[test]
fn boundary_roles_reapply_both_false_masks() {
    let owner = theory(
        3,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::implies(1, 0),
            Node::implies(0, 1),
            Node::implies(2, 0),
        ],
        vec![3, 4, 5],
    );
    let index = Narrower::new(&owner);
    let producers = producers(&owner);
    let mut scratch = Scratch::default();
    scratch.prepare_projected(&owner, &index, &producers);
    assert_eq!(scratch.cyclic_atoms, [0b011]);
    let position = scratch
        .body_nodes
        .iter()
        .position(|&node| node == 2)
        .unwrap();
    assert!(matches!(
        scratch.projected.as_ref().unwrap().initial[position],
        Initial::Boundary(2)
    ));
    // The same retained role alternates between seeded and inactive. Either
    // current mask suppresses it; a previous pass's false state cannot persist.
    for (node_never, atom_never, seeded) in [
        (0, 0, true),
        (0, 0b100, false),
        (0b100, 0, false),
        (0, 0, true),
    ] {
        scratch
            .initialize(
                &owner,
                &index,
                &[node_never],
                &[atom_never],
                &mut Work::new(u64::MAX),
            )
            .unwrap();
        if seeded {
            assert!(matches!(scratch.states[2], State::True));
            assert_eq!(scratch.queue, [owner.atom_count() + 2]);
        } else {
            assert!(matches!(scratch.states[2], State::Inactive));
            assert!(scratch.queue.is_empty());
        }
    }
}
