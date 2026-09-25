//! Observe execution choices while solving a native program through a session.

// ANCHOR: example
use std::convert::Infallible;
use zetesis_core::{AdmissionLimits, Atom, AtomPattern, Model, Predicate, Program, Template};
use zetesis_cpu::Cancellation;
use zetesis_solve::{
    Backend, Completion, ExecutionObservation, ExecutionObserver, Grounder, PreparedInput, Session,
    SolveConfig,
};

#[derive(Default)]
struct Preparation {
    grounder: Option<Grounder>,
}

impl ExecutionObserver for Preparation {
    type Error = Infallible;

    fn observe(&mut self, event: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        if let ExecutionObservation::CpuClosure { grounder, .. } = event {
            self.grounder = Some(grounder);
        }
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ready = Predicate::new("ready", 0)?;
    let fact = Template::new(
        Some(AtomPattern::new(ready.clone(), vec![])?),
        vec![],
        vec![],
        vec![],
        vec![],
    );
    let program = Program::new(vec![fact], AdmissionLimits::default())?;
    let config = SolveConfig {
        backend: Backend::Cpu,
        grounder: Grounder::Lazy,
        models: 0,
        ..SolveConfig::default()
    };
    let mut preparation = Preparation::default();
    let mut session = Session::enumerate_observed(
        PreparedInput::program(&program),
        config,
        Cancellation::default(),
        &mut preparation,
    )?;
    assert_eq!(preparation.grounder, Some(Grounder::Lazy));

    let answer = session.next_observed(&mut preparation).unwrap()?;
    assert_eq!(
        answer.interpretation(),
        &Model::new([Atom::new(ready, vec![])?])?
    );
    assert!(session.next_observed(&mut preparation).is_none());
    assert_eq!(
        session.outcome().unwrap().completion(),
        Some(Completion::Exhausted)
    );
    Ok(())
}
// ANCHOR_END: example
