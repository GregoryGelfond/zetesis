//! Prepare ASP source, stream typed answer sets, and check complete coverage.

// ANCHOR: example
use std::{
    error::Error,
    io::{self, Write},
    num::NonZeroUsize,
};
use zetesis_cpu::Cancellation;
use zetesis_solve::{Backend, Completion, Grounder, PreparedInput, Session, SolveConfig};
use zetesis_themelios::{
    AdmissionOptions, ExpansionLimits, FormulaLimits, OutputSelection, observation, prepare_formula,
};

// Choose two tasks, but never build and deploy in the same answer.
const SOURCE: &str = r"
    task(build;test;deploy).
    2 { run(T) : task(T) } 2.
    :- run(build), run(deploy).
    #show run/1.
";

fn main() -> Result<(), Box<dyn Error>> {
    let input = prepare_formula(
        SOURCE.into(),
        AdmissionOptions::default(),
        ExpansionLimits::default(),
        FormulaLimits::default(),
    )?
    .ground()?;
    let config = SolveConfig {
        backend: Backend::Cpu,
        grounder: Grounder::Eager,
        workers: NonZeroUsize::MIN,
        models: 0, // Request the complete family, rather than one answer.
        ..SolveConfig::default()
    };
    let cancellation = Cancellation::default();
    let mut session =
        Session::enumerate(PreparedInput::formula(&input), config, cancellation.clone())?;
    let mut output = io::stdout().lock();
    for answer in session.by_ref() {
        let answer = answer?; // Preserve a failed pull instead of dropping it.
        let interpretation = answer.interpretation();
        // No #show selection: spell every atom in the typed interpretation.
        let full = observation::ObservationProgram::default().render(
            interpretation,
            &OutputSelection::default(),
            observation::Limits::default(),
            &cancellation,
        )?;
        let shown = input.metadata().observations().render(
            interpretation,
            input.metadata().output(),
            observation::Limits::default(),
            &cancellation,
        )?;
        writeln!(
            output,
            "Full answer: {}\n#show: {}\n",
            full.text(),
            shown.text()
        )?;
    }
    let outcome = session.outcome().ok_or("no final search outcome")?;
    if let Some(reason) = outcome.interruption() {
        return Err(
            format!("incomplete enumeration; printed answers are a prefix: {reason}").into(),
        );
    }
    if outcome.completion() != Some(Completion::Exhausted) {
        return Err("enumeration ended without establishing the complete family".into());
    }
    writeln!(
        output,
        "Complete family: {} answer sets.",
        outcome.verified_models()
    )?;
    Ok(())
}
// ANCHOR_END: example

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use zetesis_core::Value;

    #[test]
    fn quickstart_preserves_the_full_family_behind_show() -> Result<(), Box<dyn Error>> {
        let input = prepare_formula(
            SOURCE.into(),
            AdmissionOptions::default(),
            ExpansionLimits::default(),
            FormulaLimits::default(),
        )?
        .ground()?;
        let mut session = Session::enumerate(
            PreparedInput::formula(&input),
            SolveConfig {
                backend: Backend::Cpu,
                models: 0,
                workers: NonZeroUsize::MIN,
                ..Default::default()
            },
            Cancellation::default(),
        )?;
        let mut displays = BTreeSet::new();
        for answer in session.by_ref() {
            let answer = answer?;
            let model = answer.interpretation();
            assert_eq!(model.atoms().len(), 5);
            let tasks = model
                .atoms()
                .iter()
                .filter(|atom| atom.predicate().name() == "task")
                .map(|atom| atom.values().to_vec())
                .collect::<BTreeSet<_>>();
            assert_eq!(
                tasks,
                BTreeSet::from(
                    ["build", "test", "deploy"].map(|task| vec![Value::Symbol(task.into())])
                )
            );
            let shown = input.metadata().observations().render(
                model,
                input.metadata().output(),
                observation::Limits::default(),
                &Cancellation::default(),
            )?;
            assert!(displays.insert(shown.text().to_owned()));
        }
        assert_eq!(
            displays,
            BTreeSet::from([
                "run(build) run(test)".to_owned(),
                "run(deploy) run(test)".to_owned()
            ])
        );
        assert_eq!(session.outcome().unwrap().verified_models(), 2);
        assert_eq!(
            session.outcome().unwrap().completion(),
            Some(Completion::Exhausted)
        );
        Ok(())
    }
}
