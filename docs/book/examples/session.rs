//! Enumerate and inspect the complete answer-set family through a Rust session.

// ANCHOR: example
use std::collections::BTreeSet;
use zetesis_algebra::Cancellation;
use zetesis_core::{Atom, Model, Predicate};
use zetesis_ferraris::Interpretation;
use zetesis_solve::{
    Backend, Completion, PreparedInput, Session, SolveConfig, WorldView, WorldViewLimits,
};
use zetesis_stable::{Check, Limits, check};
use zetesis_themelios::{AdmissionOptions, ExpansionLimits, FormulaLimits, admit_formula};

const CHOICES: &str = "a :- not b.\nb :- not a.\n";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let admitted = admit_formula(
        CHOICES.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )?;
    let config = SolveConfig {
        backend: Backend::Cpu,
        models: 0,
        ..SolveConfig::default()
    };
    let mut session = Session::enumerate(
        PreparedInput::formula(&admitted),
        config,
        Cancellation::default(),
    )?;
    let answers = session.by_ref().collect::<Result<Vec<_>, _>>()?;
    let outcome = session.outcome().expect("the iterator reached its end");

    assert_eq!(answers.len(), 2);
    let family = answers
        .iter()
        .map(|answer| answer.interpretation().clone())
        .collect::<BTreeSet<_>>();
    let a = Atom::new(Predicate::new("a", 0)?, vec![])?;
    let b = Atom::new(Predicate::new("b", 0)?, vec![])?;
    assert_eq!(
        family,
        BTreeSet::from([Model::new([a.clone()])?, Model::new([b])?])
    );
    assert_eq!(outcome.verified_models(), 2);
    assert_eq!(outcome.completion(), Some(Completion::Exhausted));
    assert!(!outcome.unsatisfiable());

    // Check the tour's representative candidate directly, without a search
    // session: the frozen reduct's proper-subset query decides membership.
    let index = admitted
        .atoms()
        .iter()
        .position(|atom| atom == a)
        .ok_or("the theory contains a")?;
    let candidate = Interpretation::new(admitted.theory(), [index])?;
    let verdict = check(
        admitted.theory(),
        &candidate,
        Limits::default(),
        &Cancellation::default(),
    );
    assert!(matches!(verdict, Check::Stable));

    // A fresh bounded collection owns the complete-family claim. A Vec gathered
    // from an arbitrary stream cannot acquire it from a separate outcome.
    let world_view = WorldView::collect(
        PreparedInput::formula(&admitted),
        config,
        WorldViewLimits {
            max_answer_sets: 2,
            ..WorldViewLimits::default()
        },
        Cancellation::default(),
    )?;
    assert_eq!(world_view.len(), 2);
    assert!(!world_view.is_empty());
    Ok(())
}
// ANCHOR_END: example

#[test]
fn tour_fixture_matches_the_session_source() {
    assert_eq!(CHOICES, include_str!("choices.lp"));
}
