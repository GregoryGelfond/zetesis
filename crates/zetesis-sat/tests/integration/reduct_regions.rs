//! The proper-subset query on regions: the coverage tree over the subsets
//! of a classical model, narrowed by the frozen reduct's knowledge. Its
//! verdicts agree with the exhaustive reference and with the clause query
//! on every candidate of small theories, its witnesses are validated proper
//! subsets, and its resources stop it without a verdict.

use crate::support::formula_theories as theories;

use std::collections::BTreeSet;

use zetesis_ferraris::{Interpretation, Node, Theory, Verdict};
use zetesis_sat::{
    Cancellation, Check, Incomplete, Limits, SearchLimits, SearchMethod, check_with,
};

use theories::mixed;
use zetesis_theory_support::theories::theory;

fn interpretation(theory: &Theory, mask: usize) -> Interpretation {
    Interpretation::new(
        theory,
        (0..theory.atom_count()).filter(|atom| mask & (1 << atom) != 0),
    )
    .unwrap()
}

/// a <- b.  b <- a.  a | b | c.  A positive cycle the support law misses.
fn cycle() -> Theory {
    let nodes = vec![
        Node::Atom(0),
        Node::Atom(1),
        Node::Atom(2),
        Node::Implies(1, 0),
        Node::Implies(0, 1),
        Node::Or(0, 1),
        Node::Or(5, 2),
    ];
    theory(3, nodes, vec![3, 4, 6])
}

/// {a}. {b}. c <- a, b.  c <- c.  A self-supporting rule.
fn self_support() -> Theory {
    let nodes = vec![
        Node::Atom(0),
        Node::Atom(1),
        Node::Atom(2),
        Node::False,
        Node::Implies(0, 3),
        Node::Or(0, 4),
        Node::Implies(1, 3),
        Node::Or(1, 6),
        Node::And(0, 1),
        Node::Implies(8, 2),
        Node::Implies(2, 2),
    ];
    theory(3, nodes, vec![5, 7, 9, 10])
}

/// a <- b.  b <- a.  :- not a.  A positive loop under a negative constraint:
/// {a, b} is a classical model whose reduct the empty set models, since
/// the constraint's negative literal is false under it and freezes to falsum.
fn loop_under_constraint() -> Theory {
    let nodes = vec![
        Node::Atom(0),
        Node::Atom(1),
        Node::False,
        Node::Implies(1, 0),
        Node::Implies(0, 1),
        Node::Implies(0, 2),
        Node::Implies(5, 2),
    ];
    theory(2, nodes, vec![3, 4, 6])
}

/// m.  x.  c <- x, not not m.  c <- c.  The doubly negated literal is true
/// under a candidate holding m and falsum inside it is masked: the
/// disjunction above the masked node must still learn from it.
fn masked_inside_a_consequent() -> Theory {
    let nodes = vec![
        Node::Atom(0),       // m
        Node::Atom(1),       // x
        Node::Atom(2),       // c
        Node::False,         // 3
        Node::Implies(0, 3), // not m
        Node::Or(4, 2),      // not m | c
        Node::Implies(1, 5), // x -> (not m | c)
        Node::Implies(2, 2), // c <- c
    ];
    theory(3, nodes, vec![0, 1, 6, 7])
}

#[test]
fn a_negative_literal_false_under_the_candidate_teaches_nothing_in_the_reduct() {
    let theory = loop_under_constraint();
    let candidate = interpretation(&theory, 0b11);
    let verdict = check_with(
        &theory,
        &candidate,
        SearchMethod::Regions,
        Limits::default(),
        &Cancellation::default(),
    );
    assert!(matches!(verdict, Check::NonMinimal(ref subset) if subset.atoms().count() == 0));
}

#[test]
fn the_region_query_agrees_with_the_reference_on_every_candidate() {
    for theory in [
        mixed(),
        cycle(),
        self_support(),
        loop_under_constraint(),
        masked_inside_a_consequent(),
    ] {
        for mask in 0..(1usize << theory.atom_count()) {
            let candidate = interpretation(&theory, mask);
            let reference = zetesis_ferraris::check(
                &theory,
                &candidate,
                zetesis_ferraris::Limits::default(),
                &Cancellation::default(),
            )
            .unwrap();
            let regions = check_with(
                &theory,
                &candidate,
                SearchMethod::Regions,
                Limits::default(),
                &Cancellation::default(),
            );
            let clauses = check_with(
                &theory,
                &candidate,
                SearchMethod::Clauses,
                Limits::default(),
                &Cancellation::default(),
            );
            assert_eq!(
                regions.accepted(),
                reference.accepted(),
                "{mask}: {regions:?}"
            );
            assert_eq!(regions.accepted(), clauses.accepted(), "{mask}");
            match regions {
                Check::Stable => {}
                Check::NotModel => {
                    assert!(matches!(reference.verdict(), Verdict::NotModel { .. }));
                }
                Check::NonMinimal(subset) => {
                    assert!(matches!(reference.verdict(), Verdict::NonMinimal { .. }));
                    let atoms: BTreeSet<usize> = subset.atoms().collect();
                    let candidate_atoms: BTreeSet<usize> = candidate.atoms().collect();
                    assert!(atoms.is_subset(&candidate_atoms) && atoms != candidate_atoms);
                    assert!(
                        zetesis_ferraris::models_reduct(
                            &theory,
                            &candidate,
                            &subset,
                            zetesis_ferraris::Limits::default(),
                            &Cancellation::default()
                        )
                        .unwrap()
                    );
                }
                zetesis_sat::Check::Inconclusive(error) => panic!("{mask}: {error}"),
            }
        }
    }
}

#[test]
fn a_work_limit_stops_the_region_query_without_a_verdict() {
    let theory = mixed();
    let candidate = interpretation(&theory, 0b00011);
    let limits = Limits {
        search: SearchLimits {
            max_work: 5,
            ..SearchLimits::default()
        },
        ..Limits::default()
    };
    let verdict = check_with(
        &theory,
        &candidate,
        SearchMethod::Regions,
        limits,
        &Cancellation::default(),
    );
    assert!(matches!(
        verdict,
        Check::Inconclusive(Incomplete::WorkLimit)
    ));
}
