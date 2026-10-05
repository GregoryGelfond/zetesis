//! Build a program, enumerate its answers and query the complete family.

#[path = "shared/agent.rs"]
mod execution;

// ANCHOR: example
use std::error::Error;
use zetesis::program::{
    Atom, Body, BodyElement, Name, Program, Relation, Rule, Sign, Signature, Statement, Term,
    program::{Choice, ChoiceElement, Condition, Guard, Show},
    symbol::VarName,
};

fn program() -> Result<Program, Box<dyn Error>> {
    let task = Name::new("task")?;
    let run = Name::new("run")?;
    let task_names = ["build", "test", "deploy"]
        .map(|name| Name::new(name).map(Term::constant))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let facts = Rule::fact(Atom::new(task.clone(), [Term::pool(task_names)?]));

    let variable = Term::variable(VarName::new("T")?);
    let selection = ChoiceElement::new(
        Atom::new(run.clone(), [variable.clone()]).into(),
        Condition::new([Atom::new(task, [variable]).into()]),
    );
    let bound = || Guard {
        relation: Some(Relation::Le),
        term: 2.into(),
    };
    let choice = Choice::new(Some(bound()), [selection], Some(bound()));
    let incompatible = ["build", "deploy"]
        .map(|name| {
            let argument = Term::constant(Name::new(name)?);
            Ok(BodyElement::Literal(
                Atom::new(run.clone(), [argument]).into(),
            ))
        })
        .into_iter()
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    let statements: [Statement; 4] = [
        facts.into(),
        Rule::fact(choice).into(),
        Rule::constraint(Body::new(incompatible)).into(),
        Show::Signature(Signature::new(Sign::Positive, run, 1)).into(),
    ];
    Ok(Program::of(statements))
}

fn main() -> Result<(), Box<dyn Error>> {
    execution::report(program()?)
}
// ANCHOR_END: example

#[test]
fn snapshot_preserves_the_complete_answer_family() -> Result<(), Box<dyn Error>> {
    execution::check_complete_family(program()?)
}

#[test]
fn shown_answers_contain_only_selected_tasks() -> Result<(), Box<dyn Error>> {
    execution::check_shown_terms(program()?)
}

#[test]
fn test_runs_in_every_answer() -> Result<(), Box<dyn Error>> {
    execution::check_query(program()?)
}
