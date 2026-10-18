//! Original degree breaks ties after joint propagation of candidate restrictions.

use super::super::{CandidateKnowledge, Conditions, Narrower, Region, RegionCounts, narrow};
use super::{budget, theory};
use crate::ferraris::conditions::Bound;
use crate::{Cancellation, Incomplete};
use zetesis_ferraris::{
    ChoiceHeads, Narrowing, NarrowingScratch, Node, OriginalSubject, RegionLimits, Theory,
};

fn original() -> Theory {
    // `(b → c) → a`: its only unknown parent reads b and c, so b ranks first.
    theory(
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::implies(1, 2),
            Node::implies(3, 0),
        ],
        vec![4],
    )
}

fn prefer_a() -> Theory {
    // Two unknown implications read a; b and c each occur in just one.
    // This separates the primary degree from the original's secondary degree.
    theory(
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::implies(0, 1),
            Node::implies(0, 2),
            Node::implies(3, 2),
            Node::implies(4, 1),
        ],
        vec![5, 6],
    )
}

fn prefer_c() -> Theory {
    // `(c → a) → b` and `(c → b) → a` leave c with two unknown parents,
    // while a and b each have one. No root decides an atom here.
    theory(
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::implies(2, 0),
            Node::implies(2, 1),
            Node::implies(3, 1),
            Node::implies(4, 0),
        ],
        vec![5, 6],
    )
}

fn tautology() -> Theory {
    theory(vec![Node::atom(0), Node::implies(0, 0)], vec![1])
}

fn permanent(theory: Theory) -> (Theory, Narrower) {
    let narrower = Narrower::new(&theory);
    (theory, narrower)
}

fn closed(
    conditions: &Conditions<(Theory, Narrower)>,
    limit: u64,
) -> (Result<Narrowing, Incomplete>, Region, RegionCounts, u64) {
    closed_with(&original(), &ChoiceHeads::default(), conditions, limit)
}

/// Narrow the open region under `original`, whose split ranks `choices` first,
/// and `conditions`.
fn closed_with(
    original: &Theory,
    choices: &ChoiceHeads,
    conditions: &Conditions<(Theory, Narrower)>,
    limit: u64,
) -> (Result<Narrowing, Incomplete>, Region, RegionCounts, u64) {
    let narrower = Narrower::new(original);
    let cancellation = Cancellation::default();
    let mut budget = budget(&cancellation, limit);
    let mut counts = RegionCounts::default();
    let mut region = Region::all_open(3);
    let result = narrow(
        (
            OriginalSubject::new(original, None).with_choices(choices),
            &narrower,
        ),
        conditions,
        (&mut region, &mut CandidateKnowledge::default()),
        &mut NarrowingScratch::default(),
        &mut budget,
        &mut counts,
        None,
    );
    (result, region, counts, budget.statistics.work)
}

#[test]
fn the_original_theory_supplies_an_unrestricted_preference() {
    let (result, region, _, _) = closed(&Conditions::default(), u64::MAX);
    assert_eq!(result.unwrap(), Narrowing::Fixed { changed: false });
    assert_eq!(region.split_atom(), Some(1));
}

#[test]
fn original_degree_breaks_a_zero_score_bound_tie() {
    let conditions = Conditions {
        permanent: vec![permanent(prefer_c())],
        bound: Some(
            Bound::prepare(&tautology(), 1, u64::MAX, &crate::Cancellation::default()).unwrap(),
        ),
    };
    let (result, region, _, _) = closed(&conditions, u64::MAX);
    assert_eq!(result.unwrap(), Narrowing::Fixed { changed: false });
    assert_eq!(region.split_atom(), Some(1));
}

#[test]
fn the_latest_permanent_restriction_supplies_the_preference() {
    let conditions = Conditions {
        permanent: vec![permanent(prefer_c()), permanent(prefer_a())],
        bound: None,
    };
    let (result, region, _, _) = closed(&conditions, u64::MAX);
    assert_eq!(result.unwrap(), Narrowing::Fixed { changed: false });
    assert_eq!(region.split_atom(), Some(0));
}

#[test]
fn the_current_bound_supplies_the_preference() {
    // Each subject prefers a different atom, so this distinguishes every
    // branch of the bound/permanent/original policy.
    let conditions = Conditions {
        permanent: vec![permanent(prefer_a())],
        bound: Some(
            Bound::prepare(&prefer_c(), 1, u64::MAX, &crate::Cancellation::default()).unwrap(),
        ),
    };
    let (result, region, _, _) = closed(&conditions, u64::MAX);
    assert_eq!(result.unwrap(), Narrowing::Fixed { changed: false });
    assert_eq!(region.split_atom(), Some(2));
}

#[test]
fn a_restriction_still_propagates_before_split_selection() {
    let conditions = Conditions {
        permanent: vec![permanent(theory(vec![Node::atom(1)], vec![0]))],
        bound: None,
    };
    let (result, region, _, _) = closed(&conditions, u64::MAX);
    assert_eq!(result.unwrap(), Narrowing::Fixed { changed: true });
    assert!(region.is_held(1));
    assert!(region.split_atom().is_some_and(|atom| region.is_open(atom)));
}

#[test]
fn joint_closure_charges_one_composed_preference_scan() {
    let original = original();
    let bound = tautology();
    let mut separate_work = 0;
    for subject in [&original, &bound] {
        let narrower = Narrower::new(subject);
        let (result, statistics) = narrower
            .narrow_known(
                OriginalSubject::new(subject, None),
                &mut Region::all_open(3),
                &mut narrower.knowledge(),
                &mut NarrowingScratch::default(),
                RegionLimits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(result, Narrowing::Fixed { changed: false });
        separate_work += statistics.work;
    }
    let conditions = Conditions {
        permanent: vec![],
        bound: Some(Bound::prepare(&bound, 1, u64::MAX, &crate::Cancellation::default()).unwrap()),
    };
    let (result, _, counts, work) = closed(&conditions, u64::MAX);
    result.unwrap();
    assert_eq!(counts.work, work);
    // Separate narrowing reads three degrees per subject. The joint selector
    // reads both degrees for each of the same three open atoms. Propagation
    // itself spends the same prefix, so the complete receipts agree.
    assert_eq!(work, separate_work);
}

#[test]
fn a_selector_refusal_retains_the_shared_budget_prefix() {
    let conditions = Conditions::<(Theory, Narrower)>::default();
    let (result, _, complete, work) = closed(&conditions, u64::MAX);
    result.unwrap();
    assert_eq!(complete.work, work);
    // All three original atoms are still open. Refuse the final selector read.
    let (result, region, incomplete, work) = closed(&conditions, work - 1);
    assert_eq!(result, Err(Incomplete::WorkLimit));
    assert_eq!(incomplete.work, complete.work - 1);
    assert_eq!(incomplete.work, work);
    assert_eq!(region.open().collect::<Vec<_>>(), [0, 1, 2]);
    assert_eq!(
        region.split_atom(),
        Some(2),
        "no partial preference is installed"
    );
}

#[test]
fn a_bound_selector_refusal_retains_the_shared_budget_prefix() {
    let conditions = Conditions {
        permanent: vec![],
        bound: Some(
            Bound::prepare(&tautology(), 1, u64::MAX, &crate::Cancellation::default()).unwrap(),
        ),
    };
    let (result, _, complete, work) = closed(&conditions, u64::MAX);
    result.unwrap();
    assert_eq!(complete.work, work);
    // The selected bound and original each supply one count per open atom.
    // Each cutoff is inside that final six-read scan; all subjects completed
    // propagation without ranking.
    for missing in 1..=6 {
        let (result, region, prefix, work) = closed(&conditions, complete.work - missing);
        assert_eq!(result, Err(Incomplete::WorkLimit));
        assert_eq!(prefix.work, complete.work - missing);
        assert_eq!(prefix.work, work);
        assert_eq!(
            region.split_atom(),
            Some(2),
            "no partial preference is installed"
        );
    }
}

fn choose_c() -> Theory {
    // `c ∨ ¬c` alone: c heads an atomic choice and no atom is decided.
    theory(
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::falsum(),
            Node::implies(2, 3),
            Node::or_pair([2, 4]),
        ],
        vec![5],
    )
}

/// The split of the open region under the choice theory and `conditions`,
/// with and without the original theory's choice heads.
fn composed_splits(conditions: &Conditions<(Theory, Narrower)>) -> (Option<usize>, Option<usize>) {
    let original = choose_c();
    let choices =
        zetesis_ferraris::producers(&original, RegionLimits::default(), &Cancellation::default())
            .unwrap()
            .choice_heads;
    let split = |choices: &ChoiceHeads| {
        let (result, region, _, _) = closed_with(&original, choices, conditions, u64::MAX);
        assert_eq!(result.unwrap(), Narrowing::Fixed { changed: false });
        region.split_atom()
    };
    (split(&choices), split(&ChoiceHeads::default()))
}

#[test]
fn original_choice_heads_outrank_a_restrictions_busier_atom() {
    let conditions = Conditions {
        permanent: vec![permanent(prefer_a())],
        bound: None,
    };
    // Without choices the restriction's counts select a; with them, c.
    assert_eq!(composed_splits(&conditions), (Some(2), Some(0)));
}

#[test]
fn original_choice_heads_outrank_a_bounds_busier_atom() {
    let conditions = Conditions {
        permanent: vec![],
        bound: Some(
            Bound::prepare(&prefer_a(), 1, u64::MAX, &crate::Cancellation::default()).unwrap(),
        ),
    };
    assert_eq!(composed_splits(&conditions), (Some(2), Some(0)));
}

#[test]
fn the_region_search_settles_the_choice_before_the_loop() {
    // Held branches come first. Splitting on the choice head finishes both
    // loop answers with a0 before either without it. The order of the two
    // loop alternatives is independent of this choice-priority proposition.
    let theory = crate::ferraris::fixtures::choice_beside_a_loop();
    let cancellation = Cancellation::default();
    let mut budget = budget(&cancellation, u64::MAX);
    let mut search =
        super::super::RegionSearch::new(&theory, u64::MAX, u64::MAX, &mut budget).unwrap();
    let mut owner = crate::ferraris::original_index::OriginalIndex::new(&theory, u64::MAX);
    let index = owner.ensure(&cancellation, |_| Ok(())).unwrap();
    let mut proposed = Vec::new();
    for _ in 0..2 {
        let leaf = search
            .propose(&theory, index, &mut budget, &mut None)
            .unwrap()
            .unwrap();
        proposed.push(leaf.atoms().collect::<Vec<_>>());
    }
    proposed.sort();
    assert_eq!(proposed, [vec![0, 1], vec![0, 2]]);
}

#[test]
fn active_restrictions_keep_primary_preference() {
    // Choices form b -> a -> c, opposing each restriction's preferred atom.
    let original = theory(
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::falsum(),
            Node::implies(0, 3),
            Node::or_pair([0, 4]),
            Node::implies(1, 3),
            Node::or_pair([1, 6]),
            Node::implies(2, 3),
            Node::or_pair([2, 8]),
            Node::implies(1, 5),
            Node::implies(0, 9),
        ],
        vec![7, 10, 11],
    );
    let cancellation = Cancellation::default();
    let mut opening = budget(&cancellation, u64::MAX);
    let opened = super::super::open(&original, u64::MAX, &mut opening).unwrap();
    assert!(opened.parts.subject(&original).generator_order().is_some());
    let restriction = Conditions {
        permanent: vec![permanent(prefer_a())],
        bound: None,
    };
    let bound = Conditions {
        permanent: vec![permanent(prefer_a())],
        bound: Some(Bound::prepare(&prefer_c(), 1, u64::MAX, &cancellation).unwrap()),
    };
    let narrower = Narrower::new(&original);
    for (conditions, expected) in [(Conditions::default(), 1), (restriction, 0), (bound, 2)] {
        let mut region = Region::all_open(3);
        let mut allowance = budget(&cancellation, u64::MAX);
        let result = narrow(
            (opened.parts.subject(&original), &narrower),
            &conditions,
            (&mut region, &mut CandidateKnowledge::default()),
            &mut NarrowingScratch::default(),
            &mut allowance,
            &mut RegionCounts::default(),
            None,
        );
        assert_eq!(result.unwrap(), Narrowing::Fixed { changed: false });
        assert_eq!(region.split_atom(), Some(expected));
    }
}
