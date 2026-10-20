//! Current false destinations stop discovery; recorded changes do not.

use super::*;

fn example(alternative: bool, padding: usize) -> (Example, usize) {
    let mut nodes: Vec<_> = (0..9).map(Node::atom).collect();
    nodes.push(Node::falsum());
    let seeds = [10, 11, 12];
    for control in 6..9 {
        nodes.push(Node::implies(control, 9));
    }
    nodes.extend((0..padding).map(|_| Node::falsum()));
    let barrier = nodes.len();
    nodes.push(Node::and_pair([10, 11]));
    let mut roots = Vec::new();
    for (control, seed) in (6..9).zip(seeds) {
        roots.push(nodes.len());
        nodes.push(Node::or_pair([control, seed]));
    }
    for (body, head) in [
        (0, 1),
        (1, 0),
        (2, 3),
        (3, 2),
        (4, 5),
        (5, 4),
        (barrier, 0),
        (11, 2),
        (12, 4),
    ] {
        roots.push(nodes.len());
        nodes.push(Node::implies(body, head));
    }
    if alternative {
        roots.push(nodes.len());
        nodes.push(Node::implies(12, 0));
    }
    let owner = theory(9, nodes, roots);
    let index = Narrower::new(&owner);
    let producers = crate::producers(&owner, RegionLimits::default(), &Cancellation::default())
        .unwrap()
        .producers
        .unwrap();
    (
        Example {
            owner,
            index,
            producers,
            seeds,
        },
        barrier,
    )
}

#[test]
fn old_false_parents_are_not_rediscovered() {
    let (example, barrier) = example(true, 70);
    let mut actual = example.prepared();
    let (mut never, cuts) = example.masks(0, 0);
    never[barrier / 64] |= 1 << (barrier % 64);
    example
        .complete(&mut actual, &never, &cuts, &mut Work::new(u64::MAX))
        .unwrap();
    assert_eq!(actual.atoms[0], 63);
    // The barrier already failed in the parent. Only its newly false operand
    // belongs to this interval; the alternative producer still supports 0/1.
    never[0] |= 1 << 10;
    actual.begin_local(&example.owner, true);
    actual.record_false(10);
    assert_eq!(
        actual
            .local_forward(&example.index, (&never, &cuts), &mut Work::new(u64::MAX))
            .unwrap(),
        0
    );
    assert_eq!(actual.queue, vec![example.owner.atom_count() + 10]);
    assert_eq!(actual.local.marks[actual.local.atoms + barrier], 0);
    let mut expected = example.prepared();
    example
        .complete(&mut expected, &never, &cuts, &mut Work::new(u64::MAX))
        .unwrap();
    example
        .complete(&mut actual, &never, &cuts, &mut Work::new(u64::MAX))
        .unwrap();
    assert_eq!(actual.local.passes, 1);
    assert_eq!(actual.atoms, expected.atoms);
}

#[test]
fn newly_false_parents_keep_their_input_events() {
    let (example, barrier) = example(false, 70);
    let (mut never, cuts) = example.masks(1, 0);
    never[barrier / 64] |= 1 << (barrier % 64);
    for inputs in [[10, barrier], [barrier, 10]] {
        let mut actual = example.prepared();
        actual.begin_local(&example.owner, true);
        for node in inputs.into_iter().chain(inputs) {
            actual.record_false(node);
        }
        assert_eq!(actual.queue.len(), 2);
        assert_eq!(
            actual
                .local_forward(&example.index, (&never, &cuts), &mut Work::new(u64::MAX))
                .unwrap(),
            2
        );
        assert!(actual.local.head(0) && actual.local.head(1));
        example
            .complete(&mut actual, &never, &cuts, &mut Work::new(u64::MAX))
            .unwrap();
        assert_eq!(actual.local.passes, 1);
        assert_eq!(actual.atoms[0], 60);
    }
}

#[test]
fn cut_occurrences_still_reach_live_heads() {
    let (example, _) = example(false, 0);
    let (never, cuts) = example.masks(0, 1);
    let mut actual = example.prepared();
    example.start(&mut actual, &never);
    assert_eq!(
        actual
            .local_forward(&example.index, (&never, &cuts), &mut Work::new(u64::MAX))
            .unwrap(),
        1
    );
    assert!(!actual.local.head(0) && actual.local.head(1));
    example
        .complete(&mut actual, &never, &cuts, &mut Work::new(u64::MAX))
        .unwrap();
    let mut expected = example.prepared();
    example
        .complete(&mut expected, &never, &cuts, &mut Work::new(u64::MAX))
        .unwrap();
    assert_eq!(actual.local.passes, 1);
    assert_eq!(actual.atoms, expected.atoms);
    assert_eq!(actual.atoms[0], 60);
}

#[test]
fn all_live_heads_affected_keeps_the_full_fallback() {
    let (example, _) = example(false, 0);
    for cuts in [48, 63] {
        let (never, cuts) = example.masks(3, cuts);
        let mut actual = example.prepared();
        example.start(&mut actual, &never);
        example
            .complete(&mut actual, &never, &cuts, &mut Work::new(u64::MAX))
            .unwrap();
        assert_eq!(actual.local.passes, 0);
        assert_eq!(actual.atoms[0], 0);
        assert!(actual.local.touched.is_empty());
    }
}

#[test]
fn stopped_filtered_replay_requires_a_complete_retry() {
    let (example, barrier) = example(true, 0);
    let (mut parent_never, cuts) = example.masks(0, 0);
    parent_never[barrier / 64] |= 1 << (barrier % 64);
    let prepare = || {
        let mut scratch = example.prepared();
        example
            .complete(&mut scratch, &parent_never, &cuts, &mut Work::new(u64::MAX))
            .unwrap();
        assert_eq!(scratch.atoms[0], 63);
        scratch.begin_local(&example.owner, true);
        scratch.record_false(12);
        scratch
    };
    let mut never = parent_never.clone();
    never[0] |= 1 << 12;
    let mut expected = example.prepared();
    example
        .complete(&mut expected, &never, &cuts, &mut Work::new(u64::MAX))
        .unwrap();
    assert_eq!(expected.atoms[0], 12);
    let mut measured = prepare();
    let mut completed = Work::new(u64::MAX);
    example
        .complete(&mut measured, &never, &cuts, &mut completed)
        .unwrap();
    assert_eq!(measured.local.passes, 1);
    assert_eq!(measured.atoms, expected.atoms);
    for failure in [
        Stop::WorkLimit,
        Stop::Cancelled,
        Stop::Deadline,
        Stop::Allocation,
    ] {
        for ceiling in 0..completed.spent {
            let mut actual = prepare();
            let mut allowance = Allowance {
                remaining: ceiling,
                failure,
            };
            let mut work = Work::reserved(&mut allowance);
            assert_eq!(
                example.complete(&mut actual, &never, &cuts, &mut work),
                Err(failure)
            );
            assert!(!actual.local.collecting);
            assert_eq!(actual.local.passes, 0);
            work.refund();
            example
                .complete(&mut actual, &never, &cuts, &mut Work::new(u64::MAX))
                .unwrap();
            assert_eq!(actual.local.passes, 0);
            assert_eq!(actual.atoms, expected.atoms);
            assert!(actual.local.touched.is_empty());
        }
    }
}
