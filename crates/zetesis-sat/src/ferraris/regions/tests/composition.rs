//! A closed subject needs revisiting only after a later subject changes its input.

use super::super::{CandidateKnowledge, Conditions, Narrower, Region, RegionCounts, narrow};
use super::{NARROWING_ATTEMPTS, budget, enumeration};
use crate::ferraris::conditions::Bound;
use crate::{Cancellation, Limits};
use zetesis_ferraris::{
    AdmissionLimits, FormulaParts, Interpretation, Narrowing, NarrowingScratch, Node, Theory,
};

fn theory(nodes: Vec<Node>, roots: Vec<usize>) -> Theory {
    Theory::new(
        3,
        FormulaParts::new(nodes, vec![]).unwrap(),
        roots,
        AdmissionLimits::default(),
    )
    .unwrap()
}

/// `b <- a` and the choice of c. The first root makes a either a fact or a choice.
fn original(fact: bool) -> Theory {
    theory(
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::falsum(),
            Node::implies(0, 3),
            Node::or_pair([0, 4]),
            Node::implies(2, 3),
            Node::or_pair([2, 6]),
            Node::implies(0, 1),
        ],
        vec![if fact { 0 } else { 5 }, 7, 8],
    )
}

fn satisfies(theory: &Theory, mask: usize) -> bool {
    let interpretation =
        Interpretation::new(theory, (0..3).filter(|atom| mask & (1 << atom) != 0)).unwrap();
    zetesis_ferraris::models(
        theory,
        &interpretation,
        zetesis_ferraris::Limits::default(),
        &Cancellation::default(),
    )
    .unwrap()
}

/// Independent satisfaction and reduct checks agree with complete traversal.
fn assert_family(original: &Theory, conditions: &Conditions<(Theory, Narrower)>, region: &Region) {
    let admitted = |mask: &usize| {
        satisfies(original, *mask)
            && conditions
                .iter()
                .all(|(restriction, _)| satisfies(restriction, *mask))
    };
    let classical: Vec<_> = (0..8).filter(admitted).collect();
    assert_eq!(classical, [3, 7], "a and b hold; c remains a choice");
    let represented: Vec<_> = (0..8)
        .filter(|mask| {
            (0..3).all(|atom| {
                let present = mask & (1 << atom) != 0;
                (!region.is_held(atom) || present) && (!region.is_cut(atom) || !present)
            })
        })
        .collect();
    assert_eq!(represented, classical);
    let stable: Vec<_> = classical
        .into_iter()
        .filter(|mask| {
            let interpretation =
                Interpretation::new(original, (0..3).filter(|atom| mask & (1 << atom) != 0))
                    .unwrap();
            zetesis_ferraris::check(
                original,
                &interpretation,
                zetesis_ferraris::Limits::default(),
                &Cancellation::default(),
            )
            .unwrap()
            .accepted()
        })
        .collect();
    let mut search = enumeration(original, 1, false, Limits::default());
    for (restriction, _) in &conditions.permanent {
        search.restrict_candidates(restriction).unwrap();
    }
    if let Some(bound) = &conditions.bound {
        search.tighten_candidate_bound(&bound.index.0).unwrap();
    }
    let mut actual: Vec<usize> = search
        .by_ref()
        .map(|answer| answer.unwrap().atoms().map(|atom| 1 << atom).sum())
        .collect();
    actual.sort_unstable();
    assert!(search.exhausted());
    assert_eq!(stable, [3, 7]);
    assert_eq!(actual, stable);
}

fn close(original: &Theory, conditions: &Conditions<(Theory, Narrower)>) -> (Region, usize) {
    let cancellation = Cancellation::default();
    let mut budget = budget(&cancellation, u64::MAX);
    let narrower = Narrower::new(original);
    let mut region = Region::all_open(3);
    let mut counts = RegionCounts::default();
    let mut knowledge = CandidateKnowledge::default();
    let mut scratch = NarrowingScratch::default();
    let before = NARROWING_ATTEMPTS.get();
    assert_eq!(
        narrow(
            (original, &narrower, None),
            conditions,
            (&mut region, &mut knowledge),
            &mut scratch,
            &mut budget,
            &mut counts,
            None,
        )
        .unwrap(),
        Narrowing::Fixed { changed: true },
    );
    let attempts = NARROWING_ATTEMPTS.get() - before;
    assert_eq!(counts.work, budget.statistics.work);
    assert!(region.is_held(0) && region.is_held(1) && region.is_open(2));
    assert_eq!(region.split_atom(), Some(2));
    // A joint fixed point has no further atom decisions. Reuse the exact
    // knowledge and scratch so stale pending work cannot hide in a fresh state.
    assert_eq!(
        narrow(
            (original, &narrower, None),
            conditions,
            (&mut region, &mut knowledge),
            &mut scratch,
            &mut budget,
            &mut counts,
            None,
        )
        .unwrap(),
        Narrowing::Fixed { changed: false },
    );
    assert_eq!(counts.work, budget.statistics.work);
    (region, attempts)
}

#[test]
fn original_decisions_do_not_repeat_closed_subjects() {
    let original = original(true);
    let tautology = theory(vec![Node::atom(0), Node::implies(0, 0)], vec![1]);
    let conditions = Conditions {
        permanent: vec![],
        bound: Some(Bound::prepare(&tautology, 1).unwrap()),
    };
    let (region, attempts) = close(&original, &conditions);
    // The old joint loop invoked original and bound twice. Neither invocation
    // of that second round did useful work, though both prepared scratch.
    assert_eq!(attempts, 2);
    assert_family(&original, &conditions, &region);
}

#[test]
fn later_decisions_reclose_earlier_subjects() {
    let original = original(false);
    let held_a = theory(vec![Node::atom(0)], vec![0]);
    for conditions in [
        Conditions {
            permanent: vec![(held_a.clone(), Narrower::new(&held_a))],
            bound: None,
        },
        Conditions {
            permanent: vec![],
            bound: Some(Bound::prepare(&held_a, 1).unwrap()),
        },
    ] {
        let (region, attempts) = close(&original, &conditions);
        // The later subject first forces a; only a second original closure
        // can derive b. The unchanged later subject then completes the round.
        assert_eq!(attempts, 4);
        assert_family(&original, &conditions, &region);
    }
}
