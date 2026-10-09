//! Joint propagation keeps the established last-subject split preference.

use super::super::{CandidateKnowledge, Conditions, Narrower, Region, RegionCounts, narrow};
use super::budget;
use crate::ferraris::conditions::Bound;
use crate::{Cancellation, Incomplete};
use zetesis_ferraris::{
    AdmissionLimits, FormulaParts, Narrowing, NarrowingScratch, Node, OriginalSubject,
    RegionLimits, Theory,
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
    // `(a → c) → b` leaves every atom open and ranks a first.
    theory(
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::implies(0, 2),
            Node::implies(3, 1),
        ],
        vec![4],
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
    let original = original();
    let narrower = Narrower::new(&original);
    let cancellation = Cancellation::default();
    let mut budget = budget(&cancellation, limit);
    let mut counts = RegionCounts::default();
    let mut region = Region::all_open(3);
    let result = narrow(
        (&original, &narrower, None),
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
fn a_zero_score_bound_prefers_the_lowest_open_atom() {
    let conditions = Conditions {
        permanent: vec![permanent(prefer_c())],
        bound: Some(Bound::prepare(&tautology(), 1).unwrap()),
    };
    let (result, region, _, _) = closed(&conditions, u64::MAX);
    assert_eq!(result.unwrap(), Narrowing::Fixed { changed: false });
    assert_eq!(region.split_atom(), Some(0));
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
        bound: Some(Bound::prepare(&prefer_c(), 1).unwrap()),
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
fn joint_closure_charges_one_preference_scan() {
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
        bound: Some(Bound::prepare(&bound, 1).unwrap()),
    };
    let (result, _, counts, work) = closed(&conditions, u64::MAX);
    result.unwrap();
    assert_eq!(counts.work, work);
    // Both independent narrowings scan three open atoms. Joint closure
    // retains only the bound's scan, saving exactly the original's three reads.
    assert_eq!(work + 3, separate_work);
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
        bound: Some(Bound::prepare(&tautology(), 1).unwrap()),
    };
    let (result, _, complete, work) = closed(&conditions, u64::MAX);
    result.unwrap();
    assert_eq!(complete.work, work);
    // The selected bound scans three open atoms. Each cutoff is inside that
    // final scan; the other subjects completed propagation without ranking.
    for missing in 1..=3 {
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
