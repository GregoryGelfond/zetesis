//! Native narrowing preserves every compatible frozen model, including J ⊄ M.

use crate::{
    AdmissionLimits, EvaluationLimits, EvaluationWorkspace, FormulaNodes, FormulaTransaction,
    FrozenSubject, Interpretation, Limits, Narrower, Narrowing, NarrowingScratch, NodeView, Region,
    RegionLimits, Theory, models_reduct,
};
use zetesis_cpu::Cancellation;

fn push(transaction: &mut FormulaTransaction<'_>, node: NodeView<'_>) -> usize {
    transaction.push(node, usize::MAX, usize::MAX).unwrap()
}

fn theory(bound: i128, shape: usize) -> Theory {
    let mut nodes = FormulaNodes::default();
    let mut transaction = nodes.transaction();
    for atom in 0..3 {
        push(&mut transaction, NodeView::Atom(atom));
    }
    let falsum = push(&mut transaction, NodeView::False);
    let implication = push(&mut transaction, NodeView::Implies(0, 1));
    let negative = push(&mut transaction, NodeView::Implies(1, falsum));
    let conjunction = push(&mut transaction, NodeView::And(&[implication, 2]));
    let guards = transaction
        .push_threshold_family(
            &[0, 0, 1, implication, negative, conjunction],
            &[2, 3, 0, 4, 2, 1],
            &[bound, bound + 1, bound + 2],
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
    let roots = match shape {
        0 => vec![guards[0]],
        1 => vec![push(&mut transaction, NodeView::Implies(guards[0], falsum))],
        2 => vec![
            guards[0],
            push(&mut transaction, NodeView::Implies(guards[2], falsum)),
        ],
        _ => vec![push(
            &mut transaction,
            NodeView::Implies(guards[0], guards[1]),
        )],
    };
    transaction.commit();
    Theory::new(3, nodes.into_parts(), roots, AdmissionLimits::default()).unwrap()
}

fn world(theory: &Theory, mask: usize) -> Interpretation {
    Interpretation::new(theory, (0..3).filter(|atom| mask & (1 << atom) != 0)).unwrap()
}

fn region(code: usize) -> Region {
    let mut result = Region::all_open(3);
    let mut digits = code;
    for atom in 0..3 {
        match digits % 3 {
            1 => assert!(result.hold(atom)),
            2 => assert!(result.cut(atom)),
            _ => {}
        }
        digits /= 3;
    }
    result
}

fn agrees(region: &Region, mask: usize) -> bool {
    let mut probe = region.clone();
    (0..3).all(|atom| {
        if mask & (1 << atom) != 0 {
            probe.hold(atom)
        } else {
            probe.cut(atom)
        }
    })
}

#[test]
fn narrowing_keeps_all_arbitrary_frozen_models() {
    let cancellation = Cancellation::default();
    let mut non_subset_models = 0;
    for bound in [-1, 0, 1, 4, 6, 9, 12, 13] {
        for shape in 0..4 {
            let theory = theory(bound, shape);
            let narrower = Narrower::new(&theory);
            for candidate in 0..8 {
                let outer = world(&theory, candidate);
                let mut workspace = EvaluationWorkspace::default();
                let evaluated = workspace
                    .evaluate(&outer, EvaluationLimits::default(), &cancellation)
                    .result
                    .unwrap();
                let truth: Vec<_> = (0..theory.nodes().len())
                    .map(|node| evaluated.node_truth(node).unwrap())
                    .collect();
                let frozen_models: Vec<_> = (0..8)
                    .filter(|&tested| {
                        models_reduct(
                            &theory,
                            &outer,
                            &world(&theory, tested),
                            Limits::default(),
                            &cancellation,
                        )
                        .unwrap()
                    })
                    .collect();
                non_subset_models += frozen_models
                    .iter()
                    .filter(|&&tested| tested & !candidate != 0)
                    .count();
                for code in 0..27 {
                    let before = region(code);
                    let mut after = before.clone();
                    let outcome = narrower
                        .narrow_frozen_known(
                            FrozenSubject::new(&theory, &truth),
                            &mut after,
                            &mut narrower.knowledge(),
                            &mut NarrowingScratch::default(),
                            RegionLimits::default(),
                            &cancellation,
                        )
                        .unwrap()
                        .0;
                    for &tested in &frozen_models {
                        if agrees(&before, tested) {
                            assert_ne!(
                                outcome,
                                Narrowing::Refuted,
                                "M={candidate}, J={tested}, bound={bound}, shape={shape}, region={code}"
                            );
                            assert!(
                                agrees(&after, tested),
                                "narrowing removed a frozen model: M={candidate}, J={tested}, bound={bound}, shape={shape}, region={code}"
                            );
                        }
                    }
                }
            }
        }
    }
    assert!(non_subset_models > 0);
}
