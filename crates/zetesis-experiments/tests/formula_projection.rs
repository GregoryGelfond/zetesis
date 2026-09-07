//! Portable admission boundaries of the explicitly physical paired experiment.

use clap::Parser;
use std::io::{self, Write};
use zetesis_experiments::{
    Backend, CommandOptions, Experiment, FormulaBenchmarkError, FormulaOptions,
    run_formula_projection,
};

fn projection_options(backend: &str) -> FormulaOptions {
    let parsed = CommandOptions::try_parse_from([
        "zetesis-bench",
        "formula-projection",
        "--backend",
        backend,
        "--atoms",
        "3",
        "--batches",
        "8",
        "--repetitions",
        "2",
    ])
    .unwrap();
    let Some(Experiment::FormulaProjection(options)) = parsed.command else {
        panic!("paired formula command")
    };
    options
}

#[test]
fn cpu_selection_cannot_claim_projection_qualification() {
    let mut output = Vec::new();
    let error = run_formula_projection(&projection_options("cpu"), &mut output).unwrap_err();
    assert!(matches!(
        error,
        FormulaBenchmarkError::ProjectionRequiresMetal
    ));
    assert!(output.is_empty());
}

#[test]
fn empty_projection_dimensions_fail_before_device_selection() {
    let mut options = projection_options("metal");
    assert_eq!(options.backend, Backend::Metal);
    options.atoms.clear();
    let mut output = Vec::new();
    assert!(matches!(
        run_formula_projection(&options, &mut output),
        Err(FormulaBenchmarkError::Dimensions)
    ));
    assert!(output.is_empty());
}

struct ClosedOutput;
impl Write for ClosedOutput {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::ErrorKind::BrokenPipe.into())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn closed_projection_output_stops_before_device_selection() {
    let error =
        run_formula_projection(&projection_options("metal"), &mut ClosedOutput).unwrap_err();
    let FormulaBenchmarkError::Output(error) = error else {
        panic!("the first header write must fail")
    };
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
}

#[test]
fn projection_help_states_the_execution_scope() {
    let help = CommandOptions::try_parse_from(["zetesis-bench", "formula-projection", "--help"])
        .unwrap_err();
    assert_eq!(help.kind(), clap::error::ErrorKind::DisplayHelp);
    let text = help.to_string();
    for scope in [
        "per-candidate CPU quotas",
        "Requires physical Metal",
        "Hybrid residual checks stay serial",
        "cumulative-budget",
    ] {
        assert!(text.contains(scope), "missing execution boundary: {scope}");
    }
}
