//! Capacity deductions preserve original models, not arbitrary frozen models.
use crate::test_support::close;
use crate::{
    AdmissionLimits, EvaluationLimits, EvaluationWorkspace, FormulaNodes, FormulaParts,
    FrozenSubject, Interpretation, Narrower, Narrowing, NarrowingScratch, Node, NodeView, Region,
    RegionLimits, Theory,
};
use zetesis_cpu::Cancellation;

fn push(nodes: &mut FormulaNodes, view: NodeView<'_>) -> usize {
    let mut transaction = nodes.transaction();
    let result = transaction.push(view, usize::MAX, usize::MAX).unwrap();
    transaction.commit();
    result
}

fn theory(aliases_are_roots: bool, duplicate: bool, doubled: bool) -> Theory {
    let mut nodes =
        FormulaNodes::new(FormulaParts::new((0..6).map(Node::atom).collect(), vec![]).unwrap());
    let falsum = push(&mut nodes, NodeView::False);
    let count = nodes
        .push_threshold_family(
            &[0, 1, 2, 3],
            &[1, 1, 1, 1],
            &[if duplicate { 2 } else { 3 }],
            usize::MAX,
            usize::MAX,
        )
        .unwrap()[0];
    let upper = push(&mut nodes, NodeView::Implies(count, falsum));
    let mut roots = vec![upper];
    for (source, target) in [(4, 0), (5, usize::from(!duplicate))] {
        let mut implication = push(&mut nodes, NodeView::Implies(source, target));
        if doubled {
            let negative = push(&mut nodes, NodeView::Implies(implication, falsum));
            implication = push(&mut nodes, NodeView::Implies(negative, falsum));
        }
        if aliases_are_roots {
            roots.push(implication);
        }
    }
    let first = push(&mut nodes, NodeView::And(&[4, 5]));
    let second = push(&mut nodes, NodeView::And(&[2, 3]));
    let alternatives = push(&mut nodes, NodeView::Or(&[first, second]));
    roots.push(alternatives);
    Theory::new(6, nodes.into_parts(), roots, AdmissionLimits::default()).unwrap()
}

#[test]
fn held_contributions_exclude_over_capacity_alternatives() {
    for doubled in [false, true] {
        let mut nodes =
            FormulaNodes::new(FormulaParts::new((0..9).map(Node::atom).collect(), vec![]).unwrap());
        let falsum = push(&mut nodes, NodeView::False);
        let count = nodes
            .push_threshold_family(
                &[0, 1, 2, 3, 4, 5],
                &[1, 1, 1, 1, 1, 1],
                &[4],
                usize::MAX,
                usize::MAX,
            )
            .unwrap()[0];
        let mut roots = vec![push(&mut nodes, NodeView::Implies(count, falsum))];
        for atom in 0..3 {
            let mut guard = push(&mut nodes, NodeView::Implies(atom + 6, atom));
            if doubled {
                let neg = push(&mut nodes, NodeView::Implies(guard, falsum));
                guard = push(&mut nodes, NodeView::Implies(neg, falsum));
            }
            roots.push(guard);
        }
        let first = push(&mut nodes, NodeView::And(&[6, 7, 8]));
        let second = push(&mut nodes, NodeView::And(&[3, 4, 5]));
        roots.push(push(&mut nodes, NodeView::Or(&[first, second])));
        let theory = Theory::new(9, nodes.into_parts(), roots, AdmissionLimits::default()).unwrap();
        let index = Narrower::new(&theory);
        assert!(index.capacity_index_bytes() > 0);
        let mut without = index.clone();
        without.thresholds.capacity = None;
        let mut actual = Region::all_open(9);
        actual.hold(0);
        actual.hold(3);
        let mut control = actual.clone();
        assert_ne!(
            close(
                &theory,
                &without,
                &mut control,
                &mut without.knowledge(),
                &mut NarrowingScratch::default()
            )
            .0,
            Narrowing::Refuted
        );
        assert_eq!(
            close(
                &theory,
                &index,
                &mut actual,
                &mut index.knowledge(),
                &mut NarrowingScratch::default()
            )
            .0,
            Narrowing::Refuted
        );
    }
}

#[test]
fn repeated_necessary_conditions_count_once() {
    let theory = theory(true, true, true);
    let index = Narrower::new(&theory);
    let mut region = Region::all_open(6);
    region.hold(0);
    assert_ne!(
        close(
            &theory,
            &index,
            &mut region,
            &mut index.knowledge(),
            &mut NarrowingScratch::default()
        )
        .0,
        Narrowing::Refuted
    );
    assert!(region.is_held(4) && region.is_held(5));
}

#[test]
fn nonroot_implications_supply_no_alias() {
    let theory = theory(false, false, false);
    let index = Narrower::new(&theory);
    let mut region = Region::all_open(6);
    region.hold(0);
    region.hold(2);
    assert_ne!(
        close(
            &theory,
            &index,
            &mut region,
            &mut index.knowledge(),
            &mut NarrowingScratch::default()
        )
        .0,
        Narrowing::Refuted
    );
    assert!(region.is_held(4) && region.is_held(5));
}

fn agrees(region: &Region, mask: usize) -> bool {
    (0..6).all(|atom| {
        region
            .decision(atom)
            .is_none_or(|held| held == (mask & (1 << atom) != 0))
    })
}

#[test]
fn capacity_preserves_every_compatible_original_model() {
    for doubled in [false, true] {
        for duplicate in [false, true] {
            let theory = theory(true, duplicate, doubled);
            let index = Narrower::new(&theory);
            let mut evaluation = EvaluationWorkspace::default();
            let models: Vec<_> = (0..64)
                .filter(|&mask| {
                    let model = Interpretation::new(
                        &theory,
                        (0..6).filter(|&atom| mask & (1 << atom) != 0),
                    )
                    .unwrap();
                    evaluation
                        .evaluate(
                            &model,
                            EvaluationLimits::default(),
                            &Cancellation::default(),
                        )
                        .result
                        .unwrap()
                        .is_model()
                })
                .collect();
            assert!(!models.is_empty());
            for mut code in 0..729 {
                let mut region = Region::all_open(6);
                for atom in 0..6 {
                    match code % 3 {
                        1 => {
                            region.hold(atom);
                        }
                        2 => {
                            region.cut(atom);
                        }
                        _ => {}
                    }
                    code /= 3;
                }
                let before = region.clone();
                let result = close(
                    &theory,
                    &index,
                    &mut region,
                    &mut index.knowledge(),
                    &mut NarrowingScratch::default(),
                )
                .0;
                for &model in &models {
                    if agrees(&before, model) {
                        assert_ne!(result, Narrowing::Refuted);
                        assert!(agrees(&region, model));
                    }
                }
            }
        }
    }
}

#[test]
fn frozen_closure_ignores_capacity_projection() {
    let theory = theory(true, false, true);
    let index = Narrower::new(&theory);
    let mut without = index.clone();
    without.thresholds.capacity = None;
    let mut evaluation = EvaluationWorkspace::default();
    for mask in 0..64 {
        let model =
            Interpretation::new(&theory, (0..6).filter(|&atom| mask & (1 << atom) != 0)).unwrap();
        let evaluated = evaluation
            .evaluate(
                &model,
                EvaluationLimits::default(),
                &Cancellation::default(),
            )
            .result
            .unwrap();
        let truth: Vec<_> = (0..theory.nodes().len())
            .map(|node| evaluated.node_truth(node).unwrap())
            .collect();
        let subject = FrozenSubject::new(&theory, &truth);
        let mut actual = Region::all_open(6);
        let mut expected = actual.clone();
        let a = index
            .narrow_frozen_known(
                subject,
                &mut actual,
                &mut index.knowledge(),
                &mut NarrowingScratch::default(),
                RegionLimits { max_work: u64::MAX },
                &Cancellation::default(),
            )
            .unwrap();
        let b = without
            .narrow_frozen_known(
                subject,
                &mut expected,
                &mut without.knowledge(),
                &mut NarrowingScratch::default(),
                RegionLimits { max_work: u64::MAX },
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(a.0, b.0);
        assert_eq!(actual, expected);
        assert_eq!(a.1.work, b.1.work);
    }
}
mod events;
