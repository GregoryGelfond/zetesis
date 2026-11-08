//! Complete answer families under independently selectable candidate orderings.

use super::{ROUTES, enumeration};
use crate::Limits;
use zetesis_ferraris::{AdmissionLimits, FormulaParts, Node, Theory};

fn choices_with_requirements(selected: u8) -> Theory {
    // Every atom has an atomic choice. For each classical model M, its choices
    // freeze to facts for every member of M, so M is also reduct-minimal.
    // The optional disjunction, two implications and constraint therefore have
    // the independent truth-table reference used below.
    let nodes = vec![
        Node::atom(0),
        Node::atom(1),
        Node::atom(2),
        Node::falsum(),
        Node::implies(0, 3),
        Node::or_pair([0, 4]),
        Node::implies(1, 3),
        Node::or_pair([6, 1]),
        Node::implies(2, 3),
        Node::or_pair([2, 8]),
        Node::or_pair([0, 1]),
        Node::implies(0, 1),
        Node::implies(1, 0),
        Node::and_pair([1, 2]),
        Node::implies(13, 3),
    ];
    let mut roots = vec![5, 7, 9];
    for (bit, root) in [10, 11, 12, 14].into_iter().enumerate() {
        if selected & (1 << bit) != 0 {
            roots.push(root);
        }
    }
    Theory::new(
        3,
        FormulaParts::new(nodes, vec![]).unwrap(),
        roots,
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn expected(selected: u8) -> Vec<Vec<usize>> {
    let mut family = Vec::new();
    for interpretation in 0_u8..8 {
        let a = interpretation & 1 != 0;
        let b = interpretation & 2 != 0;
        let c = interpretation & 4 != 0;
        let requirements = [a || b, !a || b, !b || a, !(b && c)];
        if requirements
            .iter()
            .enumerate()
            .all(|(bit, &holds)| selected & (1 << bit) == 0 || holds)
        {
            family.push(
                (0..3)
                    .filter(|&atom| interpretation & (1 << atom) != 0)
                    .collect(),
            );
        }
    }
    family.sort();
    family
}

#[test]
fn every_candidate_schedule_preserves_complete_families() {
    for selected in 0..16 {
        let theory = choices_with_requirements(selected);
        let expected = expected(selected);
        for (route, workers, producers) in ROUTES {
            let mut search = enumeration(&theory, route, workers, producers, Limits::default());
            let mut actual: Vec<Vec<usize>> = search
                .by_ref()
                .map(|answer| answer.unwrap().atoms().collect())
                .collect();
            actual.sort();
            assert_eq!(
                search.statistics().regions.unwrap().generator_order.status,
                crate::GeneratorOrderStatus::Prepared
            );
            assert_eq!(
                actual, expected,
                "requirements {selected}, {route}, {workers} workers"
            );
            assert!(
                search.exhausted(),
                "requirements {selected}, {route}, {workers} workers"
            );
            assert!(
                search.statistics().regions.unwrap().counts.regions > 0,
                "this comparison must exercise candidate traversal"
            );
        }
    }
}
