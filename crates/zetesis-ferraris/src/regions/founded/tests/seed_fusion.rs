//! Direct seeds preserve the queued closure and complete current-mask resets.

use super::*;
use crate::{AdmissionLimits, ExtremumCutRule, ExtremumRow, FormulaNodes, FormulaParts, NodeView};

fn mixed_seeds() -> Theory {
    let mut nodes =
        FormulaNodes::new(FormulaParts::new((0..3).map(Node::atom).collect(), Vec::new()).unwrap());
    let mut append = nodes.transaction();
    let falsum = append
        .push(NodeView::False, usize::MAX, usize::MAX)
        .unwrap();
    let negative = append
        .push(NodeView::Implies(2, falsum), usize::MAX, usize::MAX)
        .unwrap();
    let empty = append
        .push(NodeView::And(&[]), usize::MAX, usize::MAX)
        .unwrap();
    let threshold = append
        .push_threshold_family(&[2], &[2], &[1], usize::MAX, usize::MAX)
        .unwrap()[0];
    let extremum = append
        .push_extremum_cut_family_range(
            ExtremumRow {
                conditions: &[2],
                ranks: &[1],
                rank_count: 3,
            },
            &[ExtremumCutRule::Prefix(2)],
            usize::MAX,
            usize::MAX,
        )
        .unwrap()
        .start;
    let body = append
        .push(
            NodeView::Or(&[2, negative, empty, threshold, extremum]),
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
    let roots = vec![
        append
            .push(NodeView::Implies(body, 0), usize::MAX, usize::MAX)
            .unwrap(),
        append
            .push(NodeView::Implies(0, 1), usize::MAX, usize::MAX)
            .unwrap(),
        append
            .push(NodeView::Implies(1, 0), usize::MAX, usize::MAX)
            .unwrap(),
    ];
    append.commit();
    Theory::new(3, nodes.into_parts(), roots, AdmissionLimits::default()).unwrap()
}

fn queued_projection(owner: &Theory, index: &Narrower, producers: &Producers) -> Scratch {
    let mut reference = Scratch::default();
    reference.prepare_projected(owner, index, producers);
    let plan = reference.projected.as_mut().unwrap();
    // Test-only reference: reset every role and put true seeds on the original
    // queue. No production mode or second closure algorithm is introduced.
    plan.seed_start = plan.initial.len();
    reference
}

fn assert_same_closure(actual: &Scratch, expected: &Scratch) {
    assert_eq!(actual.atoms, expected.atoms);
    for &node in &actual.body_nodes {
        assert_eq!(
            format!("{:?}", actual.states[node]),
            format!("{:?}", expected.states[node]),
            "node {node}"
        );
    }
}

#[test]
fn all_small_masks_match_the_queued_seed_schedule() {
    for owner in [mixed_seeds(), projected_parents::shared_bodies(2)] {
        let index = Narrower::new(&owner);
        let producers = producers(&owner);
        let mut actual = Scratch::default();
        actual.prepare_projected(&owner, &index, &producers);
        let mut expected = queued_projection(&owner, &index, &producers);
        let selected = actual.body_nodes.clone();
        let seed_start = actual.projected.as_ref().unwrap().seed_start;
        let seeds = selected[seed_start..].to_vec();
        assert!(!seeds.is_empty());
        assert!(
            actual.projected.as_ref().unwrap().initial[..seed_start]
                .iter()
                .all(|role| !role.is_seed())
        );
        assert!(
            actual.projected.as_ref().unwrap().initial[seed_start..]
                .iter()
                .all(|role| role.is_seed())
        );
        // Vary all included false formulas independently of cut atoms, including
        // masks not produced by ordinary Boolean propagation. Seed eligibility
        // must use both current masks, never a previous region's truth.
        for false_bodies in 0..1_usize << selected.len() {
            let mut never = vec![0; flag_words(owner.nodes().len())];
            for (position, &node) in selected.iter().enumerate() {
                if false_bodies & (1 << position) != 0 {
                    never[node / 64] |= 1 << (node % 64);
                }
            }
            for cut in 0..1_u64 << owner.atom_count() {
                let mut work = Meter::local(u64::MAX);
                actual
                    .complete(&owner, &index, &producers, &never, &[cut], &mut work)
                    .map_err(zetesis_algebra::Failure::into_stop)
                    .unwrap();
                let mut reference_work = Meter::local(u64::MAX);
                expected
                    .complete(
                        &owner,
                        &index,
                        &producers,
                        &never,
                        &[cut],
                        &mut reference_work,
                    )
                    .map_err(zetesis_algebra::Failure::into_stop)
                    .unwrap();
                assert_same_closure(&actual, &expected);
                let active = seeds
                    .iter()
                    .filter(|&&node| matches!(actual.states[node], State::True))
                    .count();
                assert_eq!(
                    reference_work.charged() - work.charged(),
                    2 * u64::try_from(active).unwrap()
                );
                assert!(
                    seeds
                        .iter()
                        .all(|node| !actual.queue.contains(&(owner.atom_count() + node)))
                );
                assert_eq!(
                    actual.missing_in_word(0, cut),
                    expected.missing_in_word(0, cut)
                );
            }
        }
    }
}

#[test]
fn seed_partition_uses_existing_storage_and_preserves_roles() {
    let owner = mixed_seeds();
    let index = Narrower::new(&owner);
    let producers = producers(&owner);
    let mut scratch = Scratch::default();
    scratch.prepare_projected(&owner, &index, &producers);
    let selected = scratch.body_nodes.clone();
    let plan = scratch.projected.as_ref().unwrap();
    for (position, &node) in selected.iter().enumerate() {
        let expected = Initial::read(&owner, &index, &scratch.cyclic_atoms, node).unwrap();
        assert_eq!(
            format!("{:?}", plan.initial[position]),
            format!("{expected:?}")
        );
    }
    assert!(
        plan.initial
            .iter()
            .any(|role| matches!(role, Initial::Boundary(_)))
    );
    assert!(
        plan.initial
            .iter()
            .any(|role| matches!(role, Initial::True))
    );
    for native in [true, false] {
        assert!(selected.iter().enumerate().any(|(position, &node)| {
            let matches_kind = if native {
                matches!(owner.view().node(node).unwrap(), NodeView::Threshold(_))
            } else {
                matches!(owner.view().node(node).unwrap(), NodeView::ExtremumCut(_))
            };
            matches_kind && matches!(plan.initial[position], Initial::True)
        }));
    }
    // Empty conjunctions are normalized by the formula builder. Exercise the
    // defensive zero-dependency role directly in the immutable partition.
    let original = [
        Initial::Awaiting(2),
        Initial::True,
        Initial::Awaiting(0),
        Initial::Inactive,
    ];
    let mut roles = original;
    let mut ids = [0, 1, 2, 3];
    let split = initialization::partition(&mut ids, &mut roles, &mut Meter::local(u64::MAX))
        .map_err(zetesis_algebra::Failure::into_stop)
        .unwrap();
    assert_eq!(split, 2);
    for (position, &node) in ids.iter().enumerate() {
        assert_eq!(
            format!("{:?}", roles[position]),
            format!("{:?}", original[node])
        );
        assert_eq!(roles[position].is_seed(), position >= split);
    }
    let expected = plan.parents.retained_bytes() - size_of::<Adjacency>() as u128
        + plan.heads.retained_bytes()
        - size_of::<Adjacency>() as u128
        + plan.initial.capacity() as u128 * size_of::<Initial>() as u128;
    assert_eq!(plan.payload_bytes(), expected, "no additional seed vector");
}

#[test]
fn interrupted_seed_replay_rebuilds_every_current_state() {
    let owner = mixed_seeds();
    let index = Narrower::new(&owner);
    let producers = producers(&owner);
    let never = vec![0; flag_words(owner.nodes().len())];
    let mut expected = Scratch::default();
    expected.prepare_projected(&owner, &index, &producers);
    let mut completed = Meter::local(u64::MAX);
    expected
        .complete(&owner, &index, &producers, &never, &[0], &mut completed)
        .map_err(zetesis_algebra::Failure::into_stop)
        .unwrap();
    for failure in [Stop::WorkLimit, Stop::Cancelled, Stop::Deadline] {
        for ceiling in 0..completed.charged() {
            let mut actual = Scratch::default();
            actual.prepare_projected(&owner, &index, &producers);
            let mut allowance = crate::test_support::Allowance {
                remaining: ceiling,
                failure,
            };
            let mut work = Meter::reserved(&mut allowance);
            assert_eq!(
                actual
                    .complete(&owner, &index, &producers, &never, &[0], &mut work)
                    .map_err(zetesis_algebra::Failure::into_stop),
                Err(failure)
            );
            assert_eq!(work.charged(), ceiling);
            work.refund();
            let mut retry = Meter::local(u64::MAX);
            actual
                .complete(&owner, &index, &producers, &never, &[0], &mut retry)
                .map_err(zetesis_algebra::Failure::into_stop)
                .unwrap();
            assert_same_closure(&actual, &expected);
            assert_eq!(retry.charged(), completed.charged());
        }
    }
}

#[test]
fn interrupted_partition_has_no_owner_authority() {
    let owner = mixed_seeds();
    let index = Narrower::new(&owner);
    let producers = producers(&owner);
    let mut expected = Scratch::default();
    expected
        .prepare_bodies(&owner, &index, &producers, &mut Meter::local(u64::MAX))
        .map_err(zetesis_algebra::Failure::into_stop)
        .unwrap();
    let mut full = Meter::local(u64::MAX);
    expected
        .prepare_bodies(&owner, &index, &producers, &mut full)
        .map_err(zetesis_algebra::Failure::into_stop)
        .unwrap();
    // Stop at every prefix of the final partition, including its charged swap
    // reads. A separate copy supplies its cost without depending on row order.
    let mut before = Scratch::default();
    before
        .prepare_bodies(&owner, &index, &producers, &mut Meter::local(u64::MAX))
        .map_err(zetesis_algebra::Failure::into_stop)
        .unwrap();
    let mut roles = initialization::prepare(
        &owner,
        &index,
        &before.body_nodes,
        &before.cyclic_atoms,
        &mut Meter::local(u64::MAX),
    )
    .map_err(zetesis_algebra::Failure::into_stop)
    .unwrap();
    let mut partition = Meter::local(u64::MAX);
    initialization::partition(&mut before.body_nodes, &mut roles, &mut partition)
        .map_err(zetesis_algebra::Failure::into_stop)
        .unwrap();
    let start = full.charged() - partition.charged();
    for cutoff in start..full.charged() {
        let mut actual = Scratch::default();
        actual
            .prepare_bodies(&owner, &index, &producers, &mut Meter::local(u64::MAX))
            .map_err(zetesis_algebra::Failure::into_stop)
            .unwrap();
        assert_eq!(
            actual
                .prepare_bodies(&owner, &index, &producers, &mut Meter::local(cutoff))
                .map_err(zetesis_algebra::Failure::into_stop),
            Err(Stop::WorkLimit)
        );
        assert!(actual.owner.is_none());
        assert!(actual.projected.is_none());
        actual.prepare_projected(&owner, &index, &producers);
        assert_eq!(actual.body_nodes, expected.body_nodes);
        for (left, right) in actual
            .projected
            .as_ref()
            .unwrap()
            .initial
            .iter()
            .zip(&expected.projected.as_ref().unwrap().initial)
        {
            assert_eq!(format!("{left:?}"), format!("{right:?}"));
        }
    }
}

#[test]
fn seed_plans_rebuild_across_different_exact_owners() {
    let first = mixed_seeds();
    let other = projected_parents::shared_bodies(2);
    let mut reused = Scratch::default();
    for owner in [&first, &other, &first] {
        let index = Narrower::new(owner);
        let producers = producers(owner);
        reused.prepare_projected(owner, &index, &producers);
        let mut expected = queued_projection(owner, &index, &producers);
        for blocked in [true, false, true] {
            let never = vec![if blocked { u64::MAX } else { 0 }; flag_words(owner.nodes().len())];
            for scratch in [&mut reused, &mut expected] {
                scratch
                    .complete(
                        owner,
                        &index,
                        &producers,
                        &never,
                        &[0],
                        &mut Meter::local(u64::MAX),
                    )
                    .map_err(zetesis_algebra::Failure::into_stop)
                    .unwrap();
            }
            assert_same_closure(&reused, &expected);
        }
    }
}
