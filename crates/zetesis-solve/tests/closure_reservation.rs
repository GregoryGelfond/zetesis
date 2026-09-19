//! The collective closure ceiling is validated against its worker product
//! before a session starts, never discovered by the first batch.

use std::num::NonZeroUsize;

use zetesis_core::{AdmissionLimits, AtomPattern, Predicate, Program, Template};
use zetesis_cpu::Control;
use zetesis_solve::{Backend, Completion, PreparedInput, Session, SolveConfig, SolveError};

/// Five independent choices: thirty-two candidates, all answer sets.
fn program() -> Program {
    let choice = |name: &str| {
        let atom = AtomPattern::new(Predicate::new(name, 0).unwrap(), vec![]).unwrap();
        Template::new(Some(atom.clone()), vec![], vec![atom], vec![], vec![])
    };
    Program::new(
        ["a", "b", "c", "d", "e"].map(choice).to_vec(),
        AdmissionLimits::default(),
    )
    .unwrap()
}

fn with_workers(workers: usize) -> SolveConfig {
    SolveConfig {
        backend: Backend::Cpu,
        models: 0,
        workers: NonZeroUsize::new(workers).unwrap(),
        ..Default::default()
    }
}

#[test]
fn a_worker_product_above_the_collective_ceiling_is_refused_by_name() {
    let config = with_workers(5);
    assert!(matches!(
        config.validate(),
        Err(SolveError::ClosureReservation {
            workers: 5,
            max_closure_bytes: 134_217_728,
            max_closure_batch_bytes: 536_870_912,
        })
    ));
    let text = config.validate().unwrap_err().to_string();
    assert!(text.contains('5') && text.contains("671088640"), "{text}");
}

#[test]
fn a_product_at_the_ceiling_is_admitted() {
    assert!(with_workers(4).validate().is_ok());
    assert!(
        SolveConfig::for_allowance(SolveConfig::REFERENCE_MEMORY, NonZeroUsize::new(8).unwrap())
            .validate()
            .is_ok()
    );
}

#[test]
fn eight_workers_complete_the_family_at_their_collective_share() {
    let config = SolveConfig {
        backend: Backend::Cpu,
        models: 0,
        ..SolveConfig::for_allowance(SolveConfig::REFERENCE_MEMORY, NonZeroUsize::new(8).unwrap())
    };
    let program = program();
    let mut session =
        Session::builder(PreparedInput::program(&program), config, Control::default())
            .start()
            .unwrap();
    let mut answers = 0;
    for answer in session.by_ref() {
        answer.unwrap();
        answers += 1;
    }
    assert_eq!(answers, 32);
    assert_eq!(
        session.outcome().unwrap().completion(),
        Some(Completion::Exhausted)
    );
}
