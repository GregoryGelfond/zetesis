//! Theories shared by the region searches' route tests.

use zetesis_ferraris::{AdmissionLimits, FormulaParts, Node, Theory};

/// `{a0}` is an atomic choice; `a1 :- not a2` and `a2 :- not a1` form an even
/// loop that a constraint rejects together, and `a3`, `a4` each require both
/// through their own conjunction, so the loop atoms keep more unknown parents
/// than the choice head. Its answer sets are `{a1}` and `{a2}`, each with or
/// without `a0`.
pub(crate) fn choice_beside_a_loop() -> Theory {
    Theory::new(
        5,
        FormulaParts::new(
            vec![
                Node::atom(0),
                Node::atom(1),
                Node::atom(2),
                Node::atom(3),
                Node::atom(4),
                Node::falsum(),
                Node::implies(0, 5),
                Node::or_pair([0, 6]),
                Node::implies(2, 5),
                Node::implies(8, 1),
                Node::implies(1, 5),
                Node::implies(10, 2),
                Node::and_pair([1, 2]),
                Node::implies(12, 5),
                Node::and_pair([1, 2]),
                Node::implies(14, 3),
                Node::and_pair([1, 2]),
                Node::implies(16, 4),
            ],
            vec![],
        )
        .unwrap(),
        vec![7, 9, 11, 13, 15, 17],
        AdmissionLimits::default(),
    )
    .unwrap()
}
