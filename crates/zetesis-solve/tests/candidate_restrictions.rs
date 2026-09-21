//! Ordinary sessions use source candidate restrictions while retaining reduct checks.
use std::{collections::BTreeSet, fmt::Write, num::NonZeroUsize};
use zetesis_cpu::{Cancellation, Stop};
use zetesis_solve::{
    Backend, Completion, Grounder, Interruption, PreparedInput, Session, SolveConfig,
};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, admit_extended};

fn path() -> String {
    let mut source = String::new();
    for node in 1..=8 {
        write!(source, "node({node}).").unwrap();
    }
    for node in 1..8 {
        write!(source, "edge({node},{}).", node + 1).unwrap();
    }
    source.push_str("{in(X)} :- node(X). :- edge(X,Y), in(X), in(Y).");
    source
}
fn config(grounder: Grounder) -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        grounder,
        models: 0,
        workers: NonZeroUsize::MIN,
        ..Default::default()
    }
}

#[test]
fn ordinary_sessions_propose_only_path_answer_sets() {
    let owner = admit_extended(
        path(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
    )
    .unwrap();
    let mut families = Vec::new();
    for grounder in [Grounder::Eager, Grounder::Lazy] {
        let mut session = Session::builder(
            PreparedInput::admitted(&owner),
            config(grounder),
            Cancellation::default(),
        )
        .start()
        .unwrap();
        let family: BTreeSet<_> = session
            .by_ref()
            .map(|answer| {
                answer
                    .unwrap()
                    .interpretation()
                    .atoms()
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .collect();
        let outcome = session.outcome().unwrap();
        assert_eq!(family.len(), 55);
        assert_eq!(outcome.completion(), Some(Completion::Exhausted));
        assert_eq!(outcome.candidate_progress(), 55);
        assert!(outcome.countermodel_statistics().is_none());
        let candidates = outcome.candidate_statistics().unwrap();
        assert_eq!(candidates.restriction_conjunctions, 7);
        assert!(candidates.conflicts > 0);
        assert!(candidates.restriction_work > 0);
        families.push(family);
    }
    assert_eq!(families[0], families[1]);
}

#[test]
fn candidate_preparation_stop_is_not_an_empty_answer_family() {
    let owner = admit_extended(
        path(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
    )
    .unwrap();
    for (max_search_work, max_candidate_bytes, expected) in [
        (0, 1_000_000, Stop::WorkLimit),
        (1_000_000, 0, Stop::Allocation),
    ] {
        let mut session = Session::builder(
            PreparedInput::admitted(&owner),
            SolveConfig {
                max_search_work,
                max_candidate_bytes,
                ..config(Grounder::Lazy)
            },
            Cancellation::default(),
        )
        .start()
        .unwrap();
        assert!(session.next().is_none());
        let outcome = session.outcome().unwrap();
        assert_eq!(outcome.completion(), Some(Completion::Interrupted));
        assert_eq!(outcome.interruption(), Some(Interruption::Oracle(expected)));
        assert_eq!(outcome.candidate_progress(), 0);
        assert!(outcome.candidate_statistics().is_some());
    }
}
