//! Alternate support and worker handoffs retain the complete-result boundary.

use super::*;

#[derive(Clone, Copy)]
enum Support {
    Boundary,
    DisjunctiveFact,
    ConditionalChoice,
}

fn alternate(support: Support) -> Example {
    let mut nodes: Vec<_> = (0..6).map(Node::atom).collect();
    nodes.push(Node::falsum());
    nodes.push(Node::implies(4, 6));
    nodes.push(Node::implies(5, 6));
    let mut roots = Vec::new();
    for (body, head) in [(0, 1), (1, 0), (2, 3), (3, 2), (7, 0), (8, 2)] {
        roots.push(nodes.len());
        nodes.push(Node::implies(body, head));
    }
    match support {
        Support::Boundary => {
            roots.push(nodes.len());
            nodes.push(Node::implies(2, 0));
        }
        Support::DisjunctiveFact => {
            roots.push(nodes.len());
            nodes.push(Node::or_pair([0, 1]));
        }
        Support::ConditionalChoice => {
            let negative = nodes.len();
            nodes.push(Node::implies(0, 6));
            let choice = nodes.len();
            nodes.push(Node::or_pair([0, negative]));
            roots.push(nodes.len());
            nodes.push(Node::implies(8, choice));
        }
    }
    let owner = theory(6, nodes, roots);
    let index = Narrower::new(&owner);
    let producers = crate::regions::unmarked_producers(
        &owner,
        RegionLimits::default(),
        &Cancellation::default(),
    )
    .unwrap()
    .producers
    .unwrap();
    Example {
        owner,
        index,
        producers,
        seeds: [7, 8, 8],
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
                &mut Meter::local(u64::MAX),
            )
            .map_err(zetesis_algebra::Failure::into_stop)
            .unwrap();
    }
    let (never, cuts) = example.masks(0, 0);
    example
        .complete(&mut scratch, &never, &cuts, &mut Meter::local(u64::MAX))
        .unwrap();
    assert_eq!(scratch.atoms[0], 15);
    scratch
}

fn check_alternate(support: Support) {
    let example = alternate(support);
    let mut actual = prepared(&example);
    let mut expected = prepared(&example);
    let (never, cuts) = example.masks(1, 0);
    // Every unaffected cyclic atom node in B must be seeded explicitly,
    // even when a sibling left its cell inactive.
    for state in &mut actual.states {
        if !matches!(state, State::Outside) {
            *state = State::Inactive;
        }
    }
    example
        .complete(&mut expected, &never, &cuts, &mut Meter::local(u64::MAX))
        .unwrap();
    example.start(&mut actual, &never);
    example
        .complete(&mut actual, &never, &cuts, &mut Meter::local(u64::MAX))
        .unwrap();
    assert_eq!(actual.local.passes, 1);
    assert_eq!(actual.atoms, expected.atoms);
    assert_eq!(actual.atoms[0], 15);
}

#[test]
fn unaffected_cyclic_boundary_atoms_are_seeds() {
    check_alternate(Support::Boundary);
}

#[test]
fn affected_heads_retain_all_fact_producers() {
    check_alternate(Support::DisjunctiveFact);
}

#[test]
fn affected_heads_retain_conditional_choices() {
    check_alternate(Support::ConditionalChoice);
}

#[test]
fn other_subjects_end_pending_local_collection() {
    use crate::{FrozenSubject, NarrowingScratch, OriginalSubject, Region};
    let example = Example::new(false);
    let (never, cuts) = example.masks(3, 0);
    for frozen in [false, true] {
        let mut scratch = NarrowingScratch {
            common: crate::regions::CommonScratch {
                founded: example.prepared(),
                ..crate::regions::CommonScratch::default()
            },
            ..NarrowingScratch::default()
        };
        example.start(&mut scratch.common.founded, &never);
        let touched = scratch.common.founded.local.touched.len();
        assert!(touched > 0);
        let foreign = theory(9, vec![Node::atom(0)], vec![]);
        let index = Narrower::new(&foreign);
        let mut region = Region::all_open(9);
        let mut quota = Allowance {
            remaining: u64::MAX,
            failure: Stop::WorkLimit,
        };
        let attempt = if frozen {
            crate::test_support::stop_narrowing(&index.narrow_frozen_known_reserved(
                FrozenSubject::new(&foreign, &[true]),
                &mut region,
                &mut index.frozen_knowledge(),
                &mut scratch,
                &Cancellation::default(),
                &mut zetesis_algebra::Meter::reserved(&mut quota),
            ))
        } else {
            crate::test_support::stop_narrowing(&index.propagate_known_reserved(
                OriginalSubject::new(&foreign, None),
                &mut region,
                &mut index.knowledge(),
                &mut scratch,
                &Cancellation::default(),
                &mut zetesis_algebra::Meter::reserved(&mut quota),
            ))
        };
        assert!(matches!(
            attempt.result.unwrap(),
            crate::Narrowing::Fixed { .. }
        ));
        assert_eq!(attempt.statistics.founded_work, 0);
        assert!(
            scratch
                .common
                .founded
                .owner
                .as_ref()
                .is_some_and(|owner| owner.same_instance(&example.owner))
        );
        assert!(scratch.common.founded.projected.is_some());
        assert!(!scratch.common.founded.local.collecting);
        assert_eq!(scratch.common.founded.local.touched.len(), touched);
        // The ended collection belongs to an abandoned region. The next
        // inherited narrowing resets its marks, charged per mark, then
        // collects its own inputs and replays them locally.
        let mut reset = Meter::local(u64::MAX);
        scratch
            .common
            .founded
            .begin_local(&example.owner, true, &mut reset)
            .unwrap();
        assert!(scratch.common.founded.local.collecting);
        assert!(scratch.common.founded.local.touched.is_empty());
        assert_eq!(reset.charged(), touched as u64);
        for node in 0..example.owner.nodes().len() {
            if bit(&never, node) {
                scratch.common.founded.record_false(node);
            }
        }
        example
            .complete(
                &mut scratch.common.founded,
                &never,
                &cuts,
                &mut Meter::local(u64::MAX),
            )
            .unwrap();
        assert_eq!(scratch.common.founded.local.passes, 1);
        assert_eq!(scratch.common.founded.atoms[0] & 63, 48);
        assert!(scratch.common.founded.local.touched.is_empty());
    }
}

#[test]
fn refused_replacement_preserves_retained_membership() {
    let first = Example::new(false);
    let next = Example::new(true);
    let prepare = |scratch: &mut Scratch, work: &mut Meter<'_>| {
        scratch
            .prepare_bodies(&next.owner, &next.index, &next.producers, work)
            .map_err(zetesis_algebra::Failure::into_stop)
    };
    let mut probe = first.prepared();
    prepare(&mut probe, &mut Meter::local(u64::MAX)).unwrap();
    let mut complete = Meter::local(u64::MAX);
    prepare(&mut probe, &mut complete).unwrap();
    let mut actual = first.prepared();
    prepare(&mut actual, &mut Meter::local(u64::MAX)).unwrap();
    let retained = actual.retained_bytes();
    assert!(actual.local.retained_bytes() > 0);
    let mut stopped = Meter::local(complete.charged() - 1);
    assert_eq!(prepare(&mut actual, &mut stopped), Err(Stop::WorkLimit));
    assert!(actual.owner.is_none() && actual.projected.is_none());
    assert_eq!(actual.retained_bytes(), retained);
}
