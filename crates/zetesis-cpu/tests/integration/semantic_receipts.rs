//! Public native-result identity and retained candidate termination contracts.

use zetesis_core::{
    Atom, AtomPattern, GroundProgram, Interpretation, Predicate, Program, Seed, StaticLimits,
    Template,
};
use zetesis_cpu::{
    Cancellation, CandidateLimits, CandidateTermination, Candidates, Limits, Stop, check,
    check_static,
};
use zetesis_test_support::programs::program;

fn fact(name: &str) -> Template {
    Template::new(
        Some(AtomPattern::new(Predicate::new(name, 0).unwrap(), vec![]).unwrap()),
        vec![],
        vec![],
        vec![],
        vec![],
    )
}

fn compile(program: &Program) -> GroundProgram {
    GroundProgram::compile(program, StaticLimits::default()).unwrap()
}

fn expected(name: &str) -> Interpretation {
    Interpretation::new([Atom::new(Predicate::new(name, 0).unwrap(), vec![]).unwrap()]).unwrap()
}

#[test]
fn lazy_receipt_retains_the_checked_program() {
    let source = program(vec![fact("p")]);
    let independent = program(vec![fact("p")]);
    let result = check(
        &source,
        &Seed::new(&source, []).unwrap(),
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    assert!(result.program().same_instance(&source));
    assert_eq!(result.interpretation(), &expected("p"));
    let stable = result.into_stable_interpretation().unwrap();
    assert!(stable.program().same_instance(&source.clone()));
    assert!(!stable.program().same_instance(&independent));
    assert_eq!(stable.interpretation(), &expected("p"));
    assert_eq!(stable.into_interpretation(), expected("p"));
}

#[test]
fn rejected_lazy_check_cannot_yield_a_stable_receipt() {
    let source = program(vec![Template::new(None, vec![], vec![], vec![], vec![])]);
    let rejected = check(
        &source,
        &Seed::new(&source, []).unwrap(),
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap()
    .into_stable_interpretation()
    .unwrap_err();
    assert!(rejected.constraint_violated());
    assert!(rejected.program().same_instance(&source));
    assert!(rejected.interpretation().atoms().is_empty());
}

#[test]
fn seed_mismatch_cannot_yield_a_stable_receipt() {
    let predicate = Predicate::new("p", 0).unwrap();
    let atom = Atom::new(predicate.clone(), vec![]).unwrap();
    let pattern = AtomPattern::new(predicate, vec![]).unwrap();
    // The gate admits p in a seed, but the positive self-loop supplies no support.
    let source = program(vec![Template::new(
        Some(pattern.clone()),
        vec![pattern.clone()],
        vec![pattern],
        vec![],
        vec![],
    )]);
    let seed = Seed::new(&source, [atom]).unwrap();
    let lazy = check(&source, &seed, Limits::default(), &Cancellation::default()).unwrap();
    assert!(lazy.seed_mismatch());
    assert!(!lazy.constraint_violated());
    assert!(lazy.into_stable_interpretation().is_err());
    let graph = compile(&source);
    let dense = check_static(&graph, &seed, Limits::default(), &Cancellation::default()).unwrap();
    assert!(dense.seed_mismatch());
    assert!(!dense.constraint_violated());
    assert!(dense.stable_interpretation(&graph).unwrap().is_none());
}

#[test]
fn static_receipt_accepts_a_recompiled_same_instance() {
    let source = program(vec![fact("p")]);
    let graph = compile(&source);
    let result = check_static(
        &graph,
        &Seed::new(&source, []).unwrap(),
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let rebuilt = compile(&source.clone());
    assert_eq!(result.interpretation(&rebuilt).unwrap(), expected("p"));
    let stable = result.stable_interpretation(&rebuilt).unwrap().unwrap();
    assert!(stable.program().same_instance(&source));
    assert_eq!(stable.into_interpretation(), expected("p"));
}

#[test]
fn static_decoding_rejects_equal_width_foreign_graphs() {
    let source = program(vec![fact("p")]);
    let graph = compile(&source);
    let result = check_static(
        &graph,
        &Seed::new(&source, []).unwrap(),
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    // Both equal syntax and an equally wide, differently named carrier are
    // foreign instances. Length validation alone cannot distinguish them.
    for foreign in [program(vec![fact("p")]), program(vec![fact("q")])] {
        let other = compile(&foreign);
        assert_eq!(other.atoms().len(), graph.atoms().len());
        assert!(matches!(
            result.interpretation(&other),
            Err(Stop::WrongProgram)
        ));
        assert!(matches!(
            result.stable_interpretation(&other),
            Err(Stop::WrongProgram)
        ));
    }
}

#[test]
fn rejected_static_check_cannot_yield_a_stable_receipt() {
    let source = program(vec![Template::new(None, vec![], vec![], vec![], vec![])]);
    let graph = compile(&source);
    let result = check_static(
        &graph,
        &Seed::new(&source, []).unwrap(),
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap();
    assert!(result.stable_interpretation(&graph).unwrap().is_none());
    let foreign = compile(&program(vec![]));
    assert!(matches!(
        result.stable_interpretation(&foreign),
        Err(Stop::WrongProgram)
    ));
}

#[test]
fn candidate_exhaustion_remains_observable() {
    let source = program(vec![]);
    let mut candidates =
        Candidates::new(&source, CandidateLimits::default(), Cancellation::default());
    assert_eq!(candidates.termination(), None);
    assert!(candidates.next().unwrap().unwrap().atoms().is_empty());
    assert_eq!(candidates.termination(), None);
    for _ in 0..3 {
        assert!(candidates.next().is_none());
        assert_eq!(
            candidates.termination(),
            Some(CandidateTermination::Exhausted)
        );
    }
}

#[test]
fn candidate_refusal_remains_observable() {
    let source = program(vec![]);
    let mut candidates = Candidates::new(
        &source,
        CandidateLimits {
            max_candidates: 0,
            ..CandidateLimits::default()
        },
        Cancellation::default(),
    );
    assert!(matches!(candidates.next(), Some(Err(Stop::CandidateLimit))));
    for _ in 0..3 {
        assert_eq!(
            candidates.termination(),
            Some(CandidateTermination::Stopped(Stop::CandidateLimit))
        );
        assert!(candidates.next().is_none());
    }
}

#[test]
fn candidate_cancellation_remains_observable() {
    let source = program(vec![]);
    let cancellation = Cancellation::default();
    let mut candidates = Candidates::new(&source, CandidateLimits::default(), cancellation.clone());
    cancellation.cancel();
    assert!(matches!(candidates.next(), Some(Err(Stop::Cancelled))));
    for _ in 0..3 {
        assert_eq!(
            candidates.termination(),
            Some(CandidateTermination::Stopped(Stop::Cancelled))
        );
        assert!(candidates.next().is_none());
    }
}
