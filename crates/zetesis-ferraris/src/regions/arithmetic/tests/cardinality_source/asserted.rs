//! Asserted intervals retain their original context and every occurrence.

use super::*;

fn intervals() -> Theory {
    constructed_theory(4, |nodes| {
        let falsum = push(nodes, NodeView::False);
        let lower = threshold(nodes, &[0, 1, 2], &[3, 3, 3], 4);
        let upper = threshold(nodes, &[0, 1, 2], &[3, 3, 3], 7);
        let negative = negate(nodes, upper, falsum);
        let interval = push(nodes, NodeView::And(&[lower, negative]));
        let other = threshold(nodes, &[1, 2, 3], &[1, 1, 1], 1);
        let not_other = negate(nodes, other, falsum);
        let double_other = negate(nodes, not_other, falsum);
        let nested = push(nodes, NodeView::And(&[interval, double_other]));
        let not_nested = negate(nodes, nested, falsum);
        let double_nested = negate(nodes, not_nested, falsum);
        vec![nested, double_nested]
    })
}

#[test]
fn nested_mixed_bounds_agree_with_original_truth() {
    let owner = intervals();
    let cancellation = Cancellation::default();
    let mut meter = budget(&cancellation);
    let mut storage = Storage::default();
    let mut evaluator = EvaluationWorkspace::default();
    for ordinal in 0..owner.roots().len() {
        let bounds =
            cardinality_source::read_root(&owner, ordinal, &mut storage, &mut meter).unwrap();
        assert_eq!(bounds.len(), 3);
        assert_eq!(
            bounds
                .iter()
                .map(|bound| (bound.side, bound.value, bound.cardinality))
                .collect::<Vec<_>>(),
            [
                (BoundSide::Lower, 2, 3),
                (BoundSide::Upper, 2, 3),
                (BoundSide::Lower, 1, 3),
            ]
        );
        for bound in &bounds {
            assert_eq!(bound.source.kind, Kind::Asserted);
            assert_eq!(bound.source.root, ordinal);
            assert!(matches!(
                owner.view().node(bound.source.node).unwrap(),
                NodeView::Threshold(_)
            ));
            assert!(bound.guard.is_empty());
            let live = storage.bytes();
            assert!(
                cardinality_source::check(&owner, &owner, bound, &mut storage, &mut meter).unwrap()
            );
            assert_eq!(storage.bytes(), live);
        }
        for assignment in 0..1 << owner.atom_count() {
            let interpretation = world(&owner, assignment);
            let evaluated =
                evaluator.evaluate(&interpretation, EvaluationLimits::default(), &cancellation);
            let original = evaluated
                .result
                .unwrap()
                .node_truth(owner.roots()[ordinal])
                .unwrap();
            assert_eq!(
                bounds.iter().all(|bound| holds(bound, assignment)),
                original,
                "root {ordinal}, assignment {assignment}"
            );
        }
        release(bounds, &mut storage);
        assert_eq!(storage.bytes(), 0);
    }
}

#[test]
fn shared_conjunctions_keep_each_source_occurrence() {
    let owner = constructed_theory(2, |nodes| {
        let falsum = push(nodes, NodeView::False);
        let lower = threshold(nodes, &[0, 1], &[1, 1], 1);
        let upper = threshold(nodes, &[0, 1], &[1, 1], 2);
        let negative = negate(nodes, upper, falsum);
        let interval = push(nodes, NodeView::And(&[lower, negative]));
        vec![push(nodes, NodeView::And(&[interval, interval]))]
    });
    let cancellation = Cancellation::default();
    let mut storage = Storage::default();
    let bounds =
        cardinality_source::read_root(&owner, 0, &mut storage, &mut budget(&cancellation)).unwrap();
    assert_eq!(bounds.len(), 4);
    assert_eq!(bounds[0].source, bounds[2].source);
    assert_eq!(bounds[1].source, bounds[3].source);
    assert_ne!(bounds[0].source.node, bounds[1].source.node);
    assert_eq!(
        bounds.iter().map(|bound| bound.side).collect::<Vec<_>>(),
        [
            BoundSide::Lower,
            BoundSide::Upper,
            BoundSide::Lower,
            BoundSide::Upper,
        ]
    );
    assert_eq!(
        (0..4)
            .filter(|&assignment| bounds.iter().all(|bound| holds(bound, assignment)))
            .collect::<Vec<_>>(),
        [1, 2]
    );
    release(bounds, &mut storage);
    assert_eq!(storage.bytes(), 0);
}

#[test]
fn an_ineligible_last_child_discards_every_tentative_bound() {
    let owner = constructed_theory(3, |nodes| {
        let falsum = push(nodes, NodeView::False);
        let valid = threshold(nodes, &[0, 1], &[1, 1], 1);
        let negative = negate(nodes, valid, falsum);
        let interval = push(nodes, NodeView::And(&[valid, negative]));
        let negated_interval = negate(nodes, interval, falsum);
        let double_interval = negate(nodes, negated_interval, falsum);
        let disjunction = push(nodes, NodeView::Or(&[0, 1]));
        let implication = push(nodes, NodeView::Implies(valid, 2));
        let compound = push(nodes, NodeView::And(&[0, 1]));
        let invalid = [
            0,
            disjunction,
            implication,
            double_interval,
            threshold(nodes, &[0, 1], &[1, 2], 1),
            threshold(nodes, &[0, 0], &[1, 1], 1),
            threshold(nodes, &[compound, 2], &[1, 1], 1),
            threshold(nodes, &[0, 1], &[0, 0], 1),
            threshold(nodes, &[0, 1], &[1, 1], 0),
        ];
        invalid
            .into_iter()
            .map(|last| {
                let nested = push(nodes, NodeView::And(&[valid, last]));
                push(nodes, NodeView::And(&[interval, nested]))
            })
            .collect()
    });
    let cancellation = Cancellation::default();
    let mut storage = Storage::default();
    let mut meter = budget(&cancellation);
    for ordinal in 0..owner.roots().len() {
        let bounds =
            cardinality_source::read_root(&owner, ordinal, &mut storage, &mut meter).unwrap();
        assert!(bounds.is_empty(), "root {ordinal}");
        release(bounds, &mut storage);
        assert_eq!(storage.bytes(), 0, "root {ordinal}");
    }
}

#[test]
fn independent_rereading_authenticates_the_entire_asserted_context() {
    let owner = constructed_theory(2, |nodes| {
        let row = threshold(nodes, &[0, 1], &[1, 1], 1);
        let invalid = push(nodes, NodeView::Or(&[0, 1]));
        vec![row, push(nodes, NodeView::And(&[row, invalid]))]
    });
    let cancellation = Cancellation::default();
    let mut meter = budget(&cancellation);
    let mut storage = Storage::default();
    let mut bounds = cardinality_source::read_root(&owner, 0, &mut storage, &mut meter).unwrap();
    assert_eq!(bounds.len(), 1);
    let live = storage.bytes();
    assert!(
        cardinality_source::check(&owner, &owner, &bounds[0], &mut storage, &mut meter).unwrap()
    );
    bounds[0].source.root = 1;
    assert!(
        !cardinality_source::check(&owner, &owner, &bounds[0], &mut storage, &mut meter).unwrap()
    );
    assert_eq!(storage.bytes(), live);
    release(bounds, &mut storage);
    assert_eq!(storage.bytes(), 0);
}

#[test]
fn asserted_count_support_has_no_eight_atom_parity_limit() {
    let owner = constructed_theory(129, |nodes| {
        let falsum = push(nodes, NodeView::False);
        let conditions: Vec<_> = (0..129).collect();
        let weights = vec![3; 129];
        let lower = threshold(nodes, &conditions, &weights, 109);
        let upper = threshold(nodes, &conditions, &weights, 253);
        let negative = negate(nodes, upper, falsum);
        let interval = push(nodes, NodeView::And(&[lower, negative]));
        let not_interval = negate(nodes, interval, falsum);
        vec![negate(nodes, not_interval, falsum)]
    });
    let cancellation = Cancellation::default();
    let mut storage = Storage::default();
    let mut meter = budget(&cancellation);
    let bounds = cardinality_source::read_root(&owner, 0, &mut storage, &mut meter).unwrap();
    assert_eq!(bounds.len(), 2);
    for bound in &bounds {
        assert_eq!(&*bound.support, [u64::MAX, u64::MAX, 1, 0]);
        assert_eq!(bound.cardinality, 129);
        assert!(bound.guard.is_empty());
        assert!(
            cardinality_source::check(&owner, &owner, bound, &mut storage, &mut meter).unwrap()
        );
    }
    assert_eq!((bounds[0].side, bounds[0].value), (BoundSide::Lower, 37));
    assert_eq!((bounds[1].side, bounds[1].value), (BoundSide::Upper, 84));
    release(bounds, &mut storage);
    assert_eq!(storage.bytes(), 0);
}

#[test]
fn every_nested_work_refusal_prefix_publishes_no_family() {
    let owner = intervals();
    let cancellation = Cancellation::default();
    let mut full = budget(&cancellation);
    let mut storage = Storage::default();
    let bounds = cardinality_source::read_root(&owner, 1, &mut storage, &mut full).unwrap();
    assert_eq!(bounds.len(), 3);
    release(bounds, &mut storage);
    for maximum in 0..full.work() {
        let mut meter = Budget::new(Work::new(u64::MAX), maximum, usize::MAX, &cancellation);
        assert!(matches!(
            cardinality_source::read_root(&owner, 1, &mut Storage::default(), &mut meter),
            Err(BudgetError::WorkDeclined)
        ));
        assert!(meter.work() <= maximum);
    }
}

#[test]
fn every_nested_storage_refusal_prefix_publishes_no_family() {
    let owner = intervals();
    let cancellation = Cancellation::default();
    let mut full = budget(&cancellation);
    let mut storage = Storage::default();
    let bounds = cardinality_source::read_root(&owner, 1, &mut storage, &mut full).unwrap();
    assert_eq!(bounds.len(), 3);
    release(bounds, &mut storage);
    let peak = usize::try_from(full.peak_bytes()).unwrap();
    for maximum in 0..peak {
        let mut meter = Budget::new(Work::new(u64::MAX), u64::MAX, maximum, &cancellation);
        assert!(matches!(
            cardinality_source::read_root(&owner, 1, &mut Storage::default(), &mut meter),
            Err(BudgetError::StorageDeclined { limit, .. }) if limit == maximum
        ));
    }
}

#[test]
fn every_nested_checker_refusal_prefix_stays_incomplete() {
    let owner = intervals();
    let cancellation = Cancellation::default();
    let mut storage = Storage::default();
    let bounds =
        cardinality_source::read_root(&owner, 1, &mut storage, &mut budget(&cancellation)).unwrap();
    let base = storage.bytes();
    let mut full = budget(&cancellation);
    assert!(
        cardinality_source::check(&owner, &owner, &bounds[1], &mut storage, &mut full).unwrap()
    );
    assert_eq!(storage.bytes(), base);
    for maximum in 0..full.work() {
        let mut meter = Budget::new(Work::new(u64::MAX), maximum, usize::MAX, &cancellation);
        assert_eq!(
            cardinality_source::check(
                &owner,
                &owner,
                &bounds[1],
                &mut Storage::with_live_bytes(base),
                &mut meter,
            ),
            Err(BudgetError::WorkDeclined)
        );
        assert!(meter.work() <= maximum);
    }
    release(bounds, &mut storage);
    assert_eq!(storage.bytes(), 0);
}
