//! A dependency component is released when each of its cycles passes
//! through a circular producer, and kept whole when one survives without them.

use super::*;

/// Free choices `e` and `f` (atoms 0 and 1); a pair `g`, `h` (atoms 2 and 3)
/// founded twice, `g ← h`, `h ← g`, `g ← e`, `h ← f`; and an alias pair `a`,
/// `b` (atoms 4 and 5) founded once, `a ← e`, `b ← a`, `a ← b`.
fn loop_beside_alias() -> Theory {
    theory(
        6,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::atom(3),
            Node::atom(4),
            Node::atom(5),
            Node::falsum(),
            Node::implies(0, 6),
            Node::or_pair([0, 7]),
            Node::implies(1, 6),
            Node::or_pair([1, 9]),
            Node::implies(3, 2),
            Node::implies(2, 3),
            Node::implies(0, 2),
            Node::implies(1, 3),
            Node::implies(0, 4),
            Node::implies(4, 5),
            Node::implies(5, 4),
        ],
        vec![8, 10, 11, 12, 13, 14, 15, 16, 17],
    )
}

/// The inventory as the search reads it, circular producers marked.
fn marked(owner: &Theory) -> Producers {
    crate::producers(owner, RegionLimits::default(), &Cancellation::default())
        .unwrap()
        .producers
        .unwrap()
}

#[test]
fn only_the_loop_founded_twice_has_cyclic_heads() {
    let owner = loop_beside_alias();
    let index = Narrower::new(&owner);
    let producers = marked(&owner);
    let mut scratch = Scratch::default();
    scratch.prepare_projected(&owner, &index, &producers);
    assert_eq!(&*scratch.cyclic_atoms, [0b00_1100]);
}

#[test]
fn every_loop_has_cyclic_heads_when_no_producer_is_marked() {
    let owner = loop_beside_alias();
    let index = Narrower::new(&owner);
    let producers = super::producers(&owner);
    let mut scratch = Scratch::default();
    scratch.prepare_projected(&owner, &index, &producers);
    assert_eq!(&*scratch.cyclic_atoms, [0b11_1100]);
}

#[test]
fn the_alias_pair_falls_by_support_beside_a_loop_cut_by_the_closure() {
    let owner = loop_beside_alias();
    let index = Narrower::new(&owner);
    let producers = marked(&owner);
    let mut region = Region::all_open(owner.atom_count());
    region.cut(0);
    region.cut(1);
    let attempt = index.narrow_known(
        OriginalSubject::new(&owner, Some(&producers)),
        &mut region,
        &mut index.knowledge(),
        &mut NarrowingScratch::default(),
        RegionLimits::default(),
        &Cancellation::default(),
    );
    assert_eq!(attempt.result.unwrap(), Narrowing::Fixed { changed: true });
    assert!((2..6).all(|atom| region.is_cut(atom)));
    assert_eq!(producers.founded_cycles.get(), Some(&true));
}

/// A loop of three, `a ← c`, `c ← b`, `b ← a` (atoms 0, 1 and 2 as `a`, `b`,
/// `c`), and an alias `x ← a`, `a ← x` (atom 3) that hangs on it. Only
/// `a ← x` is circular.
fn loop_holding_an_alias() -> Theory {
    theory(
        4,
        vec![
            Node::atom(0),
            Node::atom(1),
            Node::atom(2),
            Node::atom(3),
            Node::implies(2, 0),
            Node::implies(1, 2),
            Node::implies(0, 1),
            Node::implies(0, 3),
            Node::implies(3, 0),
        ],
        vec![4, 5, 6, 7, 8],
    )
}

#[test]
fn a_surviving_cycle_keeps_the_alias_it_holds() {
    let owner = loop_holding_an_alias();
    let index = Narrower::new(&owner);
    let producers = marked(&owner);
    let mut scratch = Scratch::default();
    scratch.prepare_projected(&owner, &index, &producers);
    assert_eq!(&*scratch.cyclic_atoms, [0b1111]);
}

#[test]
fn an_unfounded_loop_falls_with_the_alias_it_holds() {
    // The alias reads as a seed if it leaves the loop's component, and the
    // circular rule would then found the loop from it.
    let owner = loop_holding_an_alias();
    let index = Narrower::new(&owner);
    let producers = marked(&owner);
    let mut region = Region::all_open(owner.atom_count());
    let attempt = index.narrow_known(
        OriginalSubject::new(&owner, Some(&producers)),
        &mut region,
        &mut index.knowledge(),
        &mut NarrowingScratch::default(),
        RegionLimits::default(),
        &Cancellation::default(),
    );
    assert_eq!(attempt.result.unwrap(), Narrowing::Fixed { changed: true });
    assert!((0..4).all(|atom| region.is_cut(atom)));
}
