// Shared Agent/Solver execution for both canonical construction examples.
// ANCHOR: example
use std::error::Error;
use zetesis::{
    Program, Solver,
    program::{Atom, Name, Term},
    query::{AgentReading, Query, Snapshot},
    solve::agent::Agent,
};

fn snapshot(knowledge: Program) -> Result<Snapshot, Box<dyn Error>> {
    Ok(Agent::new(knowledge, Solver::default()).snapshot()?)
}

fn test_runs_query() -> Result<Query, Box<dyn Error>> {
    let test = Term::constant(Name::new("test")?);
    Ok(Query::of(Atom::new(Name::new("run")?, [test]))?)
}

pub(super) fn report(knowledge: Program) -> Result<(), Box<dyn Error>> {
    let snapshot = snapshot(knowledge)?;
    for model in snapshot.members() {
        println!("Shown: {:?}", model.shown().symbols());
    }
    println!(
        "Does every answer run test? {}",
        snapshot.answer(&test_runs_query()?)
    );
    Ok(())
}
// ANCHOR_END: example

// ANCHOR: hidden
// ANCHOR_END: hidden

#[cfg(test)]
use std::collections::BTreeSet;
#[cfg(test)]
use zetesis::program::{AnswerSet, Sign, Symbol};

#[cfg(test)]
fn applied(predicate: &str, argument: &str) -> Result<Symbol, Box<dyn Error>> {
    Ok(Symbol::function(
        Name::new(predicate)?,
        [Symbol::constant(Name::new(argument)?)],
        Sign::Positive,
    ))
}

#[cfg(test)]
pub(super) fn check_complete_family(knowledge: Program) -> Result<(), Box<dyn Error>> {
    let snapshot = snapshot(knowledge)?;
    let facts = ["build", "test", "deploy"]
        .map(|name| applied("task", name))
        .into_iter()
        .collect::<Result<AnswerSet, _>>()?;
    let mut expected = BTreeSet::new();
    for companion in ["build", "deploy"] {
        let mut answer = facts.clone();
        answer.extend([applied("run", "test")?, applied("run", companion)?]);
        expected.insert(answer);
    }
    assert_eq!(
        snapshot
            .members()
            .map(|model| model.atoms().clone())
            .collect::<BTreeSet<_>>(),
        expected
    );
    Ok(())
}

#[cfg(test)]
pub(super) fn check_shown_terms(knowledge: Program) -> Result<(), Box<dyn Error>> {
    let snapshot = snapshot(knowledge)?;
    let expected = ["build", "deploy"]
        .map(|companion| {
            Ok(BTreeSet::from([
                applied("run", "test")?,
                applied("run", companion)?,
            ]))
        })
        .into_iter()
        .collect::<Result<BTreeSet<_>, Box<dyn Error>>>()?;
    assert_eq!(
        snapshot
            .members()
            .map(|model| model.shown().symbols().clone())
            .collect::<BTreeSet<_>>(),
        expected
    );
    Ok(())
}

#[cfg(test)]
pub(super) fn check_query(knowledge: Program) -> Result<(), Box<dyn Error>> {
    let snapshot = snapshot(knowledge)?;
    assert_eq!(
        snapshot.answer(&test_runs_query()?),
        zetesis::query::Answer::Yes
    );
    Ok(())
}
