//! Shared threshold requirements are useful even before a condition is known.

use zetesis_cpu::Cancellation;
use zetesis_ferraris::{
    AdmissionLimits, EvaluationLimits, EvaluationWorkspace, FormulaNodes, FrozenSubject,
    Interpretation, Narrower, Narrowing, NarrowingScratch, NodeView, Region, RegionLimits, Theory,
};

// H requires at least two of a,b,c; L requires one. H and (L -> each atom)
// have the unique model {a,b,c}. Independent per-guard bounds cannot force an
// atom: three possible unit contributions still meet H after removing any one.
// The false fourth condition keeps a bound-4 owner live in physical metadata
// while false in M, exercising owner identity independently of frozen truth.
pub(crate) fn theory(weight: u64, multiples: &[i128]) -> Theory {
    let mut nodes = FormulaNodes::default();
    let mut transaction = nodes.transaction();
    for atom in 0..3 {
        transaction
            .push(NodeView::Atom(atom), usize::MAX, usize::MAX)
            .unwrap();
    }
    transaction
        .push(NodeView::False, usize::MAX, usize::MAX)
        .unwrap();
    let weight = i128::from(weight);
    let bounds: Vec<_> = multiples.iter().map(|factor| factor * weight).collect();
    let guards = transaction
        .push_threshold_family(
            &[0, 1, 2, 3],
            &[u64::try_from(weight).unwrap(); 4],
            &bounds,
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
    let low = guards[multiples.iter().position(|&bound| bound == 1).unwrap()];
    let high = guards[multiples.iter().position(|&bound| bound == 2).unwrap()];
    let mut roots = vec![high];
    for atom in 0..3 {
        roots.push(
            transaction
                .push(NodeView::Implies(low, atom), usize::MAX, usize::MAX)
                .unwrap(),
        );
    }
    transaction.commit();
    Theory::new(3, nodes.into_parts(), roots, AdmissionLimits::default()).unwrap()
}

#[test]
fn required_bound_closes_the_frozen_candidate_without_splitting() {
    for weight in [1, u64::MAX] {
        for order in [&[1, 2][..], &[2, 1], &[4, 1, 2]] {
            let theory = theory(weight, order);
            let candidate = Interpretation::new(&theory, [0, 1, 2]).unwrap();
            let cancellation = Cancellation::default();
            let mut workspace = EvaluationWorkspace::default();
            let evaluation =
                workspace.evaluate(&candidate, EvaluationLimits::default(), &cancellation);
            let truth = evaluation.result.unwrap();
            assert!(truth.is_model());
            let narrower = Narrower::new(&theory);
            let mut region = Region::all_open(3);
            let (outcome, _) = narrower
                .narrow_frozen_known(
                    FrozenSubject::new(&theory, truth.truth()),
                    &mut region,
                    &mut narrower.knowledge(),
                    &mut NarrowingScratch::default(),
                    RegionLimits::default(),
                    &cancellation,
                )
                .unwrap();
            assert!(matches!(outcome, Narrowing::Fixed { .. }));
            assert_eq!(region.held().collect::<Vec<_>>(), [0, 1, 2]);
            assert_eq!(region.open().count(), 0);
        }
    }
}
