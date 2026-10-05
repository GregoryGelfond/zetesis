//! Assert and retract a constraint between questions over one knowledge base.

// ANCHOR: example
use std::error::Error;
use zetesis::{
    Solver, atom, constraint, program,
    query::{AgentReading, Query},
    solve::agent::Agent,
};

fn main() -> Result<(), Box<dyn Error>> {
    let mut agent = Agent::new(
        program! { 1 { route(north); route(south) } 1. },
        Solver::default(),
    );
    let north = Query::of(atom! { route(north) })?;
    println!("Before: {}", agent.answer(&north)?);

    let closure = agent.assert(constraint! { :- route(south) })?;
    println!("South closed: {}", agent.answer(&north)?);

    agent.retract(closure)?;
    println!("Reopened: {}", agent.answer(&north)?);
    Ok(())
}
// ANCHOR_END: example

#[test]
fn retracting_the_constraint_restores_the_family() -> Result<(), Box<dyn Error>> {
    use std::collections::BTreeSet;
    let mut agent = Agent::new(
        program! { 1 { route(north); route(south) } 1. },
        Solver::default(),
    );
    let original = agent
        .snapshot()?
        .members()
        .map(|model| model.atoms().clone())
        .collect::<BTreeSet<_>>();
    let closure = agent.assert(constraint! { :- route(south) })?;
    assert_eq!(
        agent.answer(&Query::of(atom! { route(north) })?)?,
        zetesis::query::Answer::Yes
    );
    agent.retract(closure)?;
    let restored = agent
        .snapshot()?
        .members()
        .map(|model| model.atoms().clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(restored, original);
    Ok(())
}
