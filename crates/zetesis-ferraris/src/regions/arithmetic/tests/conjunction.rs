//! A borrowed entry saves its read without dropping occurrence evidence.

use zetesis_cpu::{Cancellation, Stop};

use crate::{
    AdmissionError, NodeView, Theory,
    regions::{
        Work,
        arithmetic::{
            budget::{Budget, BudgetError},
            conjunction::Conjunction,
            storage::Storage,
        },
    },
    test_support::{constructed_theory, push},
};

fn budget(cancellation: &Cancellation) -> Budget<'_, 'static> {
    Budget::new(Work::new(u64::MAX), u64::MAX, usize::MAX, cancellation)
}

fn repeated_body() -> Theory {
    constructed_theory(3, |nodes| {
        let pair = push(nodes, NodeView::And(&[0, 1]));
        vec![push(nodes, NodeView::And(&[pair, pair, 2]))]
    })
}

fn observe(
    owner: &Theory,
    body: usize,
    borrowed: bool,
    reject: Option<usize>,
) -> (Result<bool, BudgetError>, Vec<usize>, u64) {
    let cancellation = Cancellation::default();
    let mut meter = budget(&cancellation);
    let mut storage = Storage::default();
    let mut walk = Conjunction::new(&mut storage, &mut meter).unwrap();
    // Both callers have inspected their entry before deciding to traverse it.
    let view = meter.node(owner, body).unwrap();
    let mut leaves = Vec::new();
    let visit = |node, _, _: &mut Storage, _: &mut Budget<'_, '_>| {
        leaves.push(node);
        Ok(reject != Some(node))
    };
    let result = if borrowed {
        walk.walk_view(owner, (body, view), &mut storage, &mut meter, visit)
    } else {
        walk.walk(owner, body, &mut storage, &mut meter, visit)
    };
    walk.release(&mut storage).unwrap();
    assert_eq!(storage.bytes(), 0);
    (result, leaves, meter.work())
}

#[test]
fn a_borrowed_entry_preserves_repeated_occurrences() {
    let owner = repeated_body();
    for borrowed in [false, true] {
        let (result, leaves, _) = observe(&owner, owner.roots()[0], borrowed, None);
        assert_eq!(result, Ok(true));
        assert_eq!(leaves, [0, 1, 0, 1, 2]);
    }
}

#[test]
fn a_borrowed_entry_saves_only_its_canonical_read() {
    let owner = repeated_body();
    // Atom, inline conjunction and stored conjunction have distinct read costs.
    for (entry, saved) in [(0, 4), (3, 5), (owner.roots()[0], 12)] {
        let (read_result, read_leaves, read_work) = observe(&owner, entry, false, None);
        let (borrowed_result, borrowed_leaves, borrowed_work) = observe(&owner, entry, true, None);
        assert_eq!(read_result, Ok(true));
        assert_eq!(borrowed_result, read_result);
        assert_eq!(borrowed_leaves, read_leaves);
        assert_eq!(read_work - borrowed_work, saved);
    }
}

#[test]
fn a_rejected_occurrence_stops_both_entry_forms() {
    let owner = repeated_body();
    for borrowed in [false, true] {
        let (result, leaves, _) = observe(&owner, owner.roots()[0], borrowed, Some(1));
        assert_eq!(result, Ok(false));
        assert_eq!(leaves, [0, 1]);
    }
}

#[test]
fn borrowing_an_entry_does_not_bypass_cancellation() {
    let owner = repeated_body();
    let cancellation = Cancellation::default();
    let mut meter = budget(&cancellation);
    let mut storage = Storage::default();
    let mut walk = Conjunction::new(&mut storage, &mut meter).unwrap();
    let root = owner.roots()[0];
    let view = meter.node(&owner, root).unwrap();
    let work = meter.work();
    cancellation.cancel();
    let mut visited = false;
    let result = walk.walk_view(
        &owner,
        (root, view),
        &mut storage,
        &mut meter,
        |_, _, _, _| {
            visited = true;
            Ok(true)
        },
    );
    assert_eq!(result, Err(BudgetError::Stopped(Stop::Cancelled)));
    assert!(!visited);
    assert_eq!(meter.work(), work);
    walk.release(&mut storage).unwrap();
    assert_eq!(storage.bytes(), 0);
}

#[test]
fn an_absent_entry_keeps_its_typed_read_failure() {
    let owner = repeated_body();
    let cancellation = Cancellation::default();
    let mut meter = budget(&cancellation);
    let mut storage = Storage::default();
    let mut walk = Conjunction::new(&mut storage, &mut meter).unwrap();
    let work = meter.work();
    let result = walk.walk(
        &owner,
        owner.nodes().len(),
        &mut storage,
        &mut meter,
        |_, _, _, _| panic!("an absent entry must not be visited"),
    );
    assert_eq!(result, Err(BudgetError::Source(AdmissionError::Edge)));
    assert_eq!(meter.work() - work, 4);
    walk.release(&mut storage).unwrap();
    assert_eq!(storage.bytes(), 0);
}
