//! Observe execution choices while solving an admitted formula through a session.

// ANCHOR: example
use std::convert::Infallible;
use zetesis_algebra::Cancellation;
use zetesis_core::{Atom, Model, Predicate};
use zetesis_solve::{
    Backend, Completion, ExecutionObservation, ExecutionObserver, Grounder, PreparedInput, Session,
    SolveConfig,
};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

#[derive(Default)]
struct Preparation {
    grounder: Option<Grounder>,
}

impl ExecutionObserver for Preparation {
    type Error = Infallible;

    fn observe(&mut self, event: ExecutionObservation<'_>) -> Result<(), Self::Error> {
        if let ExecutionObservation::CpuFormula { grounder, .. } = event {
            self.grounder = Some(grounder);
        }
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let admitted = admit_formula(
        "ready.".into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )?;
    let config = SolveConfig {
        backend: Backend::Cpu,
        grounder: Grounder::Eager,
        models: 0,
        ..SolveConfig::default()
    };
    let mut preparation = Preparation::default();
    let mut session = Session::enumerate_observed(
        PreparedInput::formula(&admitted),
        config,
        Cancellation::default(),
        &mut preparation,
    )?;
    assert_eq!(preparation.grounder, Some(Grounder::Eager));

    let answer = session.next_observed(&mut preparation).unwrap()?;
    assert_eq!(
        answer.interpretation(),
        &Model::new([Atom::new(Predicate::new("ready", 0)?, vec![])?])?
    );
    assert!(session.next_observed(&mut preparation).is_none());
    assert_eq!(
        session.outcome().unwrap().completion(),
        Some(Completion::Exhausted)
    );
    Ok(())
}
// ANCHOR_END: example
