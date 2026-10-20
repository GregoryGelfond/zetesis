//! A currently false body blocks only paths through that body.

use super::*;

fn example(shared: bool, disjunctive: bool) -> Example {
    let mut nodes: Vec<_> = (0..8).map(Node::atom).collect();
    nodes.extend([
        Node::falsum(),
        Node::implies(6, 8),
        Node::implies(7, 8),
        Node::or_pair([4, 5]),
        Node::and_pair([9, 11]),
    ]);
    let mut roots = vec![4, 5];
    for (body, head) in [(0, 1), (1, 0), (2, 3), (3, 2), (12, 0), (10, 2)] {
        roots.push(nodes.len());
        nodes.push(Node::implies(body, head));
    }
    for (atom, negative) in [(6, 9), (7, 10)] {
        roots.push(nodes.len());
        nodes.push(Node::or_pair([atom, negative]));
    }
    if shared {
        roots.push(nodes.len());
        nodes.push(Node::implies(11, 0));
    }
    if disjunctive {
        let head = nodes.len();
        nodes.push(Node::or_pair([0, 1]));
        roots.push(nodes.len());
        nodes.push(Node::implies(12, head));
    }
    let owner = theory(8, nodes, roots);
    let index = Narrower::new(&owner);
    let producers = crate::producers(&owner, RegionLimits::default(), &Cancellation::default())
        .unwrap()
        .producers
        .unwrap();
    Example {
        owner,
        index,
        producers,
        seeds: [9, 10, 10],
    }
}

fn prepared(example: &Example) -> Scratch {
    let mut scratch = Scratch::default();
    for _ in 0..2 {
        scratch
            .prepare_bodies(
                &example.owner,
                &example.index,
                &example.producers,
                &mut Work::new(u64::MAX),
            )
            .unwrap();
    }
    example
        .complete(&mut scratch, &[0], &[0], &mut Work::new(u64::MAX))
        .unwrap();
    assert_eq!(scratch.atoms[0], 15);
    scratch
}

fn selection(example: &Example, never: &[u64], cuts: &[u64]) -> Scratch {
    let mut scratch = prepared(example);
    example.start(&mut scratch, never);
    assert_eq!(
        scratch
            .local_forward(&example.index, &mut Work::new(u64::MAX))
            .unwrap(),
        2
    );
    let end = scratch.queue.len();
    scratch
        .local_bodies(
            &example.owner,
            &example.index,
            &example.producers,
            end,
            (never, cuts),
            &mut Work::new(u64::MAX),
        )
        .unwrap();
    scratch
}

fn assert_full_result(example: &Example, never: &[u64], cuts: &[u64], atoms: u64) {
    let mut expected = prepared(example);
    example
        .complete(&mut expected, never, cuts, &mut Work::new(u64::MAX))
        .unwrap();
    let mut actual = prepared(example);
    example.start(&mut actual, never);
    example
        .complete(&mut actual, never, cuts, &mut Work::new(u64::MAX))
        .unwrap();
    assert_eq!(actual.local.passes, 1);
    assert_eq!(actual.atoms, expected.atoms);
    assert_eq!(actual.atoms[0], atoms);
}

#[test]
fn false_body_operands_are_not_reverse_selected() {
    let example = example(false, false);
    // Holding control6 falsifies its negation and therefore this conjunction.
    let never = [1 << 9 | 1 << 12];
    let scratch = selection(&example, &never, &[0]);
    assert!(scratch.local.body(12));
    for node in [4, 5, 9, 11] {
        assert!(!scratch.local.body(node), "unneeded descendant {node}");
    }
    assert_full_result(&example, &never, &[0], 12);
}

#[test]
fn another_live_producer_keeps_shared_descendants() {
    let example = example(true, false);
    let never = [1 << 9 | 1 << 12];
    let scratch = selection(&example, &never, &[0]);
    for node in [4, 5, 11, 12] {
        assert!(scratch.local.body(node), "required body node {node}");
    }
    assert!(!scratch.local.body(9));
    assert_full_result(&example, &never, &[0], 15);
}

#[test]
fn cut_heads_omit_only_their_exclusive_producer_bodies() {
    for shared in [false, true] {
        let example = example(false, shared);
        let never = [1];
        let cuts = [1];
        let scratch = selection(&example, &never, &cuts);
        assert!(scratch.local.head(0) && scratch.local.head(1));
        assert!(scratch.local.body(0));
        assert!(!scratch.local.body(1));
        // The disjunctive producer still enters the other, uncut head.
        assert_eq!(scratch.local.body(12), shared);
        assert_full_result(&example, &never, &cuts, if shared { 14 } else { 12 });
    }
}
