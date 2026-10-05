//! Native solving through the facade as the only application dependency.

use std::error::Error;
use z::{
    Solver, atom, program,
    query::{AgentReading, Answer, Query},
    solve::agent::Agent,
};

#[test]
fn renamed_facade_queries_the_complete_native_family() -> Result<(), Box<dyn Error>> {
    let mut agent = Agent::new(
        program! {
            p.
            { q }.
            #show q/0.
        },
        Solver::default(),
    );
    let snapshot = agent.snapshot()?;
    assert_eq!(snapshot.members().count(), 2);
    assert_eq!(snapshot.answer(&Query::of(atom! { p })?), Answer::Yes);
    assert_eq!(snapshot.answer(&Query::of(atom! { q })?), Answer::Unknown);
    Ok(())
}
