//! Build a program, enumerate its answers and query the complete family.

#[path = "shared/agent.rs"]
mod execution;

// ANCHOR: example
use std::error::Error;
use zetesis::{Program, program};

fn program() -> Program {
    program! {
        task(build; test; deploy).
        2 { run(T) : task(T) } 2.
        :- run(build), run(deploy).
        #show run/1.
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    execution::report(program())
}
// ANCHOR_END: example

#[test]
fn snapshot_preserves_the_complete_answer_family() -> Result<(), Box<dyn Error>> {
    execution::check_complete_family(program())
}

#[test]
fn shown_answers_contain_only_selected_tasks() -> Result<(), Box<dyn Error>> {
    execution::check_shown_terms(program())
}

#[test]
fn test_runs_in_every_answer() -> Result<(), Box<dyn Error>> {
    execution::check_query(program())
}
