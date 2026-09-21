//! The shared selection and owned materialization doors use one candidate cursor.

use zetesis_core::{AdmissionLimits, AtomPattern, Predicate, Program, Template, Term, Value};
use zetesis_cpu::{
    Cancellation, CandidateLimits, CandidateRestrictionLimits, CandidateTermination, Candidates,
    Stop,
};

fn program() -> Program {
    let mut rules: Vec<_> = ["a", "b", "c"]
        .into_iter()
        .map(|name| {
            let atom = AtomPattern::new(
                Predicate::new(name, 1).unwrap(),
                vec![Term::Constant(Value::String("shared-payload".repeat(256)))],
            )
            .unwrap();
            Template::new(Some(atom.clone()), vec![], vec![atom], vec![], vec![])
        })
        .collect();
    let positive = rules[..2]
        .iter()
        .map(|rule| rule.head().unwrap().clone())
        .collect();
    rules.push(Template::new(None, positive, vec![], vec![], vec![]));
    Program::new(rules, AdmissionLimits::default()).unwrap()
}

fn candidates(source: &Program, limits: CandidateLimits) -> Candidates<'_> {
    Candidates::restricted(
        source,
        limits,
        CandidateRestrictionLimits::default(),
        Cancellation::default(),
    )
}

#[test]
fn mixed_selection_and_owned_pulls_preserve_the_restricted_sequence() {
    let source = program();
    let expected: Vec<_> = candidates(&source, CandidateLimits::default())
        .map(|seed| seed.unwrap().atoms().clone())
        .collect();
    let names: Vec<Vec<_>> = expected
        .iter()
        .map(|atoms| atoms.iter().map(|atom| atom.predicate().name()).collect())
        .collect();
    assert_eq!(
        names,
        [
            vec![],
            vec!["a"],
            vec!["b"],
            vec!["c"],
            vec!["a", "c"],
            vec!["b", "c"]
        ]
    );
    let mut mixed = candidates(&source, CandidateLimits::default());
    for (index, expected) in expected.into_iter().enumerate() {
        let actual = if index % 2 == 0 {
            mixed.next_selection().unwrap().unwrap().to_seed()
        } else {
            mixed.next().unwrap().unwrap()
        };
        assert_eq!(actual.atoms(), &expected);
    }
    assert!(mixed.next_selection().is_none());
    assert!(mixed.next().is_none());
    assert_eq!(mixed.termination(), Some(CandidateTermination::Exhausted));
}

#[test]
fn later_carries_and_batches_share_the_original_atom_payload() {
    let source = program();
    let mut candidates = candidates(&source, CandidateLimits::default());
    let empty = candidates.next_selection().unwrap().unwrap();
    assert_eq!(empty.view().atoms().len(), 0);
    assert_eq!(candidates.discovered_atoms(), 0);
    let first = candidates.next_selection().unwrap().unwrap();
    let atom = first.view().atoms().next().unwrap();
    let mut repeated = 0;
    while let Some(selection) = candidates.next_selection() {
        let selection = selection.unwrap();
        if let Some(stored) = selection.view().atoms().find(|stored| *stored == atom) {
            assert!(std::ptr::eq(stored, atom));
            assert_eq!(stored.values().as_ptr(), atom.values().as_ptr());
            repeated += 1;
        }
    }
    assert!(repeated > 0);
    drop(candidates);
    assert_eq!(first.view().atoms().next().unwrap(), atom);
}

#[test]
fn both_candidate_doors_share_inclusive_limits_and_terminal_errors() {
    let source = program();
    for maximum in 0..=6 {
        let limits = CandidateLimits {
            max_candidates: maximum,
            ..CandidateLimits::default()
        };
        let mut owned = candidates(&source, limits);
        let mut shared = candidates(&source, limits);
        loop {
            let a = owned.next();
            let b = shared
                .next_selection()
                .map(|result| result.map(|selection| selection.to_seed()));
            match (a, b) {
                (Some(Ok(a)), Some(Ok(b))) => assert_eq!(a.atoms(), b.atoms()),
                (Some(Err(a)), Some(Err(b))) => {
                    assert_eq!(a, b);
                    break;
                }
                (None, None) => break,
                _ => panic!("different candidate boundaries"),
            }
        }
        assert_eq!(owned.termination(), shared.termination());
        assert!(owned.next_selection().is_none());
        assert!(shared.next().is_none());
    }
}

#[test]
fn carrier_refusal_is_shared_after_the_first_empty_candidate() {
    let source = program();
    let mut candidates = candidates(
        &source,
        CandidateLimits {
            max_carrier_atoms: 0,
            ..CandidateLimits::default()
        },
    );
    assert_eq!(
        candidates
            .next_selection()
            .unwrap()
            .unwrap()
            .view()
            .atoms()
            .len(),
        0
    );
    assert!(matches!(candidates.next(), Some(Err(Stop::CarrierLimit))));
    assert!(candidates.next_selection().is_none());
    assert_eq!(
        candidates.termination(),
        Some(CandidateTermination::Stopped(Stop::CarrierLimit))
    );
}

#[test]
fn discovered_positions_preserve_static_checks_across_batches() {
    use zetesis_core::{GroundProgram, StaticLimits};
    use zetesis_cpu::{Limits, check_static, check_static_view};
    let program = program();
    let graph = GroundProgram::compile(&program, StaticLimits::default()).unwrap();
    let mut candidates = candidates(&program, CandidateLimits::default());
    let mut retained = Vec::new();
    while let Some(selection) = candidates.next_selection() {
        retained.push(selection.unwrap());
    }
    assert_eq!(retained.len(), 6);
    drop(candidates);
    for selection in retained.iter().rev().chain(&retained) {
        let owned = selection.to_seed();
        let reference =
            check_static(&graph, &owned, Limits::default(), &Cancellation::default()).unwrap();
        let actual = check_static_view(
            &graph,
            selection.view(),
            Limits::default(),
            &Cancellation::default(),
        )
        .unwrap();
        assert!(actual.accepted());
        assert_eq!(
            graph
                .model_from_words(actual.closure_words())
                .unwrap()
                .atoms()
                .iter()
                .collect::<Vec<_>>(),
            owned.atoms().iter().collect::<Vec<_>>()
        );
        assert_eq!(actual.statistics(), reference.statistics());
    }
}
