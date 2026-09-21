//! Solve source from Rust and check whether enumeration completed.

// ANCHOR: example
use std::io::Write;
use zetesis_cpu::Control;
use zetesis_solve::{Backend, Completion, PreparedInput, Session, SolveConfig};
use zetesis_themelios::{AdmissionOptions, admit};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let admitted = admit(
        "a :- not b. b :- not a.".into(),
        AdmissionOptions::default(),
    )?;
    let config = SolveConfig {
        backend: Backend::Cpu,
        models: 0,
        ..SolveConfig::default()
    };
    let mut session = Session::enumerate(
        PreparedInput::admitted(&admitted),
        config,
        Control::default(),
    )?;
    let mut output = std::io::stdout().lock();
    for answer in session.by_ref() {
        writeln!(output, "{:?}", answer?.interpretation())?;
    }
    let outcome = session
        .outcome()
        .ok_or("the session has no final outcome")?;
    if outcome.completion() != Some(Completion::Exhausted) {
        return Err("answer-set enumeration did not complete".into());
    }
    Ok(())
}
// ANCHOR_END: example
