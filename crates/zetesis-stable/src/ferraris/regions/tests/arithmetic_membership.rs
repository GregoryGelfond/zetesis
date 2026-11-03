use super::*;
use crate::ferraris::regions::arithmetic_counts::ArithmeticCounts;

#[test]
fn an_original_parity_unit_does_not_enter_the_frozen_query() {
    // a <- a. :- not a. Original parity preparation retains the unit a;
    // the self-supporting definition avoids the unrelated unheaded-a=false
    // completion. At {a} the reduct is a -> a and true, so {} is a countermodel.
    // Carrying the original unit into membership would lose that countermodel.
    let theory = zetesis_theory_support::theories::theory(
        1,
        vec![
            Node::atom(0),
            Node::falsum(),
            Node::implies(0, 1),
            Node::implies(2, 1),
            Node::implies(0, 0), // a <- a
        ],
        vec![3, 4],
    );
    let cancellation = Cancellation::default();
    let mut preparation_budget = budget(&cancellation, u64::MAX);
    let opened = open(
        &theory,
        u64::MAX,
        LearningLimits::DEFAULT,
        &mut preparation_budget,
    )
    .unwrap();
    assert_eq!(opened.statistics.parity.units, 1);
    assert!(opened.root.unwrap().is_held(0));

    let candidate = Interpretation::new(&theory, [0]).unwrap();
    let index = built(&theory);
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
    let mut query_budget = budget(&cancellation, u64::MAX);
    let mut statistics = Statistics::default();
    let result = ReductQuery::new(&index)
        .check(
            &evaluated,
            Limits::default(),
            &mut query_budget,
            &mut statistics,
            &mut NarrowingScratch::default(),
        )
        .unwrap();
    let crate::Check::NonMinimal(witness) = result else {
        panic!("the empty interpretation must witness nonminimality");
    };
    assert!(witness.theory().same_instance(&theory));
    assert!(witness.atoms().next().is_none());
    // This calls the real general frozen-region query directly; a positive
    // certificate cannot make the absence of arithmetic activity vacuous.
    assert!(statistics.reduct.regions.regions > 0);
    assert!(statistics.reduct.regions.work > 0);
    assert_eq!(
        statistics.reduct.regions.arithmetic,
        ArithmeticCounts::default()
    );
}
