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
