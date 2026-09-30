//! Carried knowledge and fresh closure agree across atom and node mask words.

use zetesis_cpu::{Cancellation, Stop};
use zetesis_ferraris::{
    EvaluationLimits, EvaluationWorkspace, Interpretation, Knowledge, Narrower, Narrowing,
    NarrowingStatistics, Node, Region, RegionLimits, Theory,
};

const ATOMS: usize = 130;
const CANDIDATE: [usize; 4] = [0, 63, 64, 129];

fn theory() -> Theory {
    // The atom nodes span three words. The roots are in the third node word;
    // cutting 63 must propagate through 64 to 129 in different atom words.
    let mut nodes: Vec<_> = (0..ATOMS).map(Node::Atom).collect();
    nodes.extend([Node::Or(63, 64), Node::Implies(64, 129)]);
    super::theory(ATOMS, nodes, vec![ATOMS, ATOMS + 1])
}

fn with_frozen(action: impl FnOnce(&Theory, Option<&[bool]>)) {
    let theory = theory();
    let candidate = Interpretation::new(&theory, CANDIDATE).unwrap();
    let mut workspace = EvaluationWorkspace::default();
    let cancellation = Cancellation::default();
    let attempt = workspace.evaluate(&candidate, EvaluationLimits::default(), &cancellation);
    let evaluation = attempt.result.unwrap();
    assert!(evaluation.is_model());
    action(&theory, Some(evaluation.truth()));
}

fn narrow(
    theory: &Theory,
    narrower: &Narrower,
    frozen: Option<&[bool]>,
    region: &mut Region,
    knowledge: &mut Knowledge,
    limits: RegionLimits,
) -> Result<(Narrowing, NarrowingStatistics), Stop> {
    let cancellation = Cancellation::default();
    if let Some(truth) = frozen {
        narrower.narrow_frozen_known(theory, truth, region, knowledge, limits, &cancellation)
    } else {
        // No support extraction: unrelated atoms must remain undecided.
        narrower.narrow_known(theory, None, region, knowledge, limits, &cancellation)
    }
}

fn closed_parent(
    theory: &Theory,
    narrower: &Narrower,
    frozen: Option<&[bool]>,
) -> (Region, Knowledge) {
    let mut region = Region::all_open(ATOMS);
    if frozen.is_some() {
        for atom in (0..ATOMS).filter(|atom| !CANDIDATE.contains(atom)) {
            assert!(region.cut(atom));
        }
    }
    assert!(region.hold(0));
    assert!(region.cut(128));
    let mut knowledge = narrower.knowledge();
    let (outcome, _) = narrow(
        theory,
        narrower,
        frozen,
        &mut region,
        &mut knowledge,
        RegionLimits::default(),
    )
    .unwrap();
    assert_eq!(outcome, Narrowing::Fixed { changed: false });
    (region, knowledge)
}

fn decisions(region: &Region) -> Vec<Option<bool>> {
    (0..ATOMS).map(|atom| region.decision(atom)).collect()
}

fn children_match_fresh(theory: &Theory, frozen: Option<&[bool]>) {
    let narrower = Narrower::new(theory);
    let (parent, knowledge) = closed_parent(theory, &narrower, frozen);
    let parent_decisions = decisions(&parent);
    let (cut, held) = parent.split(63);
    for (mut carried, value) in [(cut, false), (held, true)] {
        // Both paths use the same narrower, producer choice and frozen mask.
        // Carried knowledge comes only from this child's closed ancestor.
        let mut inherited = knowledge.clone();
        let mut fresh = carried.clone();
        let carried_outcome = narrow(
            theory,
            &narrower,
            frozen,
            &mut carried,
            &mut inherited,
            RegionLimits::default(),
        )
        .unwrap()
        .0;
        let fresh_outcome = narrow(
            theory,
            &narrower,
            frozen,
            &mut fresh,
            &mut narrower.knowledge(),
            RegionLimits::default(),
        )
        .unwrap()
        .0;
        let mut expected = parent_decisions.clone();
        expected[63] = Some(value);
        if !value {
            expected[64] = Some(true);
            expected[129] = Some(true);
        }
        assert_eq!(carried_outcome, Narrowing::Fixed { changed: !value });
        assert_eq!(fresh_outcome, carried_outcome);
        assert_eq!(decisions(&carried), expected);
        assert_eq!(fresh, carried);
    }
}

fn repeated_closure_is_idle(theory: &Theory, frozen: Option<&[bool]>) {
    let narrower = Narrower::new(theory);
    let (parent, mut knowledge) = closed_parent(theory, &narrower, frozen);
    let (mut child, _) = parent.split(63);
    let (outcome, _) = narrow(
        theory,
        &narrower,
        frozen,
        &mut child,
        &mut knowledge,
        RegionLimits::default(),
    )
    .unwrap();
    assert_eq!(outcome, Narrowing::Fixed { changed: true });
    assert!(child.is_held(64) && child.is_held(129));
    let before = child.clone();
    let (outcome, statistics) = narrow(
        theory,
        &narrower,
        frozen,
        &mut child,
        &mut knowledge,
        RegionLimits::default(),
    )
    .unwrap();
    assert_eq!(outcome, Narrowing::Fixed { changed: false });
    assert_eq!(statistics.propagations, 0);
    assert_eq!(statistics.held, 0);
    assert_eq!(statistics.cut, 0);
    assert_eq!(child, before);
}

fn conflicting_descendant_is_refuted(theory: &Theory, frozen: Option<&[bool]>) {
    let narrower = Narrower::new(theory);
    let (parent, mut knowledge) = closed_parent(theory, &narrower, frozen);
    let (mut carried, _) = parent.split(63);
    assert!(carried.cut(129));
    let mut fresh = carried.clone();
    let carried_outcome = narrow(
        theory,
        &narrower,
        frozen,
        &mut carried,
        &mut knowledge,
        RegionLimits::default(),
    )
    .unwrap()
    .0;
    let fresh_outcome = narrow(
        theory,
        &narrower,
        frozen,
        &mut fresh,
        &mut narrower.knowledge(),
        RegionLimits::default(),
    )
    .unwrap()
    .0;
    assert_eq!(carried_outcome, Narrowing::Refuted);
    assert_eq!(fresh_outcome, carried_outcome);
    assert_eq!(decisions(&fresh), decisions(&carried));
}

fn zero_work_refuses_both_paths(theory: &Theory, frozen: Option<&[bool]>) {
    let narrower = Narrower::new(theory);
    let (parent, mut knowledge) = closed_parent(theory, &narrower, frozen);
    let (mut carried, _) = parent.split(63);
    let before = carried.clone();
    let mut fresh = carried.clone();
    let carried_result = narrow(
        theory,
        &narrower,
        frozen,
        &mut carried,
        &mut knowledge,
        RegionLimits { max_work: 0 },
    );
    let fresh_result = narrow(
        theory,
        &narrower,
        frozen,
        &mut fresh,
        &mut narrower.knowledge(),
        RegionLimits { max_work: 0 },
    );
    assert_eq!(carried_result, Err(Stop::WorkLimit));
    assert_eq!(fresh_result, Err(Stop::WorkLimit));
    assert_eq!(carried, before);
    assert_eq!(fresh, before);
    // Failed knowledge is deliberately not reused, even after a zero-work stop.
}

#[test]
fn original_children_match_fresh_knowledge_across_words() {
    children_match_fresh(&theory(), None);
}

#[test]
fn frozen_children_match_fresh_knowledge_across_words() {
    with_frozen(children_match_fresh);
}

#[test]
fn original_completed_knowledge_propagates_nothing_twice() {
    repeated_closure_is_idle(&theory(), None);
}

#[test]
fn frozen_completed_knowledge_propagates_nothing_twice() {
    with_frozen(repeated_closure_is_idle);
}

#[test]
fn original_conflicting_descendant_matches_fresh_refutation() {
    conflicting_descendant_is_refuted(&theory(), None);
}

#[test]
fn frozen_conflicting_descendant_matches_fresh_refutation() {
    with_frozen(conflicting_descendant_is_refuted);
}

#[test]
fn original_zero_work_refuses_carried_and_fresh_knowledge() {
    zero_work_refuses_both_paths(&theory(), None);
}

#[test]
fn frozen_zero_work_refuses_carried_and_fresh_knowledge() {
    with_frozen(zero_work_refuses_both_paths);
}
