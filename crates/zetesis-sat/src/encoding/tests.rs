//! Reused storage never supplies candidate-dependent clauses or truth values.

use super::*;
use crate::{Cancellation, SearchLimits, SearchStatistics};
use zetesis_ferraris::Node;

fn input() -> Theory {
    Theory::new(
        3,
        zetesis_ferraris::FormulaParts::new(
            vec![
                Node::atom(0),
                Node::atom(1),
                Node::atom(2),
                Node::falsum(),
                Node::implies(0, 3),
                Node::or_pair([0, 4]),
                Node::or_pair([1, 2]),
                Node::and_pair([5, 6]),
            ],
            vec![],
        )
        .unwrap(),
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
    workspace
        .reserve(&theory, limits, &mut budget(&cancellation, u64::MAX))
        .unwrap();
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
    workspace
        .reserve(&theory, limits, &mut budget(&cancellation, u64::MAX))
        .unwrap();
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
