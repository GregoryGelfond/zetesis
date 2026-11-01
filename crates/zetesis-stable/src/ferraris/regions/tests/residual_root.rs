//! Fresh frozen knowledge on an already projected subset region.
use super::*;

fn from_root(
    theory: &Theory,
    present: &[usize],
    root: Region,
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
    let result = ReductQuery::new(&index).check_region(
        &evaluated,
        root,
        Limits::default(),
        &mut budget(&cancellation, u64::MAX),
        (&mut statistics, &mut NarrowingScratch::default()),
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
    let (check, trace) = from_root(&theory, &[0], empty);
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
    let (check, trace) = from_root(&theory, &[0], empty);
    let Ok(Some(witness)) = check else {
        panic!("the empty subset satisfies the candidate's frozen reduct");
    };
    assert!(witness.theory().same_instance(&theory));
    assert!(witness.atoms().next().is_none());
    assert_eq!(trace.reduct.regions.leaves, 1);
    assert_eq!(trace.countermodels, 1);
}
