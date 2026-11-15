//! Circular producers are exactly the sole-head producers whose body requires
//! their head or an atom derived only through it.

use crate::{Node, RegionLimits, test_support::theory};
use zetesis_algebra::Cancellation;

/// The mark of every producer of a theory, in root order.
fn marks(atoms: usize, nodes: Vec<Node>, roots: Vec<usize>) -> Vec<bool> {
    let producers = super::super::producers(
        &theory(atoms, nodes, roots),
        RegionLimits::default(),
        &Cancellation::default(),
    )
    .unwrap()
    .producers
    .expect("every root is a rule of the support fragment");
    (0..producers.rules.len())
        .map(|row| producers.circular(row))
        .collect()
}

/// Atoms `e`, `a` and `b` as nodes 0, 1 and 2.
fn atoms() -> Vec<Node> {
    vec![Node::atom(0), Node::atom(1), Node::atom(2)]
}

#[test]
fn an_alias_pair_marks_the_rule_back_to_its_source() {
    // a ← e.  b ← a.  a ← b.
    let mut nodes = atoms();
    nodes.extend([
        Node::implies(0, 1),
        Node::implies(1, 2),
        Node::implies(2, 1),
    ]);
    assert_eq!(marks(3, nodes, vec![3, 4, 5]), [false, false, true]);
}

#[test]
fn a_pair_without_another_producer_marks_both_rules() {
    // b ← a.  a ← b. Each atom is derived only through the other.
    let mut nodes = atoms();
    nodes.extend([Node::implies(1, 2), Node::implies(2, 1)]);
    assert_eq!(marks(3, nodes, vec![3, 4]), [true, true]);
}

#[test]
fn a_rule_requiring_its_own_head_is_marked() {
    // a ← e.  a ← a ∧ e. The other producer keeps `a` from being derived
    // only through itself; the second rule requires `a` all the same.
    let mut nodes = atoms();
    nodes.extend([
        Node::implies(0, 1),
        Node::and_pair([1, 0]),
        Node::implies(4, 1),
    ]);
    assert_eq!(marks(3, nodes, vec![3, 5]), [false, true]);
}

#[test]
fn another_producer_of_the_alias_keeps_the_rule() {
    // a ← e.  b ← a.  b ← e.  a ← b.
    let mut nodes = atoms();
    nodes.extend([
        Node::implies(0, 1),
        Node::implies(1, 2),
        Node::implies(0, 2),
        Node::implies(2, 1),
    ]);
    assert_eq!(marks(3, nodes, vec![3, 4, 5, 6]), [false; 4]);
}

#[test]
fn a_fact_for_the_alias_keeps_the_rule_out_of_it() {
    // b.  b ← a.  a ← b. The fact supports `b`, so `a ← b` stays. Nothing
    // but `a ← b` produces `a`, so `b ← a` is the circular one.
    let mut nodes = atoms();
    nodes.extend([Node::implies(1, 2), Node::implies(2, 1)]);
    assert_eq!(marks(3, nodes, vec![2, 3, 4]), [false, true, false]);
}

#[test]
fn an_atom_without_producers_derives_nothing() {
    // a ← b, and nothing produces b.
    let mut nodes = atoms();
    nodes.push(Node::implies(2, 1));
    assert_eq!(marks(3, nodes, vec![3]), [false]);
}

#[test]
fn a_conjunction_requires_each_atom_operand() {
    // a ← e.  b ← a ∧ e.  a ← b.
    let mut nodes = atoms();
    nodes.extend([
        Node::implies(0, 1),
        Node::and_pair([1, 0]),
        Node::implies(4, 2),
        Node::implies(2, 1),
    ]);
    assert_eq!(marks(3, nodes, vec![3, 5, 6]), [false, false, true]);
}

#[test]
fn a_disjunction_requires_nothing() {
    // a ← e.  b ← a ∨ e.  a ← b.
    let mut nodes = atoms();
    nodes.extend([
        Node::implies(0, 1),
        Node::or_pair([1, 0]),
        Node::implies(4, 2),
        Node::implies(2, 1),
    ]);
    assert_eq!(marks(3, nodes, vec![3, 5, 6]), [false; 3]);
}

#[test]
fn a_nested_conjunction_is_not_opened() {
    // a ← e.  b ← (a ∧ e) ∧ e.  a ← b. The law would grant the mark; the
    // reading that stops at a body's own operands leaves it unused.
    let mut nodes = atoms();
    nodes.extend([
        Node::implies(0, 1),
        Node::and_pair([1, 0]),
        Node::and_pair([4, 0]),
        Node::implies(5, 2),
        Node::implies(2, 1),
    ]);
    assert_eq!(marks(3, nodes, vec![3, 6, 7]), [false; 3]);
}

#[test]
fn a_choice_of_the_alias_counts_as_its_producer() {
    // a ← e.  {b} ← a.  a ← b.
    let mut nodes = atoms();
    nodes.extend([
        Node::implies(0, 1),
        Node::falsum(),
        Node::implies(2, 4),
        Node::or_pair([2, 5]),
        Node::implies(1, 6),
        Node::implies(2, 1),
    ]);
    assert_eq!(marks(3, nodes, vec![3, 7, 8]), [false, false, true]);
}

#[test]
fn a_circular_choice_is_marked() {
    // a ← e.  b ← a.  {a} ← b.
    let mut nodes = atoms();
    nodes.extend([
        Node::implies(0, 1),
        Node::implies(1, 2),
        Node::falsum(),
        Node::implies(1, 5),
        Node::or_pair([1, 6]),
        Node::implies(2, 7),
    ]);
    assert_eq!(marks(3, nodes, vec![3, 4, 8]), [false, false, true]);
}

#[test]
fn a_producer_with_two_heads_is_never_marked() {
    // a ← e.  b ← a.  a ∨ e ← b. With the one head `a` the last rule would
    // be marked, as in the alias pair.
    let mut nodes = atoms();
    nodes.extend([
        Node::implies(0, 1),
        Node::implies(1, 2),
        Node::or_pair([1, 0]),
        Node::implies(2, 5),
    ]);
    assert_eq!(marks(3, nodes, vec![3, 4, 6]), [false; 3]);
}

#[test]
fn a_disjunctive_producer_of_the_alias_counts_when_it_requires_the_head() {
    // a ← e.  b ∨ c ← a.  a ← b, with `c` a fourth atom.
    let mut nodes = atoms();
    nodes.extend([
        Node::atom(3),
        Node::implies(0, 1),
        Node::or_pair([2, 3]),
        Node::implies(1, 5),
        Node::implies(2, 1),
    ]);
    assert_eq!(marks(4, nodes, vec![4, 6, 7]), [false, false, true]);
}

#[test]
fn each_head_is_examined_against_its_own_dependents() {
    // b ← a.  a ← b.  a ← e.  e ← a. Both `b` and `e` are derived only
    // through `a`, so each producer of `a` requires such an atom and is
    // marked. Neither `b` nor `e` has `a` derived only through it, because
    // the other one also produces `a`: the rules out of `a` stay.
    let mut nodes = atoms();
    nodes.extend([
        Node::implies(1, 2),
        Node::implies(2, 1),
        Node::implies(0, 1),
        Node::implies(1, 0),
    ]);
    assert_eq!(
        marks(3, nodes, vec![3, 4, 5, 6]),
        [false, true, true, false]
    );
}

#[test]
fn a_dependent_of_one_head_is_not_one_of_another() {
    // b ← a.  a ← b.  a ← e.  c ← b, with `c` a fourth atom. `b` is derived
    // only through `a`, which marks `a ← b`; it is not derived through `c`,
    // so `c ← b` stays, whichever head is examined first.
    let mut nodes = atoms();
    nodes.extend([
        Node::atom(3),
        Node::implies(1, 2),
        Node::implies(2, 1),
        Node::implies(0, 1),
        Node::implies(2, 3),
    ]);
    assert_eq!(
        marks(4, nodes, vec![4, 5, 6, 7]),
        [false, true, false, false]
    );
}
