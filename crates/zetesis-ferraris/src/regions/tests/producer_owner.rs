//! A producer inventory authenticates its complete extraction's admission,
//! not the dimensions or structural equality of an independently admitted theory.

use zetesis_algebra::{Cancellation, Stop};

use crate::{Node, Producers, RegionLimits, Theory, test_support::theory};

fn program(roots: Vec<usize>) -> Theory {
    theory(
        2,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::implies(0, 1),
            Node::and_pair([0, 1]),
        ],
        roots,
    )
}

fn extract(owner: &Theory) -> Producers {
    crate::producers(owner, RegionLimits::default(), &Cancellation::default())
        .unwrap()
        .producers
        .unwrap()
}

#[test]
fn theory_clones_preserve_the_producer_owner() {
    let owner = program(vec![0, 2]);
    let inventory = extract(&owner);
    assert!(inventory.belongs_to(&owner));
    assert!(inventory.belongs_to(&owner.clone()));
}

#[test]
fn inventory_clones_preserve_the_producer_owner() {
    let owner = program(vec![0, 2]);
    let retained = owner.clone();
    let inventory = extract(&owner);
    let copied = inventory.clone();
    drop(inventory);
    drop(owner);
    assert!(copied.belongs_to(&retained));
}

#[test]
fn independent_equal_admissions_do_not_share_producers() {
    let owner = program(vec![0, 2]);
    let equal = program(vec![0, 2]);
    assert_eq!(owner.nodes(), equal.nodes());
    assert_eq!(owner.roots(), equal.roots());
    assert!(!owner.same_instance(&equal));
    assert!(!extract(&owner).belongs_to(&equal));
}

#[test]
fn equal_dimensions_do_not_authenticate_foreign_producers() {
    let owner = program(vec![0, 2]);
    let foreign = program(vec![1, 2]);
    assert_eq!(owner.atom_count(), foreign.atom_count());
    assert_eq!(owner.nodes().len(), foreign.nodes().len());
    assert_eq!(owner.roots().len(), foreign.roots().len());
    assert_ne!(owner.roots(), foreign.roots());
    assert!(!extract(&owner).belongs_to(&foreign));
}

#[test]
fn a_late_unsupported_root_declines_the_whole_inventory() {
    // The fact is a valid first producer, but the following conjunctive head
    // lies outside the extraction fragment. No prefix inventory escapes.
    let owner = program(vec![0, 3]);
    let extraction =
        crate::producers(&owner, RegionLimits::default(), &Cancellation::default()).unwrap();
    assert!(extraction.producers.is_none());
}

#[test]
fn exhausted_extraction_never_publishes_an_inventory() {
    let owner = program(vec![0, 2]);
    let cancellation = Cancellation::default();
    let complete = crate::producers(&owner, RegionLimits::default(), &cancellation).unwrap();
    assert!(complete.producers.unwrap().belongs_to(&owner));
    for max_work in 0..complete.work {
        let stopped = crate::producers(&owner, RegionLimits { max_work }, &cancellation);
        assert!(matches!(stopped, Err(Stop::WorkLimit)), "limit {max_work}");
    }
}

#[test]
fn cancelled_extraction_never_publishes_an_inventory() {
    let owner = program(vec![0, 2]);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    assert!(matches!(
        crate::producers(&owner, RegionLimits::default(), &cancellation),
        Err(Stop::Cancelled)
    ));
}
