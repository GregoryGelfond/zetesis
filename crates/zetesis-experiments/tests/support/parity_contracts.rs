//! Parity compares every semantic result field, not only acceptance.

use std::error::Error;

use zetesis_core::{
    AdmissionLimits, AtomPattern, GroundProgram, Predicate, Program, Seed, StaticLimits, Template,
};
use zetesis_cpu::{Cancellation, Limits, StaticCheck, check_static};

use super::{BenchmarkError, cpu_parity};

#[derive(Clone, Copy)]
enum Case {
    Fact,
    UnsupportedCycle,
    ViolatedConstraint,
    SeedMismatch,
}

fn check(case: Case) -> StaticCheck {
    let atom = AtomPattern::new(Predicate::new("a", 0).unwrap(), Vec::new()).unwrap();
    let mut templates = vec![Template::new(
        Some(atom.clone()),
        if matches!(case, Case::UnsupportedCycle) {
            vec![atom.clone()]
        } else {
            vec![]
        },
        vec![],
        vec![],
        vec![],
    )];
    if matches!(case, Case::ViolatedConstraint) {
        templates.push(Template::new(
            None,
            vec![atom.clone()],
            vec![],
            vec![],
            vec![],
        ));
    }
    if matches!(case, Case::SeedMismatch) {
        templates.push(Template::new(
            Some(atom.clone()),
            vec![],
            vec![atom],
            vec![],
            vec![],
        ));
    }
    let program = Program::new(templates, AdmissionLimits::default()).unwrap();
    let graph = GroundProgram::compile(&program, StaticLimits::default()).unwrap();
    check_static(
        &graph,
        &Seed::new(&program, []).unwrap(),
        Limits::default(),
        &Cancellation::default(),
    )
    .unwrap()
}

#[test]
fn parity_rejects_length_closure_constraint_and_seed_disagreements() {
    let baseline = check(Case::Fact);
    cpu_parity(
        std::slice::from_ref(&baseline),
        std::slice::from_ref(&baseline),
    )
    .unwrap();
    cpu_parity(&[], &[]).unwrap();
    let empty = check(Case::UnsupportedCycle);
    let rejected = check(Case::ViolatedConstraint);
    let mismatch = check(Case::SeedMismatch);
    assert_ne!(baseline.closure_words(), empty.closure_words());
    assert_eq!(baseline.closure_words(), rejected.closure_words());
    assert!(!baseline.constraint_violated());
    assert!(rejected.constraint_violated());
    assert_eq!(baseline.closure_words(), mismatch.closure_words());
    assert!(!baseline.seed_mismatch());
    assert!(mismatch.seed_mismatch());
    for actual in [
        vec![],
        vec![baseline.clone(), baseline.clone()],
        vec![empty],
        vec![rejected],
        vec![mismatch],
    ] {
        let error = cpu_parity(std::slice::from_ref(&baseline), &actual).unwrap_err();
        assert!(matches!(error, BenchmarkError::Parity));
        assert!(error.source().is_none());
        assert_eq!(error.to_string(), "backend result mismatch");
    }
}
