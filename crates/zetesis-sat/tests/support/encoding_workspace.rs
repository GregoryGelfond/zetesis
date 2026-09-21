//! Reused storage never supplies candidate-dependent clauses or truth values.

use super::*;
use crate::{Cancellation, SearchLimits, SearchStatistics};

fn input() -> Theory {
    Theory::new(
        3,
        vec![
            Node::Atom(0),
            Node::Atom(1),
            Node::Atom(2),
            Node::False,
            Node::Implies(0, 3),
            Node::Or(0, 4),
            Node::Or(1, 2),
            Node::And(5, 6),
        ],
        vec![7],
        zetesis_ferraris::AdmissionLimits::default(),
    )
    .unwrap()
}

fn budget(cancellation: &Cancellation, max_work: u64) -> Budget<'_> {
    Budget {
        quota: crate::search::LocalQuota,
        limits: SearchLimits {
            max_work,
            ..Default::default()
        },
        cancellation,
        statistics: SearchStatistics::default(),
    }
}

fn contents(cnf: &Cnf) -> (usize, Vec<Vec<Literal>>) {
    (
        cnf.variables(),
        cnf.clauses()
            .map(|clause| clause.iter().collect())
            .collect(),
    )
}

#[test]
fn interrupted_reencoding_matches_fresh_candidate_state() {
    let theory = input();
    let cancellation = Cancellation::default();
    let limits = AdmissionLimits::default();
    let mut workspace = Workspace::default();
    workspace.reserve(&theory, limits).unwrap();
    let retained = workspace.retained_bytes();
    for mask in [7, 0, 5, 2, 7, 1, 6, 3, 4] {
        let candidate =
            Interpretation::new(&theory, (0..3).filter(|atom| mask & (1 << atom) != 0)).unwrap();
        for ceiling in 0..=180 {
            let mut fresh = budget(&cancellation, ceiling);
            let expected =
                encode(&theory, Some(&candidate), limits, &mut fresh).map(|cnf| contents(&cnf));
            let mut reused = budget(&cancellation, ceiling);
            let actual = workspace
                .encode(&theory, Some(&candidate), limits, &mut reused)
                .map(contents);
            assert_eq!(actual, expected, "candidate {mask}, ceiling {ceiling}");
            assert_eq!(reused.statistics, fresh.statistics);
            assert_eq!(workspace.retained_bytes(), retained);
        }
    }
}

#[test]
fn encoding_reuses_the_reserved_vector_allocations() {
    let theory = input();
    let limits = AdmissionLimits::default();
    let cancellation = Cancellation::default();
    let mut workspace = Workspace::default();
    workspace.reserve(&theory, limits).unwrap();
    workspace
        .encode(&theory, None, limits, &mut budget(&cancellation, 10_000))
        .unwrap();
    let pointers = (
        workspace.mask.as_ptr(),
        workspace.nodes.as_ptr(),
        workspace.strict.as_ptr(),
        workspace.cnf.as_ref().unwrap().clause_at(0).0.as_ptr(),
    );
    for mask in [7, 0, 4, 3, 7, 2, 1] {
        let candidate =
            Interpretation::new(&theory, (0..3).filter(|atom| mask & (1 << atom) != 0)).unwrap();
        workspace
            .encode(
                &theory,
                Some(&candidate),
                limits,
                &mut budget(&cancellation, 10_000),
            )
            .unwrap();
        assert_eq!(
            (
                workspace.mask.as_ptr(),
                workspace.nodes.as_ptr(),
                workspace.strict.as_ptr(),
                workspace.cnf.as_ref().unwrap().clause_at(0).0.as_ptr()
            ),
            pointers
        );
    }
}
