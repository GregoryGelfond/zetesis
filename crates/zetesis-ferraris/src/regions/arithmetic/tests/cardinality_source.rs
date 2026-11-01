mod asserted;

use zetesis_algebra::{Cancellation, Stop};

use crate::{
    AdmissionError, EvaluationLimits, EvaluationWorkspace, FormulaTransaction, NodeView, Theory,
    Threshold,
    regions::{
        Work,
        arithmetic::{
            budget::{Budget, BudgetError},
            cardinality_source::{self, Bound, Kind},
            source::Literal,
            storage::Storage,
            uniform::BoundSide,
        },
    },
    test_support::{Allowance, constructed_theory, push, world},
};

fn budget(cancellation: &Cancellation) -> Budget<'_, 'static> {
    Budget::new(Work::new(u64::MAX), u64::MAX, usize::MAX, cancellation)
}

fn threshold(
    nodes: &mut FormulaTransaction<'_>,
    conditions: &[usize],
    weights: &[u64],
    bound: i128,
) -> usize {
    push(
        nodes,
        NodeView::Threshold(Threshold {
            conditions,
            weights,
            bound,
        }),
    )
}

fn negate(nodes: &mut FormulaTransaction<'_>, node: usize, falsum: usize) -> usize {
    push(nodes, NodeView::Implies(node, falsum))
}

fn shapes() -> Theory {
    constructed_theory(5, |nodes| {
        let falsum = push(nodes, NodeView::False);
        let row = threshold(nodes, &[0, 1, 2], &[3, 3, 3], 4);
        let not_row = negate(nodes, row, falsum);
        let double_row = negate(nodes, not_row, falsum);
        let not_four = negate(nodes, 4, falsum);
        let repeated = push(nodes, NodeView::And(&[3, not_four, 3]));
        let guarded_upper = push(nodes, NodeView::And(&[repeated, row]));
        let upper = negate(nodes, guarded_upper, falsum);
        let guarded_lower = push(nodes, NodeView::And(&[3, not_row]));
        let lower = negate(nodes, guarded_lower, falsum);
        let negations: Vec<_> = (0..3).map(|atom| negate(nodes, atom, falsum)).collect();
        let clause_body = push(nodes, NodeView::And(&negations));
        let clause = negate(nodes, clause_body, falsum);
        let second = threshold(nodes, &[2, 3, 4], &[1, 1, 1], 1);
        let conjunction = push(nodes, NodeView::And(&[row, second, double_row]));
        let not_conjunction = negate(nodes, conjunction, falsum);
        let double_conjunction = negate(nodes, not_conjunction, falsum);
        vec![
            upper,
            lower,
            row,
            double_row,
            clause,
            conjunction,
            double_conjunction,
        ]
    })
}

fn release(mut bounds: Vec<Bound>, storage: &mut Storage) {
    for bound in bounds.drain(..) {
        cardinality_source::release(bound, storage).unwrap();
    }
    storage.release(bounds).unwrap();
}

fn holds(bound: &Bound, interpretation: usize) -> bool {
    let active = bound
        .guard
        .iter()
        .all(|literal| (interpretation & (1 << literal.atom) != 0) == literal.positive);
    let count: u64 = bound
        .support
        .iter()
        .enumerate()
        .map(|(word, support)| {
            let present = if word == 0 {
                u64::try_from(interpretation).unwrap()
            } else {
                0
            };
            u64::from((support & present).count_ones())
        })
        .sum();
    !active
        || match bound.side {
            BoundSide::Lower => count >= bound.value,
            BoundSide::Upper => count <= bound.value,
        }
}

#[test]
fn all_four_shapes_agree_with_original_formula_truth() {
    let owner = shapes();
    let cancellation = Cancellation::default();
    let mut meter = budget(&cancellation);
    let mut storage = Storage::default();
    let mut evaluator = EvaluationWorkspace::default();
    for ordinal in 0..owner.roots().len() {
        let bounds =
            cardinality_source::read_root(&owner, ordinal, &mut storage, &mut meter).unwrap();
        assert_eq!(bounds.len(), if ordinal < 5 { 1 } else { 3 });
        for assignment in 0..1 << owner.atom_count() {
            let interpretation = world(&owner, assignment);
            let result =
                evaluator.evaluate(&interpretation, EvaluationLimits::default(), &cancellation);
            let original = result
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
        for bound in &bounds {
            assert!(
                cardinality_source::check(&owner, &owner, bound, &mut storage, &mut meter).unwrap()
            );
            assert_eq!(bound.support.len(), 2);
            assert_eq!(bound.support[1], 0);
            assert_eq!(bound.support[0] >> owner.atom_count(), 0);
        }
        release(bounds, &mut storage);
        assert_eq!(storage.bytes(), 0);
    }
}

#[test]
fn nested_guards_retain_order_polarity_and_repeated_occurrences() {
    let owner = shapes();
    let cancellation = Cancellation::default();
    let mut storage = Storage::default();
    let bounds =
        cardinality_source::read_root(&owner, 0, &mut storage, &mut budget(&cancellation)).unwrap();
    assert_eq!(
        bounds[0].guard,
        [
            Literal {
                atom: 3,
                positive: true
            },
            Literal {
                atom: 4,
                positive: false
            },
            Literal {
                atom: 3,
                positive: true
            },
        ]
    );
    assert_eq!(
        (bounds[0].side, bounds[0].value, bounds[0].cardinality),
        (BoundSide::Upper, 1, 3)
    );
    assert_eq!(bounds[0].source.kind, Kind::UpperConstraint);
    release(bounds, &mut storage);
    assert_eq!(storage.bytes(), 0);
}

#[test]
fn contradictory_guard_is_retained_without_becoming_an_unconditional_bound() {
    let owner = constructed_theory(2, |nodes| {
        let falsum = push(nodes, NodeView::False);
        let negative = negate(nodes, 0, falsum);
        let guard = push(nodes, NodeView::And(&[0, negative]));
        let row = threshold(nodes, &[1], &[1], 1);
        let body = push(nodes, NodeView::And(&[guard, row]));
        vec![negate(nodes, body, falsum)]
    });
    let cancellation = Cancellation::default();
    let mut storage = Storage::default();
    let bounds =
        cardinality_source::read_root(&owner, 0, &mut storage, &mut budget(&cancellation)).unwrap();
    assert_eq!(bounds.len(), 1);
    assert_eq!(bounds[0].guard.len(), 2);
    assert!((0..4).all(|assignment| holds(&bounds[0], assignment)));
    release(bounds, &mut storage);
}

#[test]
fn unequal_zero_repeated_compound_and_trivial_thresholds_decline() {
    let owner = constructed_theory(3, |nodes| {
        let falsum = push(nodes, NodeView::False);
        let negative = negate(nodes, 0, falsum);
        let compound = push(nodes, NodeView::And(&[0, 1]));
        vec![
            threshold(nodes, &[0, 1], &[1, 2], 1),
            threshold(nodes, &[0, 1], &[0, 0], 1),
            threshold(nodes, &[0, 0], &[1, 1], 1),
            threshold(nodes, &[negative, 1], &[1, 1], 1),
            threshold(nodes, &[compound, 2], &[1, 1], 1),
            threshold(nodes, &[0, 1], &[1, 1], 0),
            threshold(nodes, &[0, 1], &[1, 1], -1),
            threshold(nodes, &[0, 1], &[1, 1], 3),
            threshold(nodes, &[], &[], 1),
        ]
    });
    let cancellation = Cancellation::default();
    let mut storage = Storage::default();
    let mut meter = budget(&cancellation);
    for ordinal in 0..owner.roots().len() {
        let bounds =
            cardinality_source::read_root(&owner, ordinal, &mut storage, &mut meter).unwrap();
        assert!(bounds.is_empty(), "root {ordinal}");
        release(bounds, &mut storage);
        assert_eq!(storage.bytes(), 0);
    }
}

#[test]
fn unsupported_contexts_do_not_publish_a_recognized_prefix() {
    let owner = constructed_theory(3, |nodes| {
        let falsum = push(nodes, NodeView::False);
        let row = threshold(nodes, &[0, 1], &[1, 1], 1);
        let second = threshold(nodes, &[1, 2], &[1, 1], 1);
        let two = push(nodes, NodeView::And(&[row, second]));
        let or_guard = push(nodes, NodeView::Or(&[0, 1]));
        let compound = push(nodes, NodeView::And(&[row, or_guard]));
        let invalid_assertion = push(nodes, NodeView::And(&[row, 0]));
        let neg = negate(nodes, row, falsum);
        let double = negate(nodes, neg, falsum);
        let triple = negate(nodes, double, falsum);
        let quadruple = negate(nodes, triple, falsum);
        let negative = negate(nodes, 0, falsum);
        let duplicate_clause = push(nodes, NodeView::And(&[negative, negative]));
        let other_negative = negate(nodes, 1, falsum);
        let nested_clause = push(nodes, NodeView::And(&[duplicate_clause, other_negative]));
        vec![
            negate(nodes, two, falsum),
            negate(nodes, compound, falsum),
            invalid_assertion,
            quadruple,
            negate(nodes, duplicate_clause, falsum),
            negate(nodes, nested_clause, falsum),
            negate(nodes, 0, falsum),
        ]
    });
    let cancellation = Cancellation::default();
    let mut storage = Storage::default();
    let mut meter = budget(&cancellation);
    for ordinal in 0..owner.roots().len() {
        let bounds =
            cardinality_source::read_root(&owner, ordinal, &mut storage, &mut meter).unwrap();
        assert!(bounds.is_empty(), "root {ordinal}");
        release(bounds, &mut storage);
        assert_eq!(storage.bytes(), 0);
    }
}

#[test]
fn clause_lower_has_no_eight_atom_pattern_limit() {
    let owner = constructed_theory(129, |nodes| {
        let falsum = push(nodes, NodeView::False);
        let negatives: Vec<_> = (0..129).map(|atom| negate(nodes, atom, falsum)).collect();
        let body = push(nodes, NodeView::And(&negatives));
        vec![negate(nodes, body, falsum)]
    });
    let cancellation = Cancellation::default();
    let mut storage = Storage::default();
    let bounds =
        cardinality_source::read_root(&owner, 0, &mut storage, &mut budget(&cancellation)).unwrap();
    assert_eq!(bounds.len(), 1);
    assert_eq!(bounds[0].support, [u64::MAX, u64::MAX, 1, 0]);
    assert_eq!(bounds[0].cardinality, 129);
    assert_eq!(bounds[0].value, 1);
    assert_eq!(bounds[0].side, BoundSide::Lower);
    assert_eq!(bounds[0].source.kind, Kind::ClauseLower);
    assert!(bounds[0].guard.is_empty());
    release(bounds, &mut storage);
    assert_eq!(storage.bytes(), 0);
}

#[test]
fn independent_reread_rejects_every_corrupted_proposal_field() {
    let owner = shapes();
    let cancellation = Cancellation::default();
    for field in 0..13 {
        let mut meter = budget(&cancellation);
        let mut storage = Storage::default();
        let mut bounds =
            cardinality_source::read_root(&owner, 0, &mut storage, &mut meter).unwrap();
        let candidate = &mut bounds[0];
        match field {
            0 => candidate.source.kind = Kind::Asserted,
            1 => candidate.source.root = 1,
            2 => candidate.source.node = 0,
            3 => candidate.side = BoundSide::Lower,
            4 => candidate.value += 1,
            5 => candidate.cardinality += 1,
            6 => candidate.support[0] ^= 1,
            7 => candidate.support[1] = 1,
            8 => candidate.guard.swap(0, 1),
            9 => candidate.guard[0].positive = false,
            10 => candidate.guard[0].atom = 2,
            11 => {
                candidate.support.pop();
            }
            _ => {
                candidate.guard.pop();
            }
        }
        let live = storage.bytes();
        assert!(
            !cardinality_source::check(&owner, &owner, candidate, &mut storage, &mut meter)
                .unwrap(),
            "field {field}"
        );
        assert_eq!(storage.bytes(), live, "field {field}");
        release(bounds, &mut storage);
        assert_eq!(storage.bytes(), 0);
    }
}

#[test]
fn owner_and_source_root_identity_cannot_be_replaced_by_equal_contents() {
    let owner = shapes();
    let foreign = shapes();
    let clone = owner.clone();
    let cancellation = Cancellation::default();
    let mut meter = budget(&cancellation);
    let mut storage = Storage::default();
    let mut bounds = cardinality_source::read_root(&owner, 0, &mut storage, &mut meter).unwrap();
    let live = storage.bytes();
    assert!(
        !cardinality_source::check(&owner, &foreign, &bounds[0], &mut storage, &mut meter).unwrap()
    );
    assert!(
        cardinality_source::check(&owner, &clone, &bounds[0], &mut storage, &mut meter).unwrap()
    );
    bounds[0].source.root = usize::MAX;
    assert_eq!(
        cardinality_source::check(&owner, &owner, &bounds[0], &mut storage, &mut meter),
        Err(BudgetError::Source(AdmissionError::Root))
    );
    assert_eq!(storage.bytes(), live);
    release(bounds, &mut storage);
}

#[test]
fn every_work_refusal_prefix_publishes_no_row() {
    let owner = shapes();
    let cancellation = Cancellation::default();
    let mut full = budget(&cancellation);
    let mut storage = Storage::default();
    let rows = cardinality_source::read_root(&owner, 0, &mut storage, &mut full).unwrap();
    release(rows, &mut storage);
    for maximum in 0..full.work() {
        let mut meter = Budget::new(Work::new(u64::MAX), maximum, usize::MAX, &cancellation);
        assert!(
            matches!(
                cardinality_source::read_root(&owner, 0, &mut Storage::default(), &mut meter),
                Err(BudgetError::WorkDeclined)
            ),
            "maximum {maximum}"
        );
        assert!(meter.work() <= maximum);
    }
}

#[test]
fn every_storage_refusal_prefix_publishes_no_row() {
    let owner = shapes();
    let cancellation = Cancellation::default();
    let mut full = budget(&cancellation);
    let mut storage = Storage::default();
    let rows = cardinality_source::read_root(&owner, 0, &mut storage, &mut full).unwrap();
    release(rows, &mut storage);
    let peak = usize::try_from(full.peak_bytes()).unwrap();
    for maximum in 0..peak {
        let mut meter = Budget::new(Work::new(u64::MAX), u64::MAX, maximum, &cancellation);
        assert!(
            matches!(cardinality_source::read_root(&owner, 0, &mut Storage::default(), &mut meter), Err(BudgetError::StorageDeclined { limit, .. }) if limit == maximum)
        );
    }
}

#[test]
fn global_stop_and_cancellation_keep_their_typed_failures() {
    let owner = shapes();
    let cancellation = Cancellation::default();
    for failure in [Stop::Cancelled, Stop::WorkLimit, Stop::Allocation] {
        let mut quota = Allowance {
            remaining: 100,
            failure,
        };
        let mut meter = Budget::new(
            Work::reserved(&mut quota),
            u64::MAX,
            usize::MAX,
            &cancellation,
        );
        assert!(
            matches!(cardinality_source::read_root(&owner, 0, &mut Storage::default(), &mut meter), Err(BudgetError::Stopped(actual)) if actual == failure)
        );
        meter.finish();
    }
    cancellation.cancel();
    let mut meter = Budget::new(Work::new(0), 0, 0, &cancellation);
    assert!(matches!(
        cardinality_source::read_root(&owner, usize::MAX, &mut Storage::default(), &mut meter),
        Err(BudgetError::Stopped(Stop::Cancelled))
    ));
    assert_eq!(meter.work(), 0);
}

#[test]
fn repeated_signed_guard_occurrences_keep_their_exact_decoder_work() {
    fn measured(positive: bool) -> u64 {
        let owner = constructed_theory(2, |nodes| {
            let falsum = push(nodes, NodeView::False);
            let negative = negate(nodes, 0, falsum);
            let row = threshold(nodes, &[1], &[1], 1);
            let guard = if positive { 0 } else { negative };
            let body = push(nodes, NodeView::And(&[guard, guard, guard, row]));
            vec![negate(nodes, body, falsum)]
        });
        let cancellation = Cancellation::default();
        let mut meter = budget(&cancellation);
        let mut storage = Storage::default();
        let rows = cardinality_source::read_root(&owner, 0, &mut storage, &mut meter).unwrap();
        assert_eq!(rows[0].guard.len(), 3);
        assert!(
            rows[0]
                .guard
                .iter()
                .all(|literal| literal.positive == positive)
        );
        release(rows, &mut storage);
        meter.work()
    }
    // Every occurrence is read, including repetitions. The leaf view, failed
    // threshold decoder, signed-atom decoder, and publication cost 41 for a
    // negative literal and 18 for a positive literal. Both frame/guard growth
    // schedules have the same dimensions, without assuming allocator capacity.
    assert_eq!(measured(false) - measured(true), 3 * (41 - 18));
}

#[test]
fn every_checker_work_refusal_prefix_remains_a_failure_not_acceptance() {
    let owner = shapes();
    let cancellation = Cancellation::default();
    let mut retained = Storage::default();
    let rows = cardinality_source::read_root(&owner, 0, &mut retained, &mut budget(&cancellation))
        .unwrap();
    let base = retained.bytes();
    let mut complete = budget(&cancellation);
    assert!(
        cardinality_source::check(&owner, &owner, &rows[0], &mut retained, &mut complete).unwrap()
    );
    assert_eq!(retained.bytes(), base);
    for maximum in 0..complete.work() {
        let mut meter = Budget::new(Work::new(u64::MAX), maximum, usize::MAX, &cancellation);
        let mut storage = Storage::with_live_bytes(base);
        assert_eq!(
            cardinality_source::check(&owner, &owner, &rows[0], &mut storage, &mut meter),
            Err(BudgetError::WorkDeclined),
            "maximum {maximum}"
        );
        assert!(meter.work() <= maximum);
    }
    release(rows, &mut retained);
    assert_eq!(retained.bytes(), 0);
}
