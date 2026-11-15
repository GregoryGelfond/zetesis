//! Fresh frozen knowledge on an already projected subset region.
use super::*;

fn from_root(
    theory: &Theory,
    present: &[usize],
    root: Region,
    decisions: u64,
    trace: Option<(&mut depth::QueryDepth, Option<&mut depth::DepthWorkspace>)>,
) -> (Result<Option<Interpretation>, Incomplete>, Statistics) {
    let cancellation = Cancellation::default();
    let candidate = Interpretation::new(theory, present.iter().copied()).unwrap();
    let index = built(theory);
    let mut evaluation = zetesis_ferraris::EvaluationWorkspace::default();
    let evaluated = evaluation
        .evaluate(
            &candidate,
            zetesis_ferraris::EvaluationLimits::default(),
            &cancellation,
        )
        .result
        .unwrap();
    assert!(evaluated.is_model());
    let mut statistics = Statistics::default();
    let mut budget = budget(&cancellation, u64::MAX);
    budget.limits.max_decisions = decisions;
    let result = ReductQuery::new(&index).check_region(
        &evaluated,
        root,
        Limits::default(),
        &mut budget,
        (&mut statistics, &mut NarrowingScratch::default()),
        trace,
    );
    (result, statistics)
}

#[test]
fn a_residual_singleton_is_narrowed_before_leaf_validation() {
    // The last strict-subset step can cut x before the next connective sweep.
    // The only remaining subset is empty; the frozen root requires x.
    let theory = zetesis_theory_support::theories::theory(
        1,
        vec![
            Node::atom(0),
            Node::falsum(),
            Node::implies(1, 1),
            Node::and_pair([0, 2]),
            Node::or_pair([0, 1]),
            Node::or_pair([3, 4]),
        ],
        vec![5],
    );
    let mut empty = Region::all_open(1);
    assert!(empty.cut(0));
    let (check, trace) = from_root(&theory, &[0], empty, u64::MAX, None);
    assert!(matches!(check, Ok(None)));
    assert_eq!(trace.reduct.regions.regions, 1);
    assert_eq!(trace.reduct.regions.refuted, 1);
    assert_eq!(trace.reduct.regions.leaves, 0);
}

#[test]
fn a_countermodel_in_a_residual_singleton_is_validated() {
    // x -> x has the proper-subset reduct model {} under the candidate {x}.
    let theory = zetesis_theory_support::theories::theory(
        1,
        vec![Node::atom(0), Node::implies(0, 0)],
        vec![1],
    );
    let mut empty = Region::all_open(1);
    assert!(empty.cut(0));
    let (check, trace) = from_root(&theory, &[0], empty, u64::MAX, None);
    let Ok(Some(witness)) = check else {
        panic!("the empty subset satisfies the candidate's frozen reduct");
    };
    assert!(witness.theory().same_instance(&theory));
    assert!(witness.atoms().next().is_none());
    assert_eq!(trace.reduct.regions.leaves, 1);
    assert_eq!(trace.countermodels, 1);
}

#[test]
fn the_validated_witness_ends_its_depth_prefix() {
    let theory = zetesis_theory_support::theories::theory(2, vec![], vec![]);
    let root = Region::all_open(2);
    let reference = from_root(&theory, &[0, 1], root.clone(), u64::MAX, None);
    let mut trace = depth::QueryDepth::default();
    let mut worker = depth::DepthWorkspace::new(2, u128::MAX).unwrap();
    let actual = from_root(
        &theory,
        &[0, 1],
        root,
        u64::MAX,
        Some((&mut trace, Some(&mut worker))),
    );
    assert_eq!(actual.1, reference.1);
    let witness = actual.0.unwrap().unwrap();
    let expected = reference.0.unwrap().unwrap();
    assert!(witness.theory().same_instance(expected.theory()));
    assert!(witness.atoms().eq(expected.atoms()));
    assert!(witness.atoms().next().is_none());
    assert_eq!(actual.1.countermodels, 1);
    assert_eq!(actual.1.reduct.regions.regions, 3);
    assert_eq!(worker.levels(), Some([1, 1, 1].as_slice()));
    assert_eq!(
        trace,
        depth::QueryDepth {
            attempted_splits: 2,
            published_splits: 2,
            deepest: Some(2)
        }
    );
}

#[test]
fn decision_refusal_retains_the_actual_visit_prefix() {
    let theory = zetesis_theory_support::theories::theory(2, vec![], vec![]);
    let root = Region::all_open(2);
    let reference = from_root(&theory, &[0, 1], root.clone(), 0, None);
    let mut trace = depth::QueryDepth::default();
    let mut worker = depth::DepthWorkspace::new(2, u128::MAX).unwrap();
    let actual = from_root(
        &theory,
        &[0, 1],
        root,
        0,
        Some((&mut trace, Some(&mut worker))),
    );
    assert_eq!(actual.1, reference.1);
    assert_eq!(reference.0.unwrap_err(), Incomplete::DecisionLimit);
    assert_eq!(actual.0.unwrap_err(), Incomplete::DecisionLimit);
    assert_eq!(actual.1.countermodels, 0);
    assert_eq!(actual.1.reduct.regions.leaves, 1);
    assert_eq!(worker.levels(), Some([1, 1, 1].as_slice()));
    assert_eq!(trace.published_splits, 2);
}

#[test]
fn a_disjunctive_body_does_not_establish_a_particular_head_in_a_residual() {
    // x; x -> (a | b | c); p | q; (a & p) -> g; (a & q) -> g.
    let (condition, first, second, third, left, right, guard) = (0, 1, 2, 3, 4, 5, 6);
    let mut nodes: Vec<_> = (0..7).map(Node::atom).collect();
    nodes.extend([
        Node::or_pair([first, second]),
        Node::or_pair([7, third]),
        Node::implies(condition, 8),
        Node::or_pair([left, right]),
        Node::and_pair([first, left]),
        Node::implies(11, guard),
        Node::and_pair([first, right]),
        Node::implies(13, guard),
    ]);
    let theory = zetesis_theory_support::theories::theory(7, nodes, vec![condition, 9, 10, 12, 14]);
    let mut root = Region::all_open(7);
    assert!(root.cut(guard));
    let (check, _) = from_root(
        &theory,
        &[condition, first, second, third, left, right, guard],
        root,
        u64::MAX,
        None,
    );
    let witness = check.unwrap().expect("the residual contains {x,b,p}");
    assert!(witness.contains(condition) && !witness.contains(first) && !witness.contains(guard));
    assert!(witness.contains(second) || witness.contains(third));
    assert!(witness.contains(left) || witness.contains(right));
}

#[test]
fn a_masked_antecedent_cannot_activate_a_disjunction_in_a_residual() {
    // ((a -> b) | h) -> (y | z); y -> g; z -> g.
    // M omits b and holds a, so its reduct replaces (a -> b) with falsum.
    let (first, second, other_body, left_head, right_head, guard) = (0, 1, 2, 3, 4, 5);
    let mut nodes: Vec<_> = (0..6).map(Node::atom).collect();
    nodes.extend([
        Node::implies(first, second),
        Node::or_pair([6, other_body]),
        Node::or_pair([left_head, right_head]),
        Node::implies(7, 8),
        Node::implies(left_head, guard),
        Node::implies(right_head, guard),
    ]);
    let theory = zetesis_theory_support::theories::theory(6, nodes, vec![9, 10, 11]);
    let mut root = Region::all_open(6);
    assert!(root.cut(first) && root.cut(second) && root.cut(guard));
    let (check, _) = from_root(
        &theory,
        &[first, other_body, left_head, right_head, guard],
        root,
        u64::MAX,
        None,
    );
    let witness = check.unwrap().expect("the residual contains the empty set");
    assert!(witness.atoms().next().is_none());
}

#[test]
fn an_open_body_does_not_establish_a_particular_binary_head_in_a_residual() {
    // t | u and t -> x, u -> x entail x only after an alternative is explored.
    // The open body of x -> (a | b) cannot establish a on its own.
    let (condition, first, second, left, right, guard, branch_one, branch_two) =
        (0, 1, 2, 3, 4, 5, 6, 7);
    let mut nodes: Vec<_> = (0..8).map(Node::atom).collect();
    nodes.extend([
        Node::or_pair([branch_one, branch_two]),
        Node::implies(branch_one, condition),
        Node::implies(branch_two, condition),
        Node::or_pair([first, second]),
        Node::implies(condition, 11),
        Node::or_pair([left, right]),
        Node::and_pair([first, left]),
        Node::implies(14, guard),
        Node::and_pair([first, right]),
        Node::implies(16, guard),
    ]);
    let theory = zetesis_theory_support::theories::theory(8, nodes, vec![8, 9, 10, 12, 13, 15, 17]);
    let mut root = Region::all_open(8);
    assert!(root.cut(guard));
    let (check, _) = from_root(
        &theory,
        &[
            condition, first, second, left, right, guard, branch_one, branch_two,
        ],
        root,
        u64::MAX,
        None,
    );
    let witness = check.unwrap().expect("the residual contains {t,x,b,p}");
    assert!(witness.contains(condition) && witness.contains(second));
    assert!(!witness.contains(first) && !witness.contains(guard));
    assert!(witness.contains(branch_one) || witness.contains(branch_two));
    assert!(witness.contains(left) || witness.contains(right));
}

#[test]
fn a_decided_candidate_root_skips_inner_preparation() {
    let theory = positive();
    let (check, statistics) = from_root(&theory, &[0, 1], Region::all_open(2), u64::MAX, None);
    assert!(matches!(check, Ok(None)));
    assert_eq!(statistics.reduct.regions.regions, 1);
    assert_eq!(statistics.reduct.regions.leaves, 1);
    assert_eq!(statistics.reduct.regions.refuted, 0);
    assert_eq!(statistics.countermodels, 0);
    assert_eq!(
        statistics.reduct.inner_preparation,
        crate::reduct::InnerPreparationStatistics::default()
    );
}

#[test]
fn a_decided_countermodel_skips_inner_preparation() {
    let theory = zetesis_theory_support::theories::theory(
        1,
        vec![Node::atom(0), Node::implies(0, 0)],
        vec![1],
    );
    let mut root = Region::all_open(1);
    assert!(root.cut(0));
    let (check, statistics) = from_root(&theory, &[0], root, u64::MAX, None);
    let witness = check.unwrap().expect("the empty subset is a countermodel");
    assert!(witness.theory().same_instance(&theory));
    assert!(witness.atoms().next().is_none());
    assert_eq!(statistics.countermodels, 1);
    assert_eq!(statistics.reduct.regions.leaves, 1);
    assert_eq!(
        statistics.reduct.inner_preparation,
        crate::reduct::InnerPreparationStatistics::default()
    );
}

#[test]
fn a_served_decided_root_skips_inner_preparation() {
    let theory = positive();
    let index = built(&theory);
    let cancellation = Cancellation::default();
    let candidate = Interpretation::new(&theory, [0, 1]).unwrap();
    let mut evaluation = zetesis_ferraris::EvaluationWorkspace::default();
    let evaluated = evaluation
        .evaluate(
            &candidate,
            zetesis_ferraris::EvaluationLimits::default(),
            &cancellation,
        )
        .result
        .unwrap();
    assert!(evaluated.is_model());
    let (_, narrower) = index.subject(&theory).unwrap();
    let mut entry = Entry::new(Region::all_open(2), CountermodelKnowledge::fresh(narrower));
    let mut statistics = Statistics::default();
    let mut query_budget = budget(&cancellation, u64::MAX);
    let mut preparation = InnerPreparation::Pending;
    let outcome = ReductQuery::new(&index)
        .serve(
            PublishedQuery {
                truth: evaluated.truth(),
                candidate: &candidate,
                preparation: None,
                prepare_root: true,
            },
            &mut entry,
            Limits::default(),
            &mut query_budget,
            (&mut statistics, &mut NarrowingScratch::default()),
            &mut preparation,
        )
        .unwrap();
    assert!(matches!(outcome, Served::Empty));
    assert!(matches!(preparation, InnerPreparation::Prepared(None)));
    assert_eq!(statistics.reduct.regions.regions, 1);
    assert_eq!(statistics.reduct.regions.leaves, 1);
    assert_eq!(statistics.countermodels, 0);
    assert_eq!(
        statistics.reduct.inner_preparation,
        crate::reduct::InnerPreparationStatistics::default()
    );
}

fn positive_query_under_control(
    work: u64,
    cancelled: bool,
) -> (Result<crate::Check, Incomplete>, Statistics, u64) {
    let theory = positive();
    let candidate = Interpretation::new(&theory, [0, 1]).unwrap();
    let index = built(&theory);
    let cancellation = Cancellation::default();
    let mut evaluation = zetesis_ferraris::EvaluationWorkspace::default();
    let evaluated = evaluation
        .evaluate(
            &candidate,
            zetesis_ferraris::EvaluationLimits::default(),
            &cancellation,
        )
        .result
        .unwrap();
    assert!(evaluated.is_model());
    if cancelled {
        cancellation.cancel();
    }
    let mut query_budget = budget(&cancellation, work);
    let mut statistics = Statistics::default();
    let result = ReductQuery::new(&index).check(
        &evaluated,
        Limits::default(),
        &mut query_budget,
        &mut statistics,
        &mut NarrowingScratch::default(),
    );
    (result, statistics, query_budget.statistics.work)
}

#[test]
fn a_cancelled_root_cannot_finish_as_a_leaf() {
    let (result, statistics, work) = positive_query_under_control(u64::MAX, true);
    assert!(matches!(result, Err(Incomplete::Cancelled)));
    assert_eq!(work, 0);
    assert_eq!(
        statistics.reduct.inner_preparation,
        crate::reduct::InnerPreparationStatistics::default()
    );
    assert_eq!(statistics.reduct.regions.regions, 0);
    assert_eq!(statistics.reduct.regions.refuted, 0);
    assert_eq!(statistics.reduct.regions.leaves, 0);
    assert_eq!(statistics.countermodels, 0);
}

#[test]
fn a_decided_root_requires_its_complete_narrowing_allowance() {
    let (result, completed, total) = positive_query_under_control(u64::MAX, false);
    assert!(matches!(result, Ok(crate::Check::Stable)));
    assert_eq!(
        completed.reduct.inner_preparation,
        crate::reduct::InnerPreparationStatistics::default()
    );
    assert_eq!(total, completed.reduct.regions.work);
    assert_eq!(total, 5);
    for allowance in 0..total {
        let (result, statistics, actual) = positive_query_under_control(allowance, false);
        assert!(
            matches!(result, Err(Incomplete::WorkLimit)),
            "{allowance}: {result:?}"
        );
        assert!(actual <= allowance);
        assert_eq!(actual, statistics.reduct.regions.work);
        assert_eq!(
            statistics.reduct.inner_preparation,
            crate::reduct::InnerPreparationStatistics::default()
        );
        assert_eq!(statistics.reduct.regions.regions, 0);
        assert_eq!(statistics.reduct.regions.refuted, 0);
        assert_eq!(statistics.reduct.regions.leaves, 0);
        assert_eq!(statistics.countermodels, 0);
    }
    let (result, exact, actual) = positive_query_under_control(total, false);
    assert!(matches!(result, Ok(crate::Check::Stable)));
    assert_eq!(actual, total);
    assert_eq!(exact, completed);
}
